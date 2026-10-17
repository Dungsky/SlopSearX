//! Actual native event service through loopback rmcp client and private Valkey.
mod w05_support;
use rmcp::{
    ServiceExt,
    model::*,
    transport::{
        StreamableHttpClientTransport, streamable_http_client::StreamableHttpClientTransportConfig,
    },
};
use serde_json::{Value, json};
use ssx_application::{
    NoopSink, PortFuture, SearchEngine,
    cache::RoutingIdentity,
    policy::{PolicyService, SearchPolicy},
    saved::SavedLimits,
    scope::{CatalogSnapshot, EngineCapability, ScopeResolver},
};
use ssx_domain::{AdapterResponse, SearchRequest, SearchResult, TenantId};
use ssx_infra::{
    saved_events::{observations::NoopEventObserver, *},
    saved_runner::StaticSavedAuthority,
    search::*,
    valkey::saved::{SavedStore, events::EventLimits},
};
use ssx_mcp::saved_events::{SavedEventRegistry, http_router};
use std::{
    collections::BTreeMap,
    sync::{
        Arc,
        atomic::{AtomicUsize, Ordering},
    },
    time::Duration,
};
use tokio_util::sync::CancellationToken;
#[path = "w05_support/search.rs"]
mod search_support;
struct FixtureEngine(AtomicUsize);
impl SearchEngine for FixtureEngine {
    fn search<'a>(&'a self, _: &'a SearchRequest) -> PortFuture<'a, AdapterResponse> {
        Box::pin(async move {
            self.0.fetch_add(1, Ordering::SeqCst);
            AdapterResponse {
                results: vec![SearchResult {
                    url: "https://source.test/release".into(),
                    engine: "brave".into(),
                    title: "fixture release".into(),
                    ..Default::default()
                }],
                ..Default::default()
            }
        })
    }
}
async fn call<S: rmcp::Service<rmcp::RoleClient>>(
    client: &rmcp::service::RunningService<rmcp::RoleClient, S>,
    name: &str,
    args: Value,
) -> Value {
    let response = client
        .call_tool(
            CallToolRequestParams::new(name.to_owned())
                .with_arguments(args.as_object().unwrap().clone()),
        )
        .await
        .unwrap();
    let value = serde_json::to_value(response).unwrap();
    assert_ne!(value["isError"], json!(true));
    let parsed: Value =
        serde_json::from_str(value["content"][0]["text"].as_str().unwrap()).unwrap();
    assert_eq!(
        value["structuredContent"], parsed,
        "source dict tool result must retain structuredContent"
    );
    parsed
}
#[tokio::test]
async fn actual_native_transport_create_wait_repeat_ack_restart_resume_and_grant() {
    for granted in [true, false] {
        let server = w05_support::Server::start();
        let engine = Arc::new(FixtureEngine(0.into()));
        let deps = search_support::dependencies(vec![("brave", engine.clone())]);
        let policy = deps.policy.clone();
        let search = SearchService::new(SearchConfig::default(), deps).unwrap();
        let store = SavedStore::new(
            Arc::new(server.store()),
            TenantId::new("tenant:wire").unwrap(),
        );
        let authority = Arc::new(StaticSavedAuthority {
            search: policy.clone(),
            saved_granted: true,
            auth_configured: BTreeMap::from([("brave".into(), false)]),
            limits: SavedLimits::default(),
        });
        let publisher = EventPublisher {
            limits: EventLimits::default(),
            authority: Arc::new(StaticEventAuthority {
                search: policy,
                events_granted: granted,
                saved_granted: true,
            }),
            observer: Arc::new(NoopEventObserver),
        };
        let principal =
            EventPrincipal::new(store.tenant().clone(), "trusted-fixture-subject".into()).unwrap();
        let events = EventService::new(store.clone(), &principal, publisher);
        let registry =
            SavedEventRegistry::new(events, search.clone(), authority, Duration::from_secs(2))
                .unwrap();
        let cancel = CancellationToken::new();
        let router = http_router(registry.clone(), cancel.clone());
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = listener.local_addr().unwrap();
        assert!(address.ip().is_loopback());
        let url = format!("http://{address}/mcp");
        let parsed = url::Url::parse(&url).unwrap();
        assert_eq!(parsed.scheme(), "http");
        assert_eq!(parsed.host_str(), Some("127.0.0.1"));
        assert_eq!(parsed.port(), Some(address.port()));
        let shutdown = cancel.clone();
        let host = tokio::spawn(async move {
            axum::serve(listener, router)
                .with_graceful_shutdown(shutdown.cancelled_owned())
                .await
                .unwrap();
        });
        // no_proxy prevents inherited proxy settings from routing this fixture.
        let http = reqwest::Client::builder()
            .no_proxy()
            .redirect(reqwest::redirect::Policy::none())
            .build()
            .unwrap();
        let connect = || {
            let transport = StreamableHttpClientTransport::with_client(
                http.clone(),
                StreamableHttpClientTransportConfig::with_uri(url.clone()),
            );
            async move {
                ClientConfig::default()
                    .with_protocol_version(ProtocolVersion::V_2025_11_25)
                    .serve(transport)
                    .await
                    .unwrap()
            }
        };
        let client = connect().await;
        let tools = client.list_tools(None).await.unwrap().tools;
        for name in [
            "slopsearx_create_saved_search",
            "slopsearx_read_saved_search_events",
            "slopsearx_ack_saved_search_events",
        ] {
            assert!(tools.iter().any(|t| t.name == name));
        }
        let captured: Value =
            serde_json::from_str(include_str!("../fixtures/python-sse.json")).unwrap();
        for tool in &tools {
            let expected = captured["observations"]["public_tools"]
                .as_array()
                .unwrap()
                .iter()
                .find(|v| v["name"] == tool.name.as_ref())
                .unwrap();
            assert_eq!(
                &serde_json::to_value(tool).unwrap(),
                expected,
                "complete source descriptor drift"
            );
        }
        for tool in tools.iter().filter(|t| t.name.contains("_events")) {
            let value = serde_json::to_value(tool).unwrap();
            for name in ["url", "callback", "callback_url", "webhook"] {
                assert!(value["inputSchema"]["properties"].get(name).is_none());
            }
        }
        let created=call(&client,"slopsearx_create_saved_search",json!({"query":"release changes","engines":["brave"],"interval_seconds":60,"start_immediately":true})).await;
        assert!(created["error"].is_null());
        let until = tokio::time::Instant::now() + Duration::from_secs(3);
        let batch = loop {
            let value = call(
                &client,
                "slopsearx_read_saved_search_events",
                json!({"consumer_id":"wire-agent","limit":1}),
            )
            .await;
            if !granted || value["returned"] == 1 {
                break value;
            }
            assert!(
                tokio::time::Instant::now() < until,
                "native event was never published"
            );
            tokio::time::sleep(Duration::from_millis(10)).await;
        };
        if granted {
            assert_eq!(batch["events"][0]["saved_search_id"], created["search_id"]);
            assert_eq!(batch["acknowledged_cursor"], "0-0");
            let repeated = call(
                &client,
                "slopsearx_read_saved_search_events",
                json!({"consumer_id":"wire-agent","limit":1}),
            )
            .await;
            assert_eq!(repeated, batch);
            let cursor = batch["next_cursor"].as_str().unwrap();
            let ack = call(
                &client,
                "slopsearx_ack_saved_search_events",
                json!({"consumer_id":"wire-agent","cursor":cursor}),
            )
            .await;
            assert_eq!(ack["acknowledged_cursor"], cursor);
            assert_eq!(
                ack,
                call(
                    &client,
                    "slopsearx_ack_saved_search_events",
                    json!({"consumer_id":"wire-agent","cursor":cursor})
                )
                .await
            );
            client.cancel().await.unwrap();
            let restarted = connect().await;
            let resumed = call(
                &restarted,
                "slopsearx_read_saved_search_events",
                json!({"consumer_id":"wire-agent"}),
            )
            .await;
            assert_eq!(resumed["acknowledged_cursor"], cursor);
            assert_eq!(resumed["events"], json!([]));
            assert!(
                restarted
                    .call_tool(
                        CallToolRequestParams::new("slopsearx_read_saved_search_events")
                            .with_arguments(
                            json!({"consumer_id":"agent","callback_url":"https://forbidden.test"})
                                .as_object()
                                .unwrap()
                                .clone()
                        )
                    )
                    .await
                    .is_err()
            );
            // A pre-v2 ambiguous legacy acknowledgement is quarantined on
            // the actual tool boundary, then recovered by explicit replay/ack.
            let mut connection = redis::Client::open(server.url.as_str())
                .unwrap()
                .get_connection()
                .unwrap();
            let old_key =
                ssx_infra::valkey::keys::saved("event-ack", store.tenant(), Some("recovery-agent"))
                    .unwrap();
            let _: () = redis::cmd("SET")
                .arg(&old_key)
                .arg(cursor)
                .arg("EX")
                .arg(300)
                .query(&mut connection)
                .unwrap();
            let quarantined = call(
                &restarted,
                "slopsearx_read_saved_search_events",
                json!({"consumer_id":"recovery-agent"}),
            )
            .await;
            assert_eq!(quarantined["error"]["code"], "cursor_conflict");
            let replay = call(
                &restarted,
                "slopsearx_read_saved_search_events",
                json!({"consumer_id":"recovery-agent","cursor":"0-0"}),
            )
            .await;
            assert_eq!(replay["acknowledged_cursor"], "0-0");
            assert_eq!(replay["returned"], 1);
            assert_eq!(
                replay["acknowledgement_recovery"]["state"],
                "legacy_ambiguous_ignored"
            );
            assert_eq!(
                call(
                    &restarted,
                    "slopsearx_ack_saved_search_events",
                    json!({"consumer_id":"recovery-agent","cursor":cursor})
                )
                .await["acknowledged_cursor"],
                cursor
            );
            assert_eq!(
                call(
                    &restarted,
                    "slopsearx_read_saved_search_events",
                    json!({"consumer_id":"recovery-agent"})
                )
                .await["events"],
                json!([])
            );
            let preserved: String = redis::cmd("GET")
                .arg(&old_key)
                .query(&mut connection)
                .unwrap();
            assert_eq!(preserved, cursor);
            restarted.cancel().await.unwrap();
        } else {
            assert_eq!(
                batch["error"],
                json!({"code":"tool_disabled","message":"saved-search events require the event grant (MCP_GRANT_SAVED_SEARCH_EVENTS=1)"})
            );
            let ack = call(
                &client,
                "slopsearx_ack_saved_search_events",
                json!({"consumer_id":"wire-agent","cursor":"0-0"}),
            )
            .await;
            assert_eq!(ack["error"]["code"], "tool_disabled");
            client.cancel().await.unwrap();
        }
        if !granted {
            let id =
                ssx_domain::SavedSearchId::new(created["search_id"].as_str().unwrap()).unwrap();
            let until = tokio::time::Instant::now() + Duration::from_secs(3);
            while store
                .load(&id)
                .await
                .unwrap()
                .unwrap()
                .latest_run_id
                .is_none()
            {
                assert!(
                    tokio::time::Instant::now() < until,
                    "ordinary report never published with event grant disabled"
                );
                tokio::time::sleep(Duration::from_millis(10)).await;
            }
        }
        registry.close().await;
        assert_eq!(engine.0.load(Ordering::SeqCst), 1);
        search.shutdown().await;
        cancel.cancel();
        tokio::time::timeout(Duration::from_secs(3), host)
            .await
            .unwrap()
            .unwrap();
        assert!(tokio::net::TcpStream::connect(address).await.is_err());
        drop(registry);
        drop(store);
        drop(server);
        if granted {
            println!("W05_CASE_COMPLETE:actual-mcp-legacy-quarantine-explicit-recovery");
        }
        println!(
            "W05_CASE_COMPLETE:{}",
            if granted {
                "actual-mcp-native-create-read-repeat-ack-restart-resume"
            } else {
                "actual-mcp-separate-event-grant-denied"
            }
        );
    }
}
#[tokio::test]
async fn actual_registry_close_fences_admitted_create_and_rejects_late_mutation() {
    let server = w05_support::Server::start();
    let engine = Arc::new(FixtureEngine(0.into()));
    let deps = search_support::dependencies(vec![("brave", engine.clone())]);
    let policy = deps.policy.clone();
    let search = SearchService::new(SearchConfig::default(), deps).unwrap();
    let store = SavedStore::new(
        Arc::new(server.store()),
        TenantId::new("close-fixture").unwrap(),
    );
    let authority = Arc::new(StaticSavedAuthority {
        search: policy.clone(),
        saved_granted: true,
        auth_configured: BTreeMap::from([("brave".into(), false)]),
        limits: SavedLimits::default(),
    });
    let publisher = EventPublisher {
        limits: EventLimits::default(),
        authority: Arc::new(StaticEventAuthority {
            search: policy,
            events_granted: true,
            saved_granted: true,
        }),
        observer: Arc::new(NoopEventObserver),
    };
    let principal =
        EventPrincipal::new(store.tenant().clone(), "trusted-fixture-subject".into()).unwrap();
    let registry = SavedEventRegistry::new(
        EventService::new(store.clone(), &principal, publisher),
        search.clone(),
        authority,
        Duration::from_secs(2),
    )
    .unwrap();
    let cancel = CancellationToken::new();
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap();
    assert!(address.ip().is_loopback());
    let url = format!("http://{address}/mcp");
    let parsed = url::Url::parse(&url).unwrap();
    assert_eq!(parsed.host_str(), Some("127.0.0.1"));
    let shutdown = cancel.clone();
    let router = http_router(registry.clone(), cancel.clone());
    let host = tokio::spawn(async move {
        axum::serve(listener, router)
            .with_graceful_shutdown(shutdown.cancelled_owned())
            .await
            .unwrap();
    });
    let http = reqwest::Client::builder()
        .no_proxy()
        .redirect(reqwest::redirect::Policy::none())
        .build()
        .unwrap();
    let transport = StreamableHttpClientTransport::with_client(
        http,
        StreamableHttpClientTransportConfig::with_uri(url),
    );
    let client = ClientConfig::default()
        .with_protocol_version(ProtocolVersion::V_2025_11_25)
        .serve(transport)
        .await
        .unwrap();
    let mut control = redis::Client::open(server.url.as_str())
        .unwrap()
        .get_connection()
        .unwrap();
    let _: String = redis::cmd("CLIENT")
        .arg("PAUSE")
        .arg(5000)
        .arg("WRITE")
        .query(&mut control)
        .unwrap();
    let peer = client.peer().clone();
    let create = tokio::spawn(async move {
        peer.call_tool(CallToolRequestParams::new("slopsearx_create_saved_search").with_arguments(json!({"query":"admitted before close","engines":["brave"],"interval_seconds":60,"start_immediately":true}).as_object().unwrap().clone())).await
    });
    let until = tokio::time::Instant::now() + Duration::from_secs(1);
    loop {
        let clients: String = redis::cmd("CLIENT")
            .arg("LIST")
            .query(&mut control)
            .unwrap();
        if clients.lines().any(|line| line.contains("cmd=eval ")) {
            break;
        }
        assert!(
            tokio::time::Instant::now() < until,
            "native create never reached paused Valkey write"
        );
        tokio::time::sleep(Duration::from_millis(5)).await;
    }
    let closing = registry.clone();
    let close = tokio::spawn(async move { closing.close().await });
    tokio::task::yield_now().await;
    assert!(
        !close.is_finished(),
        "close escaped a store-admitted create before registration"
    );
    let _: String = redis::cmd("CLIENT")
        .arg("UNPAUSE")
        .query(&mut control)
        .unwrap();
    assert!(create.await.unwrap().is_ok());
    tokio::time::timeout(Duration::from_secs(3), close)
        .await
        .unwrap()
        .unwrap();
    registry.close().await;
    let before = store.list_definitions(128).await.unwrap().len();
    assert_eq!(before, 1);
    let denied=client.call_tool(CallToolRequestParams::new("slopsearx_create_saved_search").with_arguments(json!({"query":"must not mutate after close","engines":["brave"],"interval_seconds":60,"start_immediately":true}).as_object().unwrap().clone())).await;
    assert!(denied.is_err());
    assert_eq!(store.list_definitions(128).await.unwrap().len(), before);
    let calls = engine.0.load(Ordering::SeqCst);
    tokio::time::sleep(Duration::from_millis(30)).await;
    assert_eq!(engine.0.load(Ordering::SeqCst), calls);
    client.cancel().await.unwrap();
    search.shutdown().await;
    cancel.cancel();
    tokio::time::timeout(Duration::from_secs(3), host)
        .await
        .unwrap()
        .unwrap();
    drop(control);
    drop(registry);
    drop(store);
    drop(server);
    println!("W05_CASE_COMPLETE:actual-mcp-close-admission-spawn-join-fence");
    println!("W05_CASE_COMPLETE:actual-mcp-late-create-rejected-without-mutation");
    println!("W05_CASE_COMPLETE:actual-mcp-repeated-close-idempotent");
}
