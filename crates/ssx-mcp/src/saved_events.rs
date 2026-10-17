//! W05 real native event bindings. Authenticated composition supplies the typed
//! principal in EventService; this module never derives authority from arguments.
//! Complete production workflow/auth composition remains W09.
use rmcp::{ErrorData, ServerHandler, model::*};
use ssx_application::saved::{SavedPolicy, prepare_create};
use ssx_infra::{
    saved_events::EventService,
    saved_runner::{SavedRunner, StaticSavedAuthority},
    search::SearchService,
    valkey::saved::SavedStore,
};
use ssx_json::{Value, json};
use std::{sync::Arc, time::Duration};
use tokio::sync::{Mutex, RwLock, watch};
#[derive(Clone)]
pub struct SavedEventRegistry {
    events: EventService,
    runner: SavedRunner,
    authority: Arc<StaticSavedAuthority>,
    stop: watch::Sender<bool>,
    admission: Arc<RwLock<bool>>,
    tasks: Arc<Mutex<tokio::task::JoinSet<()>>>,
}
impl SavedEventRegistry {
    pub fn new(
        events: EventService,
        search: SearchService,
        authority: Arc<StaticSavedAuthority>,
        timeout: Duration,
    ) -> Result<Self, ssx_infra::saved_runner::RunnerError> {
        let runner = SavedRunner::new(events.store().clone(), search, authority.clone(), timeout)?
            .with_events(events.publisher().clone());
        let (stop, _) = watch::channel(false);
        Ok(Self {
            events,
            runner,
            authority,
            stop,
            admission: Default::default(),
            tasks: Default::default(),
        })
    }
    pub async fn close(&self) {
        let mut admission = self.admission.write().await;
        *admission = true;
        self.stop.send_replace(true);
        drop(admission);
        let mut tasks = self.tasks.lock().await;
        while tasks.join_next().await.is_some() {}
    }
    fn descriptors() -> serde_json::Value {
        let source: serde_json::Value =
            serde_json::from_str(include_str!("../fixtures/python-sse.json"))
                .expect("captured descriptor fixture");
        let names = [
            "slopsearx_create_saved_search",
            "slopsearx_read_saved_search_events",
            "slopsearx_ack_saved_search_events",
        ];
        serde_json::Value::Array(
            source["observations"]["public_tools"]
                .as_array()
                .expect("captured tools")
                .iter()
                .filter(|v| v["name"].as_str().is_some_and(|n| names.contains(&n)))
                .cloned()
                .collect(),
        )
    }
    async fn invoke(&self, name: &str, args: &Value) -> Result<Value, ErrorData> {
        // Retain admission through all awaited store work and task registration.
        // close obtains exclusive admission before signalling and draining.
        let admission = self.admission.read().await;
        if *admission {
            return Err(ErrorData::internal_error(
                "saved event registry is closed",
                None,
            ));
        }
        let invalid = || ErrorData::invalid_params("invalid saved-event tool arguments", None);
        let object = args.as_object().ok_or_else(invalid)?;
        let fields: &[&str] = match name {
            "slopsearx_read_saved_search_events" => &["consumer_id", "cursor", "limit"],
            "slopsearx_ack_saved_search_events" => &["consumer_id", "cursor"],
            "slopsearx_create_saved_search" => &[
                "query",
                "engines",
                "interval_seconds",
                "retention_seconds",
                "expires_in_seconds",
                "max_results",
                "max_reports",
                "start_immediately",
            ],
            _ => return Err(ErrorData::invalid_params("unknown tool", None)),
        };
        if object.keys().any(|k| !fields.contains(&k.as_str())) {
            return Err(invalid());
        }
        if name == "slopsearx_read_saved_search_events" {
            let consumer = args["consumer_id"].as_str().ok_or_else(invalid)?;
            let cursor = match object.get("cursor") {
                Some(Value::Null) | None => None,
                Some(v) => Some(v.as_str().ok_or_else(invalid)?),
            };
            let limit = match object.get("limit") {
                None => 50,
                Some(v) => v
                    .as_u64()
                    .and_then(|v| usize::try_from(v).ok())
                    .ok_or_else(invalid)?,
            };
            return Ok(self.events.read(consumer, cursor, limit).await);
        }
        if name == "slopsearx_ack_saved_search_events" {
            return Ok(self
                .events
                .ack(
                    args["consumer_id"].as_str().ok_or_else(invalid)?,
                    args["cursor"].as_str().ok_or_else(invalid)?,
                )
                .await);
        }
        if !self.authority.saved_granted {
            return Ok(ssx_infra::saved_events::error(
                "tool_disabled",
                "saved searches require the saved-search grant",
            ));
        }
        let policy = SavedPolicy {
            search: &self.authority.search,
            saved_granted: self.authority.saved_granted,
            auth_configured: &self.authority.auth_configured,
            limits: &self.authority.limits,
        };
        let store = self.events.store();
        let now = match store.now().await {
            Ok(v) => v,
            Err(_) => {
                return Ok(ssx_infra::saved_events::error(
                    "store_unavailable",
                    "saved searches require connected Valkey",
                ));
            }
        };
        let id = SavedStore::generate_id()
            .map_err(|_| ErrorData::internal_error("saved identity unavailable", None))?;
        let prepared =
            match prepare_create(&policy, store.tenant().as_str(), id.as_str(), args, now) {
                Ok(v) => v,
                Err(e) => return Ok(ssx_infra::saved_events::error("invalid_input", e.0)),
            };
        if store.create(&prepared).await.is_err() {
            return Ok(ssx_infra::saved_events::error(
                "store_unavailable",
                "saved search could not be created",
            ));
        }
        let d = prepared.definition();
        let output = json!({"search_id":d.search_id,"artifact":{"contract":"slopsearx.artifact_ref","version":1,"kind":"saved_search","id":d.search_id},"revision":d.revision,"query":d.query,"engines":d.engines,"interval_seconds":d.interval_seconds,"retention_seconds":d.retention_seconds,"max_results":d.max_results,"max_reports":d.max_reports,"created_at":d.created_at.get(),"expires_at":d.expires_at.get(),"next_due":d.next_due.get(),"paused":d.paused,"latest_run_id":d.latest_run_id,"comparison":{"window":"fixed_first_page","identity_version":"url-source-window-v1","absence_event":"not_observed_in_latest_run","absence_is_deletion":false}});
        if args["start_immediately"] == Value::Bool(true) {
            let runner = self.runner.clone();
            let stop = self.stop.subscribe();
            let mut tasks = self.tasks.lock().await;
            while tasks.try_join_next().is_some() {}
            tasks.spawn(async move {
                let _ = runner.run_one(id, stop).await;
            });
        }
        Ok(output)
    }
}
impl ServerHandler for SavedEventRegistry {
    fn get_info(&self) -> ServerConfig {
        ServerConfig::new(ServerCapabilities::builder().enable_tools().build())
    }
    async fn list_tools(
        &self,
        _: Option<PaginatedRequestParams>,
        _: rmcp::service::RequestContext<rmcp::RoleServer>,
    ) -> Result<ListToolsResult, ErrorData> {
        serde_json::from_value(serde_json::json!({"tools":Self::descriptors()}))
            .map_err(|_| ErrorData::internal_error("invalid saved event descriptors", None))
    }
    async fn call_tool(
        &self,
        request: CallToolRequestParams,
        _: rmcp::service::RequestContext<rmcp::RoleServer>,
    ) -> Result<CallToolResponse, ErrorData> {
        let raw = serde_json::to_vec(&request.arguments.unwrap_or_default())
            .map_err(|_| ErrorData::invalid_params("invalid arguments", None))?;
        if raw.len() > 65536 {
            return Err(ErrorData::invalid_params(
                "saved event arguments exceed bound",
                None,
            ));
        }
        let args = ssx_json::from_slice(&raw)
            .map_err(|_| ErrorData::invalid_params("invalid arguments", None))?;
        let value = self.invoke(request.name.as_ref(), &args).await?;
        let text = ssx_json::to_string(&value).map_err(|_| {
            ErrorData::internal_error("saved event response serialization failed", None)
        })?;
        let structured = serde_json::from_str(&text).map_err(|_| {
            ErrorData::internal_error("saved event structured response serialization failed", None)
        })?;
        let mut response = CallToolResult::success(vec![ContentBlock::text(text)]);
        response.structured_content = Some(structured);
        Ok(response.into())
    }
}
/// Actual rmcp transport; caller owns authentication, principal resolution and
/// joined registry/HTTP lifecycle. Tests provide an authenticated fixed principal.
pub fn http_router(
    registry: SavedEventRegistry,
    cancel: tokio_util::sync::CancellationToken,
) -> axum::Router {
    use rmcp::transport::streamable_http_server::{
        StreamableHttpServerConfig, StreamableHttpService, session::local::LocalSessionManager,
    };
    let service: StreamableHttpService<SavedEventRegistry, LocalSessionManager> =
        StreamableHttpService::new(
            move || Ok(registry.clone()),
            Arc::new(LocalSessionManager::default()),
            StreamableHttpServerConfig::default()
                .with_legacy_session_mode(false)
                .with_json_response(true)
                .with_cancellation_token(cancel),
        );
    axum::Router::new().nest_service("/mcp", service)
}
