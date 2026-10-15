//! Real subprocess/HTTP qualification; synthetic application state, no provider or store.
use rmcp::{ServiceExt, model::*, transport::TokioChildProcess};
use serde_json::{Value, json};
use ssx_mcp::{
    Registry, Result, compare_descriptor,
    gateway::{Gateway, collect_pages},
    harness::{Host, http_router},
    legacy::Legacy,
};
use std::{sync::atomic::Ordering, time::Duration};
use tokio_util::sync::CancellationToken;
fn baseline() -> Value {
    serde_json::from_str(include_str!("../fixtures/python-sse.json")).unwrap()
}
fn config() -> ClientConfig {
    ClientConfig::default().with_protocol_version(ProtocolVersion::V_2025_11_25)
}
async fn verify<S: rmcp::Service<rmcp::RoleClient>>(
    client: &rmcp::service::RunningService<rmcp::RoleClient, S>,
) -> Result<()> {
    let fixture = baseline();
    let expected = &fixture["observations"];
    compare_descriptor(
        &expected["tools"],
        &serde_json::to_value(client.list_tools(None).await?.tools)?,
    )?;
    compare_descriptor(
        &expected["resources"],
        &serde_json::to_value(client.list_resources(None).await?.resources)?,
    )?;
    compare_descriptor(
        &expected["prompts"],
        &serde_json::to_value(client.list_prompts(None).await?.prompts)?,
    )?;
    let result =
        client
            .call_tool(CallToolRequestParams::new("t01_echo").with_arguments(
                serde_json::from_value(json!({"payload":{"items":[1,2]},"note":null}))?,
            ))
            .await?;
    compare_descriptor(&expected["echo_result"], &serde_json::to_value(result)?)?;
    let resource = client
        .read_resource(ReadResourceRequestParams::new("t01://text"))
        .await?;
    compare_descriptor(
        &expected["resource_result"],
        &serde_json::to_value(resource)?,
    )?;
    let prompt = client
        .get_prompt(
            GetPromptRequestParams::new("t01_prompt")
                .with_arguments(serde_json::from_value(json!({"topic":"test"}))?),
        )
        .await?;
    compare_descriptor(&expected["prompt_result"], &serde_json::to_value(prompt)?)?;
    assert!(
        client
            .call_tool(CallToolRequestParams::new("not-real"))
            .await
            .is_err()
    );
    assert!(
        client
            .call_tool(CallToolRequestParams::new("t01_echo").with_arguments(
                serde_json::from_value(json!({"payload":{"items":["wrong"]}}))?
            ))
            .await
            .is_err()
    );
    let error =
        client
            .call_tool(CallToolRequestParams::new("t01_echo").with_arguments(
                serde_json::from_value(json!({"payload":{"items":[]},"note":"error"}))?,
            ))
            .await?;
    assert_eq!(error.is_error, Some(true));
    Ok(())
}
async fn process(
    args: &[&str],
) -> Result<rmcp::service::RunningService<rmcp::RoleClient, ClientConfig>> {
    let mut command = tokio::process::Command::new(env!("CARGO_BIN_EXE_mcp-qualification"));
    command
        .args(args)
        .env_clear()
        .env("PATH", "")
        .kill_on_drop(true);
    let transport = TokioChildProcess::new(command)?;
    Ok(tokio::time::timeout(Duration::from_secs(5), config().serve(transport)).await??)
}
#[tokio::test]
async fn real_stdio_subprocess_preserves_complete_contract_and_clean_shutdown() -> Result<()> {
    let client = process(&["stdio"]).await?;
    verify(&client).await?;
    tokio::time::timeout(Duration::from_secs(3), client.cancel()).await??;
    Ok(())
}
#[tokio::test]
async fn real_streamable_http_bearer_and_stateless_contract() -> Result<()> {
    let ct = CancellationToken::new();
    let registry = Registry::synthetic();
    let host = Host::start(
        http_router(registry.clone(), Some("fixture-token".into()), ct.clone()),
        ct,
    )
    .await?;
    let url = format!("http://{}/mcp", host.address);
    let http = reqwest::Client::builder().no_proxy().build()?;
    for token in [None, Some("wrong")] {
        let mut request = http
            .post(&url)
            .json(&json!({"jsonrpc":"2.0","id":1,"method":"tools/list"}));
        if let Some(token) = token {
            request = request.bearer_auth(token);
        }
        let response = request.send().await?;
        assert_eq!(response.status(), 401);
        assert_eq!(
            response.headers()["content-type"],
            "text/plain; charset=utf-8"
        );
        assert_eq!(response.text().await?, "unauthorized");
    }
    let gateway = Gateway::connect(
        url.clone(),
        Some("fixture-token".into()),
        Duration::from_secs(3),
    )
    .await?;
    verify(&gateway.service).await?;
    assert_eq!(registry.calls.load(Ordering::SeqCst), 2);
    let response = http
        .post(&url)
        .bearer_auth("fixture-token")
        .header("accept", "application/json, text/event-stream")
        .json(&json!({"jsonrpc":"2.0","id":9,"method":"tools/list"}))
        .send()
        .await?;
    assert!(response.headers().get("mcp-session-id").is_none());
    assert_eq!(response.status(), 200);
    gateway.shutdown().await?;
    let address = host.address;
    host.shutdown().await?;
    assert!(tokio::net::TcpStream::connect(address).await.is_err());
    Ok(())
}
#[tokio::test]
async fn real_stdio_gateway_forwards_to_remote_and_owns_session() -> Result<()> {
    let ct = CancellationToken::new();
    let registry = Registry::synthetic();
    let host = Host::start(http_router(registry.clone(), None, ct.clone()), ct).await?;
    let url = format!("http://{}/mcp", host.address);
    let client = process(&["gateway", &url]).await?;
    verify(&client).await?;
    assert_eq!(registry.calls.load(Ordering::SeqCst), 2);
    tokio::time::timeout(Duration::from_secs(3), client.cancel()).await??;
    host.shutdown().await?;
    Ok(())
}
#[test]
fn descriptor_comparison_rejects_nested_schema_defaults_nullable_and_metadata_mutation() {
    let expected = baseline()["observations"]["tools"].clone();
    for (pointer, replacement) in [
        (
            "/0/inputSchema/properties/payload/properties/items/items/type",
            json!("string"),
        ),
        ("/0/inputSchema/properties/note/default", json!("new")),
        (
            "/0/inputSchema/properties/note/anyOf/1/type",
            json!("integer"),
        ),
        ("/0/_meta/fastmcp/tags", json!(["changed"])),
        ("/0/description", json!("changed")),
        ("/0/outputSchema/type", json!("array")),
    ] {
        let mut changed = expected.clone();
        *changed.pointer_mut(pointer).unwrap() = replacement;
        assert!(compare_descriptor(&expected, &changed).is_err());
    }
    let mut changed = expected.clone();
    changed[0].as_object_mut().unwrap().remove("_meta");
    assert!(compare_descriptor(&expected, &changed).is_err());
}
#[tokio::test]
async fn discovery_limits_are_fail_closed_and_empty_cursor_is_valid() -> Result<()> {
    let mut seen = Vec::new();
    let result = collect_pages(|cursor| {
        seen.push(cursor.clone());
        std::future::ready(Ok(if cursor.is_none() {
            (json!([{"name":"first"}]), Some("".into()))
        } else {
            (json!([{"name":"second"}]), None)
        }))
    })
    .await?;
    assert_eq!(result.len(), 2);
    assert_eq!(seen, vec![None, Some("".into())]);
    assert!(
        collect_pages(|_| std::future::ready(Ok((
            json!([{"name":"duplicate"}]),
            Some("next".into())
        ))))
        .await
        .is_err()
    );
    assert!(
        collect_pages(|_| std::future::ready(Ok((json!([]), Some("same".into())))))
            .await
            .is_err()
    );
    let mut pages = 0;
    assert!(
        collect_pages(|_| {
            pages += 1;
            std::future::ready(Ok((json!([]), Some(pages.to_string()))))
        })
        .await
        .is_err()
    );
    assert_eq!(pages, 128);
    let large: Vec<_> = (0..4097).map(|n| json!({"name":n.to_string()})).collect();
    assert!(
        collect_pages(|_| std::future::ready(Ok((json!(large), None))))
            .await
            .is_err()
    );
    assert!(
        collect_pages(|_| std::future::ready(Ok((json!([{}]), None))))
            .await
            .is_err()
    );
    Ok(())
}
struct SseClient {
    http: reqwest::Client,
    base: String,
    endpoint: String,
    response: reqwest::Response,
    buffer: String,
}
impl SseClient {
    async fn connect(base: String, path: &str) -> Result<Self> {
        let http = reqwest::Client::builder().no_proxy().build()?;
        let response = http.get(format!("{base}{path}")).send().await?;
        assert_eq!(response.status(), 200);
        assert_eq!(
            response.headers()["content-type"],
            baseline()["observations"]["content_type"].as_str().unwrap()
        );
        let mut client = Self {
            http,
            base,
            endpoint: String::new(),
            response,
            buffer: String::new(),
        };
        let (kind, data) = client.event().await?;
        assert_eq!(kind, "endpoint");
        client.endpoint = data;
        Ok(client)
    }
    async fn event(&mut self) -> Result<(String, String)> {
        tokio::time::timeout(Duration::from_secs(3), async {
            loop {
                self.buffer = self.buffer.replace("\r\n", "\n");
                if let Some((raw, rest)) = self.buffer.split_once("\n\n") {
                    let raw = raw.to_owned();
                    self.buffer = rest.to_owned();
                    let mut kind = String::new();
                    let mut data = String::new();
                    for line in raw.lines() {
                        if let Some(value) = line.strip_prefix("event:") {
                            kind = value.trim().into();
                        }
                        if let Some(value) = line.strip_prefix("data:") {
                            if !data.is_empty() {
                                data.push('\n');
                            }
                            data.push_str(value.strip_prefix(' ').unwrap_or(value));
                        }
                    }
                    if !data.is_empty() {
                        return Ok((kind, data));
                    }
                } else {
                    let chunk = self.response.chunk().await?.ok_or("SSE disconnected")?;
                    self.buffer
                        .push_str(&String::from_utf8(chunk.to_vec())?.replace("\r\n", "\n"));
                    if self.buffer.len() > 1024 * 1024 {
                        return Err("SSE event too large".into());
                    }
                }
            }
        })
        .await?
    }
    async fn post_raw(&self, body: &str) -> Result<Value> {
        let r = self
            .http
            .post(format!("{}{}", self.base, self.endpoint))
            .header("content-type", "application/json")
            .body(body.to_owned())
            .send()
            .await?;
        Ok(json!({"status":r.status().as_u16(),"body":r.text().await?}))
    }
    async fn post(&self, body: Value) -> Result<Value> {
        let r = self
            .http
            .post(format!("{}{}", self.base, self.endpoint))
            .json(&body)
            .send()
            .await?;
        let status = r.status().as_u16();
        Ok(json!({"status":status,"body":r.text().await?}))
    }
    async fn request(&mut self, id: u32, method: &str, params: Value) -> Result<Value> {
        assert_eq!(
            self.post(json!({"jsonrpc":"2.0","id":id,"method":method,"params":params}))
                .await?,
            baseline()["observations"]["accepted"]
        );
        let (_, body) = self.event().await?;
        let value: Value = serde_json::from_str(&body)?;
        assert_eq!(value["id"], id);
        Ok(value)
    }
}
#[tokio::test]
async fn real_legacy_sse_matches_observed_python_status_and_payloads() -> Result<()> {
    let ct = CancellationToken::new();
    let legacy = Legacy::new(
        Registry::synthetic(),
        "/messages/",
        Duration::from_secs(30),
        ct.clone(),
    )?;
    let host = Host::start(legacy.clone().router("/sse")?, ct).await?;
    let base = format!("http://{}", host.address);
    let mut client = SseClient::connect(base.clone(), "/sse").await?;
    let observation = baseline()["observations"].clone();
    assert_eq!(
        client.endpoint.split('?').next().unwrap(),
        observation["message_prefix"]
    );
    for (key, path, body) in [
        ("missing_session", "/messages/".to_string(), "{}"),
        (
            "invalid_session",
            "/messages/?session_id=invalid".into(),
            "{}",
        ),
        (
            "unknown_session",
            "/messages/?session_id=00000000000000000000000000000000".into(),
            "{}",
        ),
        ("invalid_json", client.endpoint.clone(), "{"),
        ("invalid_rpc", client.endpoint.clone(), "{}"),
        ("oversized", client.endpoint.clone(), "oversize"),
    ] {
        let body = if key == "oversized" {
            "x".repeat(4 * 1024 * 1024 + 1)
        } else {
            body.to_string()
        };
        let r = client
            .http
            .post(format!("{base}{path}"))
            .header("content-type", "application/json")
            .body(body)
            .send()
            .await?;
        let status = r.status().as_u16();
        assert_eq!(
            json!({"status":status,"body":r.text().await?}),
            observation[key],
            "{key}"
        );
    }
    let result=client.request(1,"initialize",json!({"protocolVersion":"2025-11-25","capabilities":{},"clientInfo":{"name":"test","version":"1"}})).await?;
    assert_eq!(result["result"]["protocolVersion"], "2025-11-25");
    client
        .post(json!({"jsonrpc":"2.0","method":"notifications/initialized"}))
        .await?;
    for (id, method, params, key, inner) in [
        (2, "tools/list", json!({}), "tools", Some("tools")),
        (
            3,
            "resources/list",
            json!({}),
            "resources",
            Some("resources"),
        ),
        (4, "prompts/list", json!({}), "prompts", Some("prompts")),
        (
            5,
            "tools/call",
            json!({"name":"t01_echo","arguments":{"payload":{"items":[1,2]},"note":null}}),
            "echo_result",
            None,
        ),
        (
            6,
            "resources/read",
            json!({"uri":"t01://text"}),
            "resource_result",
            None,
        ),
        (
            7,
            "prompts/get",
            json!({"name":"t01_prompt","arguments":{"topic":"test"}}),
            "prompt_result",
            None,
        ),
    ] {
        let actual = client.request(id, method, params).await?;
        let actual = if let Some(inner) = inner {
            &actual["result"][inner]
        } else {
            &actual["result"]
        };
        compare_descriptor(&observation[key], actual)?;
    }
    let head = client.http.head(format!("{base}/sse")).send().await?;
    assert_eq!(head.status(), 200);
    assert!(head.text().await?.is_empty());
    drop(client);
    tokio::time::sleep(Duration::from_millis(50)).await;
    assert_eq!(legacy.active_sessions(), 0);
    host.shutdown().await?;
    Ok(())
}
#[tokio::test]
async fn legacy_custom_paths_suffixes_method_content_type_and_expiry() -> Result<()> {
    let ct = CancellationToken::new();
    let legacy = Legacy::new(
        Registry::synthetic(),
        "/prefix/inbox/",
        Duration::from_millis(200),
        ct.clone(),
    )?;
    let host = Host::start(legacy.clone().router("/prefix/events")?, ct).await?;
    let base = format!("http://{}", host.address);
    let client = SseClient::connect(base.clone(), "/prefix/events").await?;
    assert!(client.endpoint.starts_with("/prefix/inbox/?session_id="));
    let observation = baseline()["observations"].clone();
    for (method, content_type, key) in [
        (reqwest::Method::GET, "application/json", "wrong_method"),
        (reqwest::Method::POST, "text/plain", "wrong_content_type"),
    ] {
        let r = client
            .http
            .request(method, format!("{base}{}", client.endpoint))
            .header("content-type", content_type)
            .body("{}")
            .send()
            .await?;
        let status = r.status().as_u16();
        assert_eq!(
            json!({"status":status,"body":r.text().await?}),
            observation[key]
        );
    }
    let suffix = client
        .endpoint
        .replacen("/prefix/inbox/", "/prefix/inbox/arbitrary/suffix", 1);
    let r = client
        .http
        .post(format!("{base}{suffix}"))
        .json(&json!({"jsonrpc":"2.0","method":"notifications/initialized"}))
        .send()
        .await?;
    assert_eq!(r.status(), 202);
    tokio::time::sleep(Duration::from_millis(250)).await;
    assert_eq!(legacy.active_sessions(), 0);
    let r = client
        .http
        .post(format!("{base}{}", client.endpoint))
        .json(&json!({"jsonrpc":"2.0","method":"notifications/initialized"}))
        .send()
        .await?;
    assert_eq!(r.status(), 404);
    drop(client);
    host.shutdown().await?;
    Ok(())
}
#[tokio::test]
async fn real_http_discovery_limits_and_startup_deadlines_leave_no_ready_gateway() -> Result<()> {
    for mode in [
        ssx_mcp::Discovery::RepeatedCursor,
        ssx_mcp::Discovery::Duplicate,
        ssx_mcp::Discovery::TooManyPages,
        ssx_mcp::Discovery::TooManyItems,
        ssx_mcp::Discovery::Delayed,
    ] {
        let mut registry = Registry::synthetic();
        registry.discovery = mode;
        let ct = CancellationToken::new();
        let host = Host::start(http_router(registry, None, ct.clone()), ct).await?;
        let start = std::time::Instant::now();
        let deadline = if matches!(mode, ssx_mcp::Discovery::Delayed) {
            Duration::from_millis(50)
        } else {
            Duration::from_secs(4)
        };
        assert!(
            Gateway::connect(format!("http://{}/mcp", host.address), None, deadline)
                .await
                .is_err()
        );
        assert!(start.elapsed() < Duration::from_secs(5));
        host.shutdown().await?;
    }
    let mut registry = Registry::synthetic();
    registry.initialize_delay = Duration::from_secs(1);
    let ct = CancellationToken::new();
    let host = Host::start(http_router(registry, None, ct.clone()), ct).await?;
    assert!(
        Gateway::connect(
            format!("http://{}/mcp", host.address),
            None,
            Duration::from_millis(30)
        )
        .await
        .is_err()
    );
    host.shutdown().await?;
    Ok(())
}
#[tokio::test]
async fn real_http_request_disconnect_cancels_handler_and_releases_state() -> Result<()> {
    let registry = Registry::synthetic();
    let ct = CancellationToken::new();
    let host = Host::start(http_router(registry.clone(), None, ct.clone()), ct).await?;
    let url = format!("http://{}/mcp", host.address);
    let pending = tokio::spawn(async move {
        reqwest::Client::builder().no_proxy().pool_max_idle_per_host(0).build().unwrap().post(url).header("accept","application/json, text/event-stream").header("mcp-protocol-version","2025-11-25").json(&json!({"jsonrpc":"2.0","id":99,"method":"tools/call","params":{"name":"t01_echo","arguments":{"payload":{"items":[]},"note":"wait"}}})).send().await
    });
    tokio::time::timeout(Duration::from_secs(2), async {
        while registry.in_flight.load(Ordering::SeqCst) == 0 {
            tokio::time::sleep(Duration::from_millis(5)).await;
        }
    })
    .await?;
    pending.abort();
    let _ = pending.await;
    tokio::time::timeout(Duration::from_secs(2), async {
        while registry.in_flight.load(Ordering::SeqCst) != 0 {
            tokio::time::sleep(Duration::from_millis(5)).await;
        }
    })
    .await?;
    host.shutdown().await?;
    Ok(())
}
#[tokio::test]
async fn legacy_unsupported_version_negotiates_and_jsonrpc_errors_remain_errors() -> Result<()> {
    let ct = CancellationToken::new();
    let legacy = Legacy::new(
        Registry::synthetic(),
        "/messages/",
        Duration::from_secs(10),
        ct.clone(),
    )?;
    let host = Host::start(legacy.clone().router("/sse")?, ct).await?;
    let mut client = SseClient::connect(format!("http://{}", host.address), "/sse").await?;
    let response=client.request(1,"initialize",json!({"protocolVersion":"1900-01-01","capabilities":{},"clientInfo":{"name":"test","version":"1"}})).await?;
    assert_eq!(response["result"]["protocolVersion"], "2025-11-25");
    client
        .post(json!({"jsonrpc":"2.0","method":"notifications/initialized"}))
        .await?;
    let error = client.request(2, "not-a-method", json!({})).await?;
    assert!(error.get("error").is_some());
    assert!(error.get("result").is_none());
    drop(client);
    host.shutdown().await?;
    Ok(())
}
#[tokio::test]
async fn complete_captured_public_catalog_roundtrips_through_pinned_sdk_and_http() -> Result<()> {
    let observed = baseline()["observations"].clone();
    assert_eq!(observed["public_tools"].as_array().unwrap().len(), 38);
    let mut registry = Registry::synthetic();
    registry.descriptors = std::sync::Arc::new(
        json!({"tools":observed["public_tools"],"resources":observed["public_resources"],"prompts":observed["public_prompts"]}),
    );
    let ct = CancellationToken::new();
    let host = Host::start(http_router(registry, None, ct.clone()), ct).await?;
    let gateway = Gateway::connect(
        format!("http://{}/mcp", host.address),
        None,
        Duration::from_secs(3),
    )
    .await?;
    for key in ["tools", "resources", "prompts"] {
        compare_descriptor(&observed[format!("public_{key}")], &gateway.catalog[key])?;
    }
    gateway.shutdown().await?;
    host.shutdown().await?;
    Ok(())
}
#[test]
fn unknown_schema_metadata_objects_and_refs_are_preserved_without_numeric_reinterpretation()
-> Result<()> {
    let mut descriptor = baseline()["observations"]["tools"][0].clone();
    descriptor["inputSchema"]["$defs"] = json!({"Value":{"type":"object","properties":{"$serde_json::private::Number":{"type":"string"}}}});
    descriptor["inputSchema"]["properties"]["vendor"] =
        json!({"$ref":"#/$defs/Value","x-vendor":{"$serde_json::private::Number":"123"}});
    descriptor["_meta"]["vendor"] =
        json!({"$serde_json::private::Number":"123","null":null,"empty":{}});
    let typed: Tool = serde_json::from_value(descriptor.clone())?;
    compare_descriptor(&descriptor, &serde_json::to_value(typed)?)?;
    Ok(())
}
#[tokio::test]
async fn legacy_request_cancellation_releases_inflight_work_and_sessions() -> Result<()> {
    let registry = Registry::synthetic();
    let ct = CancellationToken::new();
    let legacy = Legacy::new(
        registry.clone(),
        "/messages/",
        Duration::from_secs(10),
        ct.clone(),
    )?;
    let host = Host::start(legacy.clone().router("/sse")?, ct).await?;
    let mut client = SseClient::connect(format!("http://{}", host.address), "/sse").await?;
    client.request(1,"initialize",json!({"protocolVersion":"2025-11-25","capabilities":{},"clientInfo":{"name":"test","version":"1"}})).await?;
    client
        .post(json!({"jsonrpc":"2.0","method":"notifications/initialized"}))
        .await?;
    assert_eq!(client.post_raw(r#"{"jsonrpc":"2.0","id":18446744073709551617,"method":"tools/call","params":{"name":"t01_echo","arguments":{"payload":{"items":[]},"note":"wait"}}}"#).await?,baseline()["observations"]["accepted"]);
    tokio::time::timeout(Duration::from_secs(2), async {
        while registry.in_flight.load(Ordering::SeqCst) == 0 {
            tokio::time::sleep(Duration::from_millis(5)).await;
        }
    })
    .await?;
    assert_eq!(client.post_raw(r#"{"jsonrpc":"2.0","method":"notifications/cancelled","params":{"requestId":18446744073709551617,"reason":"test"}}"#).await?,baseline()["observations"]["accepted"]);
    tokio::time::timeout(Duration::from_secs(2), async {
        while registry.in_flight.load(Ordering::SeqCst) != 0 {
            tokio::time::sleep(Duration::from_millis(5)).await;
        }
    })
    .await?;
    drop(client);
    host.shutdown().await?;
    assert_eq!(legacy.active_sessions(), 0);
    Ok(())
}
fn raw_large_tools() -> Box<serde_json::value::RawValue> {
    let root: std::collections::BTreeMap<String, Box<serde_json::value::RawValue>> =
        serde_json::from_str(include_str!("../fixtures/python-sse.json")).unwrap();
    let mut observations: std::collections::BTreeMap<String, Box<serde_json::value::RawValue>> =
        serde_json::from_str(root["observations"].get()).unwrap();
    observations.remove("large_tools").unwrap()
}
fn raw_tools(response: &str) -> Result<String> {
    let root: std::collections::BTreeMap<String, Box<serde_json::value::RawValue>> =
        serde_json::from_str(response)?;
    let result: std::collections::BTreeMap<String, Box<serde_json::value::RawValue>> =
        serde_json::from_str(root["result"].get())?;
    let tools: Vec<Box<serde_json::value::RawValue>> = serde_json::from_str(result["tools"].get())?;
    Ok(serde_json::to_string(&tools)?)
}
fn assert_large_catalog(response: &str) -> Result<()> {
    let expected: Vec<Box<serde_json::value::RawValue>> =
        serde_json::from_str(raw_large_tools().get())?;
    let actual = raw_tools(response)?;
    assert_eq!(
        ssx_mcp::opaque::compact(&actual)?,
        ssx_mcp::opaque::compact(&serde_json::to_string(&expected)?)?
    );
    assert!(actual.contains("18446744073709551617"));
    assert!(actual.contains("$serde_json::private::Number"));
    Ok(())
}
#[test]
fn typed_sdk_large_number_limitation_is_characterized_and_not_mistaken_for_parity() -> Result<()> {
    let raw = raw_large_tools();
    let typed: Vec<Tool> = serde_json::from_str(raw.get())?;
    let rounded = serde_json::to_string(&typed)?;
    assert!(
        !rounded.contains("18446744073709551617"),
        "SDK changed; requalify the opaque bridge"
    );
    assert!(rounded.contains("$serde_json::private::Number"));
    assert!(raw.get().contains("18446744073709551617"));
    Ok(())
}
async fn raw_process_catalog(args: &[&str]) -> Result<String> {
    use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
    let mut child = tokio::process::Command::new(env!("CARGO_BIN_EXE_mcp-qualification"))
        .args(args)
        .env_clear()
        .env("PATH", "")
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::inherit())
        .kill_on_drop(true)
        .spawn()?;
    let mut input = child.stdin.take().ok_or("child stdin")?;
    let mut output = BufReader::new(child.stdout.take().ok_or("child stdout")?).lines();
    input.write_all(b"{\"jsonrpc\":\"2.0\",\"id\":1,\"method\":\"initialize\",\"params\":{\"protocolVersion\":\"2025-11-25\",\"capabilities\":{},\"clientInfo\":{\"name\":\"raw-client\",\"version\":\"1\"}}}\n").await?;
    let init = tokio::time::timeout(Duration::from_secs(3), output.next_line())
        .await??
        .ok_or("initialize missing")?;
    assert!(init.contains("result"));
    input.write_all(b"{\"jsonrpc\":\"2.0\",\"method\":\"notifications/initialized\"}\n{\"jsonrpc\":\"2.0\",\"id\":2,\"method\":\"tools/list\",\"params\":{}}\n").await?;
    let response = tokio::time::timeout(Duration::from_secs(3), output.next_line())
        .await??
        .ok_or("catalog missing")?;
    input.shutdown().await?;
    drop(input);
    assert!(
        tokio::time::timeout(Duration::from_secs(3), child.wait())
            .await??
            .success()
    );
    Ok(response)
}
#[tokio::test]
async fn opaque_large_integer_and_reserved_object_survive_real_http_stdio_sse_and_gateway()
-> Result<()> {
    let registry = Registry::large_catalog();
    let ct = CancellationToken::new();
    let host = Host::start(http_router(registry.clone(), None, ct.clone()), ct).await?;
    let url = format!("http://{}/mcp", host.address);
    let response = reqwest::Client::builder()
        .no_proxy()
        .build()?
        .post(&url)
        .header("accept", "application/json, text/event-stream")
        .json(&json!({"jsonrpc":"2.0","id":2,"method":"tools/list"}))
        .send()
        .await?
        .text()
        .await?;
    assert_large_catalog(&response)?;
    assert_large_catalog(&raw_process_catalog(&["stdio-opaque"]).await?)?;
    assert_large_catalog(&raw_process_catalog(&["gateway", &url]).await?)?;
    host.shutdown().await?;
    let ct = CancellationToken::new();
    let legacy = Legacy::new(registry, "/messages/", Duration::from_secs(10), ct.clone())?;
    let host = Host::start(legacy.clone().router("/sse")?, ct).await?;
    let mut client = SseClient::connect(format!("http://{}", host.address), "/sse").await?;
    client.request(1,"initialize",json!({"protocolVersion":"2025-11-25","capabilities":{},"clientInfo":{"name":"raw-client","version":"1"}})).await?;
    client
        .post(json!({"jsonrpc":"2.0","method":"notifications/initialized"}))
        .await?;
    client
        .post(json!({"jsonrpc":"2.0","id":2,"method":"tools/list","params":{}}))
        .await?;
    let (_, response) = client.event().await?;
    assert_large_catalog(&response)?;
    drop(client);
    host.shutdown().await?;
    Ok(())
}
#[tokio::test]
async fn selected_raw_gateway_owns_stateful_session_and_closes_it() -> Result<()> {
    let ct = CancellationToken::new();
    let deletes = std::sync::Arc::new(std::sync::atomic::AtomicUsize::new(0));
    let observed = deletes.clone();
    let router =
        ssx_mcp::harness::http_router_mode(Registry::large_catalog(), None, ct.clone(), true)
            .layer(axum::middleware::from_fn(
                move |request: axum::http::Request<axum::body::Body>,
                      next: axum::middleware::Next| {
                    let observed = observed.clone();
                    async move {
                        if request.method() == axum::http::Method::DELETE
                            && request.headers().contains_key("mcp-session-id")
                        {
                            observed.fetch_add(1, Ordering::SeqCst);
                        }
                        next.run(request).await
                    }
                },
            ));
    let host = Host::start(router, ct).await?;
    let url = format!("http://{}/mcp", host.address);
    assert_large_catalog(&raw_process_catalog(&["gateway", &url]).await?)?;
    assert_eq!(deletes.load(Ordering::SeqCst), 1);
    host.shutdown().await?;
    Ok(())
}
#[tokio::test]
async fn selected_raw_gateway_request_cancellation_reaches_remote_handler() -> Result<()> {
    use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
    let registry = Registry::synthetic();
    let ct = CancellationToken::new();
    let host = Host::start(
        ssx_mcp::harness::http_router_mode(registry.clone(), None, ct.clone(), true),
        ct,
    )
    .await?;
    let mut child = tokio::process::Command::new(env!("CARGO_BIN_EXE_mcp-qualification"))
        .args(["gateway", &format!("http://{}/mcp", host.address)])
        .env_clear()
        .env("PATH", "")
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::piped())
        .kill_on_drop(true)
        .spawn()?;
    let mut input = child.stdin.take().unwrap();
    let mut output = BufReader::new(child.stdout.take().unwrap()).lines();
    input.write_all(b"{\"jsonrpc\":\"2.0\",\"id\":1,\"method\":\"initialize\",\"params\":{\"protocolVersion\":\"2025-11-25\",\"capabilities\":{},\"clientInfo\":{\"name\":\"client\",\"version\":\"1\"}}}\n").await?;
    tokio::time::timeout(Duration::from_secs(3), output.next_line())
        .await??
        .ok_or("initialize missing")?;
    input.write_all(b"{\"jsonrpc\":\"2.0\",\"method\":\"notifications/initialized\"}\n{\"jsonrpc\":\"2.0\",\"id\":18446744073709551617,\"method\":\"tools/call\",\"params\":{\"name\":\"t01_echo\",\"arguments\":{\"payload\":{\"items\":[]},\"note\":\"wait\"}}}\n").await?;
    tokio::time::timeout(Duration::from_secs(2), async {
        while registry.in_flight.load(Ordering::SeqCst) == 0 {
            tokio::time::sleep(Duration::from_millis(5)).await;
        }
    })
    .await?;
    input.write_all(b"{\"jsonrpc\":\"2.0\",\"method\":\"notifications/cancelled\",\"params\":{\"requestId\":18446744073709551617}}\n").await?;
    tokio::time::timeout(Duration::from_secs(2), async {
        while registry.in_flight.load(Ordering::SeqCst) != 0 {
            tokio::time::sleep(Duration::from_millis(5)).await;
        }
    })
    .await?;
    input.shutdown().await?;
    drop(input);
    assert!(
        tokio::time::timeout(Duration::from_secs(3), child.wait())
            .await??
            .success()
    );
    host.shutdown().await?;
    Ok(())
}
#[tokio::test]
async fn selected_raw_gateway_accepts_incremental_sse_and_rejects_bad_remote_version() -> Result<()>
{
    use axum::{response::IntoResponse, routing::post};
    for bad in [false, true] {
        let router=axum::Router::new().route("/mcp",post(move |body:String|async move {
            let request:Value=serde_json::from_str(&body).unwrap();let method=request["method"].as_str().unwrap_or("");
            if request.get("id").is_none(){return axum::http::StatusCode::ACCEPTED.into_response();}
            let result=match method {"initialize"=>json!({"protocolVersion":if bad{"2099-01-01"}else{"2025-11-25"},"capabilities":{"tools":{},"resources":{},"prompts":{}},"serverInfo":{"name":"fixture","version":"1"}}),"tools/list"=>json!({"tools":[]}),"resources/list"=>json!({"resources":[]}),"prompts/list"=>json!({"prompts":[]}),_=>json!({"resourceTemplates":[]})};
            let response=json!({"jsonrpc":"2.0","id":request["id"],"result":result}).to_string();
            let frames=vec![Ok::<_,std::convert::Infallible>(axum::response::sse::Event::default().event("message").data(r#"{"jsonrpc":"2.0","method":"notifications/progress","params":{}}"#)),Ok(axum::response::sse::Event::default().event("message").data(response))];
            axum::response::Sse::new(futures_util::stream::iter(frames)).into_response()
        }));
        let ct = CancellationToken::new();
        let host = Host::start(router, ct).await?;
        let result = ssx_mcp::opaque::RawGateway::connect(
            format!("http://{}/mcp", host.address),
            None,
            Duration::from_secs(2),
        )
        .await;
        if bad {
            assert!(result.is_err());
        } else {
            let gateway = result?;
            assert_eq!(gateway.catalogs.len(), 4);
            gateway.close().await?;
        }
        host.shutdown().await?;
    }
    Ok(())
}
#[tokio::test]
async fn selected_raw_gateway_real_discovery_limits_and_deadline_fail_closed() -> Result<()> {
    for mode in [
        ssx_mcp::Discovery::RepeatedCursor,
        ssx_mcp::Discovery::Duplicate,
        ssx_mcp::Discovery::TooManyPages,
        ssx_mcp::Discovery::TooManyItems,
        ssx_mcp::Discovery::Delayed,
    ] {
        let mut registry = Registry::synthetic();
        registry.discovery = mode;
        let ct = CancellationToken::new();
        let host = Host::start(http_router(registry, None, ct.clone()), ct).await?;
        let deadline = if matches!(mode, ssx_mcp::Discovery::Delayed) {
            Duration::from_millis(50)
        } else {
            Duration::from_secs(4)
        };
        assert!(
            ssx_mcp::opaque::RawGateway::connect(
                format!("http://{}/mcp", host.address),
                None,
                deadline
            )
            .await
            .is_err()
        );
        host.shutdown().await?;
    }
    Ok(())
}

#[tokio::test]
async fn selected_stdio_source_lifecycle_and_adjacent_big_ids() -> Result<()> {
    use tokio::io::AsyncWriteExt;
    let ct = CancellationToken::new();
    let host = Host::start(http_router(Registry::large_catalog(), None, ct.clone()), ct).await?;
    let url = format!("http://{}/mcp", host.address);
    for args in [vec!["stdio-opaque"], vec!["gateway", url.as_str()]] {
        let mut child = tokio::process::Command::new(env!("CARGO_BIN_EXE_mcp-qualification"))
            .args(args)
            .env_clear()
            .kill_on_drop(true)
            .stdin(std::process::Stdio::piped())
            .stdout(std::process::Stdio::piped())
            .spawn()?;
        let mut input = child.stdin.take().unwrap();
        let mut output = ssx_mcp::opaque::Frames::new(child.stdout.take().unwrap());
        let cases = [
            r#"{"jsonrpc":"2.0","id":100,"method":"tools/list"}"#,
            r#"{"jsonrpc":"2.0","id":101,"method":"initialize","params":{}}"#,
            r#"{"jsonrpc":"2.0","id":102,"method":"initialize","params":{"protocolVersion":"unsupported","capabilities":{},"clientInfo":{"name":"test","version":"1"}}}"#,
            r#"{"jsonrpc":"2.0","id":18446744073709551616,"method":"tools/list"}"#,
            r#"{"jsonrpc":"2.0","id":18446744073709551617,"method":"tools/list"}"#,
            r#"{"jsonrpc":"2.0","id":104,"method":"initialize","params":{"protocolVersion":"2024-11-05","capabilities":{},"clientInfo":{"name":"test","version":"1"}}}"#,
        ];
        for (index, request) in cases.iter().enumerate() {
            input.write_all(request.as_bytes()).await?;
            input.write_all(b"\n").await?;
            let raw = tokio::time::timeout(Duration::from_secs(3), output.next())
                .await??
                .ok_or("stdio ended")?;
            let response: Value = serde_json::from_str(&raw)?;
            match index {
                0 | 1 => assert_eq!(
                    response,
                    baseline()["observations"]["lifecycle"][if index == 0 {
                        "before_initialize"
                    } else {
                        "malformed_initialize"
                    }]
                ),
                2 => assert_eq!(response["result"]["protocolVersion"], "2025-11-25"),
                3 | 4 => {
                    let fields: std::collections::BTreeMap<
                        String,
                        Box<serde_json::value::RawValue>,
                    > = serde_json::from_str(&raw)?;
                    assert_eq!(
                        fields["id"].get(),
                        if index == 3 {
                            "18446744073709551616"
                        } else {
                            "18446744073709551617"
                        }
                    );
                    assert!(raw.contains("18446744073709551617"));
                    assert!(response.get("error").is_none());
                }
                5 => assert_eq!(response["result"]["protocolVersion"], "2024-11-05"),
                _ => unreachable!(),
            }
        }
        drop(input);
        tokio::time::timeout(Duration::from_secs(3), child.wait()).await??;
    }
    host.shutdown().await?;
    Ok(())
}

#[tokio::test]
async fn legacy_source_lifecycle_matches_captured_phase_errors() -> Result<()> {
    let ct = CancellationToken::new();
    let legacy = Legacy::new(
        Registry::synthetic(),
        "/messages/",
        Duration::from_secs(10),
        ct.clone(),
    )?;
    let host = Host::start(legacy.router("/sse")?, ct).await?;
    let mut client = SseClient::connect(format!("http://{}", host.address), "/sse").await?;
    for (id, method, params, expected) in [
        (100, "tools/list", json!({}), "before_initialize"),
        (101, "initialize", json!({}), "malformed_initialize"),
    ] {
        assert_eq!(
            client.request(id, method, params).await?,
            baseline()["observations"]["lifecycle"][expected]
        );
    }
    let init = client.request(102, "initialize", json!({"protocolVersion":"unsupported","capabilities":{},"clientInfo":{"name":"test","version":"1"}})).await?;
    assert_eq!(init["result"]["protocolVersion"], "2025-11-25");
    assert!(client.request(103, "tools/list", json!({})).await?["result"]["tools"].is_array());
    let init = client.request(104, "initialize", json!({"protocolVersion":"2024-11-05","capabilities":{},"clientInfo":{"name":"test","version":"1"}})).await?;
    assert_eq!(init["result"]["protocolVersion"], "2024-11-05");
    drop(client);
    host.shutdown().await?;
    Ok(())
}

#[tokio::test]
async fn real_http_and_legacy_preserve_adjacent_large_request_ids() -> Result<()> {
    let ct = CancellationToken::new();
    let host = Host::start(http_router(Registry::large_catalog(), None, ct.clone()), ct).await?;
    let http = reqwest::Client::new();
    for id in ["18446744073709551616", "18446744073709551617"] {
        for (method, params) in [
            (
                "initialize",
                r#"{"protocolVersion":"2025-11-25","capabilities":{},"clientInfo":{"name":"test","version":"1"}}"#,
            ),
            ("tools/list", "{}"),
            (
                "tools/call",
                r#"{"name":"t01_echo","arguments":{"payload":{"items":[]}}}"#,
            ),
        ] {
            let response = http.post(format!("http://{}/mcp",host.address)).header("accept","application/json, text/event-stream").header("content-type","application/json")
                .body(format!("{{\"jsonrpc\":\"2.0\",\"id\":{id},\"method\":\"{method}\",\"params\":{params}}}")).send().await?;
            assert_eq!(response.status(), 200);
            let fields: std::collections::BTreeMap<String, Box<serde_json::value::RawValue>> =
                serde_json::from_str(&response.text().await?)?;
            assert_eq!(fields["id"].get(), id);
            assert!(!fields.contains_key("error"));
        }
    }
    host.shutdown().await?;
    let ct = CancellationToken::new();
    let legacy = Legacy::new(
        Registry::large_catalog(),
        "/messages/",
        Duration::from_secs(10),
        ct.clone(),
    )?;
    let host = Host::start(legacy.router("/sse")?, ct).await?;
    let mut client = SseClient::connect(format!("http://{}", host.address), "/sse").await?;
    for id in ["18446744073709551616", "18446744073709551617"] {
        for (method, params) in [
            (
                "initialize",
                r#"{"protocolVersion":"2025-11-25","capabilities":{},"clientInfo":{"name":"test","version":"1"}}"#,
            ),
            ("tools/list", "{}"),
            (
                "tools/call",
                r#"{"name":"t01_echo","arguments":{"payload":{"items":[]}}}"#,
            ),
        ] {
            let response=client.http.post(format!("{}{}",client.base,client.endpoint)).header("content-type","application/json").body(format!("{{\"jsonrpc\":\"2.0\",\"id\":{id},\"method\":\"{method}\",\"params\":{params}}}")).send().await?;
            assert_eq!(response.status(), 202);
            let (_, raw) = client.event().await?;
            let fields: std::collections::BTreeMap<String, Box<serde_json::value::RawValue>> =
                serde_json::from_str(&raw)?;
            assert_eq!(fields["id"].get(), id);
            assert!(!fields.contains_key("error"));
        }
    }
    drop(client);
    host.shutdown().await?;
    Ok(())
}

#[tokio::test]
async fn invalid_http_session_flood_does_not_consume_accepted_session_capacity() -> Result<()> {
    let ct = CancellationToken::new();
    let host = Host::start(
        ssx_mcp::harness::http_router_mode(Registry::synthetic(), None, ct.clone(), true),
        ct,
    )
    .await?;
    let url = format!("http://{}/mcp", host.address);
    let http = reqwest::Client::new();
    for n in 0..160 {
        let response = http
            .post(&url)
            .header("mcp-session-id", format!("invalid-{n}"))
            .header("accept", "application/json, text/event-stream")
            .json(&json!({"jsonrpc":"2.0","id":1,"method":"tools/list"}))
            .send()
            .await?;
        assert!(!response.status().is_success());
        assert_ne!(response.status(), 503);
    }
    let gateway = ssx_mcp::opaque::RawGateway::connect(url, None, Duration::from_secs(3)).await?;
    assert!(gateway.catalogs.contains_key("tools/list"));
    gateway.close().await?;
    host.shutdown().await?;
    Ok(())
}
