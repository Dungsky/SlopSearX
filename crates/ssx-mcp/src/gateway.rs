//! Bounded native discovery and forwarding over one owned upstream MCP session.
use crate::{Registry, Result};
use rmcp::{
    RoleClient, ServiceExt,
    model::*,
    service::RunningService,
    transport::{
        StreamableHttpClientTransport, streamable_http_client::StreamableHttpClientTransportConfig,
    },
};
use serde_json::{Value, json};
use std::{collections::HashSet, sync::Arc, time::Duration};
pub struct Gateway {
    pub service: RunningService<RoleClient, ClientConfig>,
    pub catalog: Value,
}
impl Gateway {
    pub async fn connect(url: String, token: Option<String>, deadline: Duration) -> Result<Self> {
        let mut config = StreamableHttpClientTransportConfig::with_uri(url);
        if let Some(token) = token {
            config = config.auth_header(token);
        }
        let http = reqwest::Client::builder()
            .redirect(reqwest::redirect::Policy::none())
            .build()?;
        let transport = StreamableHttpClientTransport::with_client(http, config);
        let service = tokio::time::timeout(
            deadline,
            ClientConfig::default()
                .with_protocol_version(ProtocolVersion::V_2025_11_25)
                .serve(transport),
        )
        .await??;
        let discovery = async {
            let tools = collect_pages(|cursor| {
                let s = &service;
                async move {
                    let page = s
                        .list_tools(
                            cursor.map(|c| PaginatedRequestParams::default().with_cursor(Some(c))),
                        )
                        .await?;
                    Ok((serde_json::to_value(page.tools)?, page.next_cursor))
                }
            })
            .await?;
            let resources = collect_pages_by(catalog_identity("resources/list")?, |cursor| {
                let s = &service;
                async move {
                    let page = s
                        .list_resources(
                            cursor.map(|c| PaginatedRequestParams::default().with_cursor(Some(c))),
                        )
                        .await?;
                    Ok((serde_json::to_value(page.resources)?, page.next_cursor))
                }
            })
            .await?;
            let prompts = collect_pages(|cursor| {
                let s = &service;
                async move {
                    let page = s
                        .list_prompts(
                            cursor.map(|c| PaginatedRequestParams::default().with_cursor(Some(c))),
                        )
                        .await?;
                    Ok((serde_json::to_value(page.prompts)?, page.next_cursor))
                }
            })
            .await?;
            Ok::<_, crate::Error>(json!({"tools":tools,"resources":resources,"prompts":prompts}))
        };
        match tokio::time::timeout(deadline, discovery).await {
            Ok(Ok(catalog)) => Ok(Self { service, catalog }),
            other => {
                let _ = service.cancel().await;
                Err(format!("gateway discovery failed: {other:?}").into())
            }
        }
    }
    pub fn registry(&self) -> Registry {
        let mut registry = Registry::synthetic();
        registry.descriptors = Arc::new(self.catalog.clone());
        registry
    }
    pub async fn shutdown(self) -> Result<()> {
        self.service.cancel().await?;
        Ok(())
    }
}
pub fn catalog_identity(method: &str) -> Result<&'static str> {
    match method {
        "tools/list" | "prompts/list" => Ok("name"),
        "resources/list" => Ok("uri"),
        "resources/templates/list" => Ok("uriTemplate"),
        _ => Err("unknown MCP catalog identity".into()),
    }
}
pub async fn collect_pages<F, Fut>(fetch: F) -> Result<Vec<Value>>
where
    F: FnMut(Option<String>) -> Fut,
    Fut: std::future::Future<Output = Result<(Value, Option<String>)>>,
{
    collect_pages_by("name", fetch).await
}
pub async fn collect_pages_by<F, Fut>(identity: &str, mut fetch: F) -> Result<Vec<Value>>
where
    F: FnMut(Option<String>) -> Fut,
    Fut: std::future::Future<Output = Result<(Value, Option<String>)>>,
{
    let mut out = Vec::new();
    let mut names = HashSet::new();
    let mut cursors = HashSet::new();
    let mut cursor = None;
    for _ in 0..128 {
        let (items, next) = fetch(cursor).await?;
        for item in items.as_array().ok_or("catalog page must be an array")? {
            let name = item
                .get(identity)
                .and_then(Value::as_str)
                .ok_or("catalog item lacks identity")?;
            if !names.insert(name.to_string()) {
                return Err("duplicate catalog name".into());
            }
            out.push(item.clone());
            if out.len() > 4096 {
                return Err("catalog exceeds 4096 items".into());
            }
        }
        let Some(next) = next else {
            return Ok(out);
        };
        if !cursors.insert(next.clone()) {
            return Err("repeated catalog cursor".into());
        }
        cursor = Some(next);
    }
    Err("catalog exceeds 128 pages".into())
}
#[derive(Clone)]
pub struct Forwarder {
    registry: Registry,
    peer: rmcp::Peer<RoleClient>,
}
impl Gateway {
    pub fn handler(&self) -> Forwarder {
        Forwarder {
            registry: self.registry(),
            peer: self.service.peer().clone(),
        }
    }
}
fn remote_error(e: rmcp::service::ServiceError) -> rmcp::ErrorData {
    match e {
        rmcp::service::ServiceError::McpError(e) => e,
        _ => rmcp::ErrorData::internal_error("remote transport unavailable", None),
    }
}
impl Forwarder {
    async fn forward(
        &self,
        request: ClientRequest,
        context: rmcp::service::RequestContext<rmcp::RoleServer>,
    ) -> std::result::Result<ServerResult, rmcp::ErrorData> {
        let mut handle = self
            .peer
            .send_cancellable_request(request, rmcp::service::PeerRequestOptions::no_options())
            .await
            .map_err(remote_error)?;
        tokio::select! {
            response=&mut handle.rx=>response.map_err(|_|rmcp::ErrorData::internal_error("remote transport closed",None))?.map_err(remote_error),
            _=context.ct.cancelled()=>{
                let _=tokio::time::timeout(Duration::from_secs(1),handle.cancel(Some("downstream cancelled".into()))).await;
                Err(rmcp::ErrorData::internal_error("cancelled",None))
            }
        }
    }
}
impl rmcp::ServerHandler for Forwarder {
    fn get_info(&self) -> ServerConfig {
        self.registry.get_info()
    }
    async fn list_tools(
        &self,
        r: Option<PaginatedRequestParams>,
        c: rmcp::service::RequestContext<rmcp::RoleServer>,
    ) -> std::result::Result<ListToolsResult, rmcp::ErrorData> {
        self.registry.list_tools(r, c).await
    }
    async fn list_resources(
        &self,
        r: Option<PaginatedRequestParams>,
        c: rmcp::service::RequestContext<rmcp::RoleServer>,
    ) -> std::result::Result<ListResourcesResult, rmcp::ErrorData> {
        self.registry.list_resources(r, c).await
    }
    async fn list_prompts(
        &self,
        r: Option<PaginatedRequestParams>,
        c: rmcp::service::RequestContext<rmcp::RoleServer>,
    ) -> std::result::Result<ListPromptsResult, rmcp::ErrorData> {
        self.registry.list_prompts(r, c).await
    }
    async fn call_tool(
        &self,
        r: CallToolRequestParams,
        c: rmcp::service::RequestContext<rmcp::RoleServer>,
    ) -> std::result::Result<CallToolResponse, rmcp::ErrorData> {
        match self
            .forward(ClientRequest::CallToolRequest(CallToolRequest::new(r)), c)
            .await?
        {
            ServerResult::CallToolResult(result) => Ok(result.into()),
            ServerResult::InputRequiredResult(result) => {
                Ok(CallToolResponse::InputRequired(result))
            }
            _ => Err(rmcp::ErrorData::internal_error(
                "unexpected upstream tool response",
                None,
            )),
        }
    }
    async fn read_resource(
        &self,
        r: ReadResourceRequestParams,
        c: rmcp::service::RequestContext<rmcp::RoleServer>,
    ) -> std::result::Result<ReadResourceResponse, rmcp::ErrorData> {
        match self
            .forward(
                ClientRequest::ReadResourceRequest(ReadResourceRequest::new(r)),
                c,
            )
            .await?
        {
            ServerResult::ReadResourceResult(result) => Ok(result.into()),
            ServerResult::InputRequiredResult(result) => {
                Ok(ReadResourceResponse::InputRequired(result))
            }
            _ => Err(rmcp::ErrorData::internal_error(
                "unexpected upstream resource response",
                None,
            )),
        }
    }
    async fn get_prompt(
        &self,
        r: GetPromptRequestParams,
        c: rmcp::service::RequestContext<rmcp::RoleServer>,
    ) -> std::result::Result<GetPromptResponse, rmcp::ErrorData> {
        match self
            .forward(ClientRequest::GetPromptRequest(GetPromptRequest::new(r)), c)
            .await?
        {
            ServerResult::GetPromptResult(result) => Ok(result.into()),
            ServerResult::InputRequiredResult(result) => {
                Ok(GetPromptResponse::InputRequired(result))
            }
            _ => Err(rmcp::ErrorData::internal_error(
                "unexpected upstream prompt response",
                None,
            )),
        }
    }
}
