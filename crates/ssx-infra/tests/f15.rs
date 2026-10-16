//! F15 offline provider and read-boundary integration; private sockets/state only.
use ssx_application::{
    NoopSink, PortFuture, SearchEngine,
    cache::*,
    policy::*,
    rerank::{Candidate, Provider},
    scope::*,
    snapshot::*,
};
use ssx_domain::*;
use ssx_infra::{
    http::*,
    rerank::{JevConfig, JevReranker, Usage, UsageSink},
    search::*,
    valkey::{Valkey, cache::ValkeyCache, snapshot::*},
};
use ssx_json::{Value, json};
use std::{
    collections::{BTreeMap, BTreeSet},
    sync::{
        Arc, Mutex,
        atomic::{AtomicUsize, Ordering},
    },
    time::Duration,
};
fn capability(name: &str) -> EngineCapability {
    EngineCapability {
        name: name.into(),
        enabled: true,
        active: true,
        available: true,
        categories: vec!["general".into()],
        media_types: vec![],
        auth_ready: true,
        circuit_open: false,
        cost_class: "free".into(),
        health_status: "unknown".into(),
        health_stale: true,
        enforced_filters: BTreeMap::new(),
    }
}

fn dependencies(engines: Vec<(&str, Arc<dyn SearchEngine>)>) -> SearchDependencies {
    SearchDependencies {
        policy: PolicyService {
            policy: SearchPolicy::default(),
            catalog: CatalogSnapshot::new(engines.iter().map(|(n, _)| capability(n)).collect())
                .unwrap(),
            resolver: ScopeResolver {
                catalog_routing: false,
                ..Default::default()
            },
        },
        engines: engines
            .into_iter()
            .map(|(n, engine)| {
                (
                    n.into(),
                    EngineBinding {
                        engine,
                        generation: "fixture-v1".into(),
                        timeout: Duration::from_secs(3),
                        scrape: false,
                    },
                )
            })
            .collect(),
        routing_identity: RoutingIdentity {
            catalog: None,
            sensitive: ["hibp", "dehashed"]
                .into_iter()
                .map(str::to_owned)
                .collect(),
            tier1: ScopeResolver::default().tier1,
            budget: None,
            ranking_strategy: "presence".into(),
            work_group_version: "scholarly-work-v3".into(),
            rerank_identity: None,
            routing_extension: None,
        },
        cache: None,
        client_admission: None,
        suggestions: None,
        jev: None,
        routing_cache: None,
        metrics: Arc::new(NoopSink),
        supervisor: Arc::new(NoopSink),
        audit: Arc::new(NoopSink),
        cooldowns: None,
        owned_http: vec![],
        owned_stores: vec![],
        clock: Arc::new(SystemSearchClock),
        reranker: None,
    }
}
fn proposal(query: &str) -> ScopeProposal {
    ScopeProposal::generic(SearchRequest {
        query: query.into(),
        ..Default::default()
    })
}
struct PrivateValkey {
    child: std::process::Child,
    dir: std::path::PathBuf,
    url: String,
}
impl PrivateValkey {
    async fn start() -> Self {
        let mut random = [0; 8];
        aws_lc_rs::rand::fill(&mut random).unwrap();
        let suffix: String = random.iter().map(|b| format!("{b:02x}")).collect();
        let dir = std::env::temp_dir().join(format!("ssx-f15-{suffix}"));
        std::fs::create_dir(&dir).unwrap();
        let child = std::process::Command::new(
            std::env::var("SLOPSEARX_TEST_VALKEY_SERVER")
                .unwrap_or_else(|_| "valkey-server".into()),
        )
        .args([
            "--port",
            "0",
            "--save",
            "",
            "--appendonly",
            "no",
            "--unixsocketperm",
            "700",
            "--dir",
        ])
        .arg(&dir)
        .arg("--unixsocket")
        .arg(dir.join("s.sock"))
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .spawn()
        .unwrap();
        let server = Self {
            child,
            url: format!("unix://{}", dir.join("s.sock").display()),
            dir,
        };
        let store = Valkey::new(&server.url, Duration::from_millis(100)).unwrap();
        tokio::time::timeout(Duration::from_secs(3), async {
            while store.ping().await.is_err() {
                tokio::time::sleep(Duration::from_millis(10)).await;
            }
        })
        .await
        .unwrap();
        store.close().await;
        server
    }
    async fn keys(&self, prefix: &str) -> Vec<String> {
        let mut c = redis::Client::open(self.url.as_str())
            .unwrap()
            .get_multiplexed_async_connection()
            .await
            .unwrap();
        redis::cmd("KEYS")
            .arg(prefix)
            .query_async(&mut c)
            .await
            .unwrap()
    }
}
impl Drop for PrivateValkey {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
        let _ = std::fs::remove_dir_all(&self.dir);
    }
}

struct Engine {
    name: &'static str,
    count: usize,
    calls: AtomicUsize,
}
impl SearchEngine for Engine {
    fn search<'a>(&'a self, _: &'a SearchRequest) -> PortFuture<'a, AdapterResponse> {
        Box::pin(async {
            self.calls.fetch_add(1, Ordering::SeqCst);
            AdapterResponse {
                results: (0..self.count)
                    .map(|i| SearchResult {
                        url: format!("https://example.test/{i}"),
                        title: format!("row{i}"),
                        content: "evidence".into(),
                        engine: self.name.into(),
                        ..Default::default()
                    })
                    .collect(),
                answers: vec![json!({"answer":"full"})],
                ..Default::default()
            }
        })
    }
}
#[derive(Default)]
struct Quota(AtomicUsize);
impl ClientAdmission for Quota {
    fn acquire<'a>(&'a self, _: &'a str) -> PortFuture<'a, bool> {
        Box::pin(async {
            self.0.fetch_add(1, Ordering::SeqCst);
            true
        })
    }
}
#[tokio::test]
async fn read_capture_preserves_full_canonical_per_caller_views_and_snapshot_semantics() {
    let server = PrivateValkey::start().await;
    let store = Arc::new(Valkey::new(&server.url, Duration::from_millis(250)).unwrap());
    let engine = Arc::new(Engine {
        name: "brave",
        count: 6,
        calls: AtomicUsize::new(0),
    });
    let quota = Arc::new(Quota::default());
    let mut deps = dependencies(vec![("brave", engine.clone())]);
    deps.cache = Some(Arc::new(CanonicalCache::new(
        Arc::new(ValkeyCache::new(store.clone())),
        CacheTtls::default(),
    )));
    deps.client_admission = Some(quota.clone());
    deps.owned_stores.push(store.clone());
    let service = SearchService::new(SearchConfig::default(), deps).unwrap();
    let mut p = proposal("capture");
    p.request.max_results = Some(1);
    p.request.include.clear();
    p.request.client_identifier = Some("caller".into());
    let first = service
        .capture_read(p.clone(), SearchCancellation::default())
        .await
        .unwrap();
    assert_eq!(first.full_result_count(), 6);
    let view = first.present_with_advisories().unwrap();
    assert_eq!(view.response.results.len(), 1);
    assert!(view.response.engine_outcomes.is_empty());
    assert!(!view.response.cached);
    assert_eq!(view.advisories[0]["capability"], "jev_reranking");
    assert_eq!(
        view.advisories[0]["quality_evidence"],
        "operator_reported_production"
    );
    assert_eq!(first.present(), view.response);
    let mut ids = BTreeSet::from([view.response.query_id]);
    let keys = server.keys("*").await;
    assert_eq!(
        keys.len(),
        1,
        "read capture creates only canonical cache, never a snapshot"
    );
    let authority = service.prepare_scope(&p).await.unwrap();
    assert_eq!(quota.0.load(Ordering::SeqCst), 1);
    let prepared = service
        .capture_prepared(authority, SearchCancellation::default())
        .await
        .unwrap();
    assert_eq!(prepared.full_result_count(), 6);
    let again = prepared.present_with_advisories().unwrap();
    assert!(again.response.cached);
    assert_eq!(again.advisories, view.advisories);
    assert!(ids.insert(again.response.query_id));
    let direct = service.search(p.clone()).await.unwrap();
    assert!(direct.cached);
    assert_eq!(direct.results.len(), 1);
    assert!(ids.insert(direct.query_id));
    let authority = service.prepare_scope(&p).await.unwrap();
    let direct_prepared = service
        .search_prepared(authority, SearchCancellation::default())
        .await
        .unwrap();
    assert!(direct_prepared.cached);
    assert!(ids.insert(direct_prepared.query_id));
    let snapshots = SnapshotService::new(
        Arc::new(ValkeySnapshots::new(store.clone())),
        Arc::new(SystemSnapshotClock),
        Arc::new(RandomSnapshotIds),
        TenantId::new("f15-private").unwrap(),
        SnapshotTtl::new(60).unwrap(),
    );
    let captured = service.search_snapshot(p, &snapshots, &[]).await.unwrap();
    assert_eq!(captured.response.results.len(), 1);
    assert!(captured.response.cached);
    assert_eq!(captured.canonical.response().results.len(), 6);
    assert_eq!(captured.canonical.response().engine_outcomes.len(), 1);
    assert_eq!(
        captured.canonical.response().answers,
        vec![json!({"answer":"full"})]
    );
    assert!(ids.insert(captured.response.query_id.clone()));
    assert_eq!(
        captured.canonical.response().query_id,
        captured.response.query_id
    );
    let SnapshotLookup::Found(snapshot) = snapshots.read(&captured.snapshot.unwrap()).await else {
        panic!("snapshot missing")
    };
    assert_eq!(snapshot.total(), 6);
    assert_eq!(quota.0.load(Ordering::SeqCst), 5);
    assert_eq!(engine.calls.load(Ordering::SeqCst), 1);
    assert_eq!(ids.len(), 5);
    service.shutdown().await;
}
#[derive(Default)]
struct UsageRecorder(Mutex<Vec<Usage>>);
impl UsageSink for UsageRecorder {
    fn observed(&self, usage: Usage) {
        self.0.lock().unwrap().push(usage);
    }
}
struct Reply {
    status: u16,
    headers: reqwest::header::HeaderMap,
    bytes: Vec<u8>,
    seen: Mutex<Vec<Request>>,
}
impl Transport for Reply {
    fn exchange(&self, request: Request) -> ExchangeFuture<'_> {
        Box::pin(async move {
            self.seen.lock().unwrap().push(request.clone());
            Ok(Response {
                status: self.status,
                url: request.url,
                headers: self.headers.clone(),
                body: self.bytes.clone(),
            })
        })
    }
}
fn candidates() -> Vec<Candidate> {
    vec![
        Candidate {
            id: "c0".into(),
            title: "first".into(),
            url: "https://a/".into(),
            snippet: "evidence".into(),
        },
        Candidate {
            id: "c1".into(),
            title: "second".into(),
            url: "https://b/".into(),
            snippet: "evidence".into(),
        },
    ]
}
fn provider(raw: Vec<u8>) -> (JevReranker, Arc<Reply>, Arc<UsageRecorder>) {
    let transport = Arc::new(Reply {
        status: 200,
        headers: Default::default(),
        bytes: raw,
        seen: Mutex::new(vec![]),
    });
    let http = AdapterHttp::default();
    http.set_transport(transport.clone()).unwrap();
    let usage = Arc::new(UsageRecorder::default());
    let config = JevConfig::from_environment(&BTreeMap::from([(
        "TYPESAFE_API_KEY".into(),
        "synthetic-offline-key".into(),
    )]))
    .unwrap();
    (
        JevReranker::with_transport(
            config,
            http,
            "https://offline.test/systemone".into(),
            usage.clone(),
        ),
        transport,
        usage,
    )
}
#[tokio::test]
async fn provider_strict_answers_stable_ties_usage_and_input_bounds_execute() {
    for (scores, expected) in [
        ("1,9", vec!["c1", "c0"]),
        ("0,-0.0", vec!["c0", "c1"]),
        ("4,4", vec!["c0", "c1"]),
    ] {
        let (a, b) = scores.split_once(',').unwrap();
        let raw = format!(
            "{{\"model\":\"jev-1.13.0\",\"answers\":{{\"c0\":{{\"type\":\"score\",\"score\":{a}}},\"c1\":{{\"type\":\"score\",\"score\":{b}}}}},\"usage\":{{\"input_tokens\":12,\"output_tokens\":3}}}}"
        );
        let (p, t, u) = provider(raw.into_bytes());
        assert_eq!(
            p.rerank("query", &candidates()).await.unwrap().ordered_ids,
            expected
        );
        {
            let seen = t.seen.lock().unwrap();
            assert_eq!(seen.len(), 1);
            assert_eq!(seen[0].method, reqwest::Method::POST);
            assert_eq!(
                seen[0].headers["authorization"],
                "Bearer synthetic-offline-key"
            );
            assert_eq!(u.0.lock().unwrap()[0].input_tokens, "12");
        }
        p.shutdown().await;
    }
    for raw in [
        r#"{"model":"wrong","answers":{},"usage":{"input_tokens":123456789012345678901234567890,"output_tokens":0}}"#,
        r#"{"model":"jev-1.13.0","answers":{"c0":{"type":"score","score":true},"c1":{"type":"score","score":4}},"usage":{"input_tokens":1,"output_tokens":2}}"#,
    ] {
        let (p, t, u) = provider(raw.as_bytes().to_vec());
        assert!(p.rerank("query", &candidates()).await.is_none());
        assert_eq!(t.seen.lock().unwrap().len(), 1);
        assert_eq!(u.0.lock().unwrap().len(), 1);
        p.shutdown().await;
    }
    let (p, t, _) = provider(b"{}".to_vec());
    let mut rows = candidates();
    rows[0].title = "x".repeat(257);
    assert!(p.rerank("query", &rows).await.is_none());
    assert!(p.rerank(&"q".repeat(4097), &candidates()).await.is_none());
    let mut rows = candidates();
    rows[1].id = "c0".into();
    assert!(p.rerank("query", &rows).await.is_none());
    assert!(p.rerank("query", &[]).await.is_none());
    assert!(t.seen.lock().unwrap().is_empty());
    p.shutdown().await;
}

struct LiveCatalog(Mutex<CatalogSnapshot>);
impl AdvisoryCatalog for LiveCatalog {
    fn snapshot(&self) -> CatalogSnapshot {
        self.0.lock().unwrap().clone()
    }
}
#[tokio::test]
async fn cached_read_uses_current_trusted_availability_without_new_execution_or_authority() {
    let server = PrivateValkey::start().await;
    let store = Arc::new(Valkey::new(&server.url, Duration::from_millis(250)).unwrap());
    let engine = Arc::new(Engine {
        name: "wikipedia",
        count: 1,
        calls: AtomicUsize::new(0),
    });
    let quota = Arc::new(Quota::default());
    let mut deps = dependencies(vec![("wikipedia", engine.clone())]);
    deps.client_admission = Some(quota.clone());
    deps.cache = Some(Arc::new(CanonicalCache::new(
        Arc::new(ValkeyCache::new(store.clone())),
        CacheTtls::default(),
    )));
    deps.owned_stores.push(store.clone());
    let mut brave = capability("brave");
    brave.auth_ready = false;
    let live = Arc::new(LiveCatalog(Mutex::new(
        CatalogSnapshot::new(vec![capability("wikipedia"), brave.clone()]).unwrap(),
    )));
    let service = SearchService::new_with_read_catalog(
        SearchConfig::default(),
        deps,
        Arc::new(ssx_registry::Registry::builtins().unwrap()),
        Some(live.clone()),
    )
    .unwrap();
    let mut p = proposal("cached availability");
    p.request.categories = Some(vec!["general".into()]);
    p.request.client_identifier = Some("caller".into());
    let first = service
        .capture_read(p.clone(), SearchCancellation::default())
        .await
        .unwrap();
    let view = first.present_with_advisories().unwrap();
    assert_eq!(view.advisories.len(), 1);
    assert_eq!(view.advisories[0]["reason"], "credentials_missing");
    assert_eq!(view.response.scope.selected_engines, vec!["wikipedia"]);
    brave.auth_ready = true;
    *live.0.lock().unwrap() = CatalogSnapshot::new(vec![capability("wikipedia"), brave]).unwrap();
    assert!(
        first
            .present_with_advisories()
            .unwrap()
            .advisories
            .is_empty(),
        "presentation re-reads live facts, even for the same immutable capture"
    );
    let second = service
        .capture_read(p.clone(), SearchCancellation::default())
        .await
        .unwrap();
    let view = second.present_with_advisories().unwrap();
    assert!(view.response.cached);
    assert!(view.advisories.is_empty());
    assert_eq!(view.response.scope.selected_engines, vec!["wikipedia"]);
    assert_eq!(engine.calls.load(Ordering::SeqCst), 1);
    assert_eq!(quota.0.load(Ordering::SeqCst), 2);
    let cancellation = SearchCancellation::default();
    cancellation.cancel();
    assert!(matches!(
        service.capture_read(p.clone(), cancellation).await,
        Err(SearchError::Cancelled)
    ));
    let authority = service.prepare_scope(&p).await.unwrap();
    let cancellation = SearchCancellation::default();
    cancellation.cancel();
    assert!(matches!(
        service.capture_prepared(authority, cancellation).await,
        Err(SearchError::Cancelled)
    ));
    p.request.engines = Some(vec!["hibp".into()]);
    assert!(matches!(
        service.capture_read(p, SearchCancellation::default()).await,
        Err(SearchError::Policy(_))
    ));
    assert_eq!(quota.0.load(Ordering::SeqCst), 2);
    assert_eq!(engine.calls.load(Ordering::SeqCst), 1);
    let keys = server.keys("*").await;
    assert_eq!(keys.len(), 1);
    let ssx_infra::valkey::Read::Found(raw) = store.get_object(&keys[0]).await else {
        panic!("canonical missing")
    };
    assert!(!ssx_json::to_string(&raw).unwrap().contains("advisories"));
    service.shutdown().await;
}

fn source_cases() -> Value {
    ssx_json::from_slice(include_bytes!("../../../docs/rust-rewrite/f15/cases.json")).unwrap()
}
fn source_response(case: &Value) -> Vec<u8> {
    use base64::Engine;
    if let Some(raw) = case["response_base64"].as_str() {
        return base64::engine::general_purpose::STANDARD
            .decode(raw)
            .unwrap();
    }
    let recipe = &case["response"];
    let base = recipe["base_text"].as_str().unwrap();
    match recipe["kind"].as_str().unwrap() {
        "padded" => {
            let mut raw = base.as_bytes().to_vec();
            raw.resize(recipe["size"].as_u64().unwrap() as usize, b' ');
            raw
        }
        "nested" => format!(
            "{},\"unused\":{}0{}}}",
            &base[..base.len() - 1],
            recipe["open"]
                .as_str()
                .unwrap()
                .repeat(recipe["depth"].as_u64().unwrap() as usize),
            recipe["close"]
                .as_str()
                .unwrap()
                .repeat(recipe["depth"].as_u64().unwrap() as usize)
        )
        .into_bytes(),
        kind => panic!("uncaptured response kind {kind}"),
    }
}
#[tokio::test]
async fn actual_source_provider_cases_replay_exact_wire_identity_usage_and_decisions() {
    use base64::Engine;
    use sha2::{Digest, Sha256};
    let fixture = source_cases();
    let mut replayed = 0;
    for case in fixture["cases"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|c| c["kind"] == "provider")
    {
        let raw = source_response(case);
        let digest = Sha256::digest(&raw)
            .iter()
            .map(|b| format!("{b:02x}"))
            .collect::<String>();
        assert_eq!(
            digest,
            case["response_sha256"].as_str().unwrap(),
            "{}",
            case["id"]
        );
        let mut headers = reqwest::header::HeaderMap::new();
        for (name, value) in case["headers"].as_object().unwrap() {
            headers.insert(
                reqwest::header::HeaderName::from_bytes(name.as_bytes()).unwrap(),
                value.as_str().unwrap().parse().unwrap(),
            );
        }
        let reply = Arc::new(Reply {
            bytes: raw,
            seen: Mutex::new(vec![]),
            status: case["status"].as_u64().unwrap() as u16,
            headers,
        });
        let http = AdapterHttp::default();
        http.set_transport(reply.clone()).unwrap();
        let usage = Arc::new(UsageRecorder::default());
        let config = JevConfig::from_environment(&BTreeMap::from([(
            "TYPESAFE_API_KEY".into(),
            "synthetic-offline-key".into(),
        )]))
        .unwrap();
        let provider = JevReranker::with_transport(
            config,
            http,
            "https://api.typesafe.ai/v1/systemone".into(),
            usage.clone(),
        );
        let rows = case["candidates"]
            .as_array()
            .unwrap()
            .iter()
            .map(|c| Candidate {
                id: c["id"].as_str().unwrap().into(),
                title: c["title"].as_str().unwrap().into(),
                url: c["url"].as_str().unwrap().into(),
                snippet: c["snippet"].as_str().unwrap().into(),
            })
            .collect::<Vec<_>>();
        let decision = provider
            .rerank(case["query"].as_str().unwrap(), &rows)
            .await;
        let actual = decision
            .map(|d| json!(d.ordered_ids))
            .unwrap_or(Value::Null);
        let safe_deep_delta = case["id"] == "unused_array_depth_120000";
        let expected = if safe_deep_delta {
            json!(["c1", "c0"])
        } else {
            case["output"]["ordered_ids"].clone()
        };
        assert_eq!(actual, expected, "{}", case["id"]);
        assert_eq!(
            provider.cache_identity(),
            case["output"]["identity"].as_str().unwrap(),
            "{}",
            case["id"]
        );
        {
            let requests = reply.seen.lock().unwrap();
            let expected = case["output"]["wire"].as_array().unwrap();
            assert_eq!(requests.len(), expected.len(), "{}", case["id"]);
            for (request, expected) in requests.iter().zip(expected) {
                assert_eq!(
                    request.method.as_str(),
                    expected["method"].as_str().unwrap()
                );
                assert_eq!(request.url.as_str(), expected["url"].as_str().unwrap());
                assert_eq!(
                    request.headers["authorization"],
                    expected["authorization"].as_str().unwrap()
                );
                assert_eq!(
                    request.headers["content-type"],
                    expected["content_type"].as_str().unwrap()
                );
                assert_eq!(
                    base64::engine::general_purpose::STANDARD.encode(&request.body),
                    expected["body_base64"].as_str().unwrap(),
                    "{}",
                    case["id"]
                );
            }
        }
        let actual=usage.0.lock().unwrap().iter().map(|u|json!({"kind":"usage","input_tokens":u.input_tokens,"output_tokens":u.output_tokens,"candidates":u.candidates})).collect::<Vec<_>>();
        let expected = if safe_deep_delta {
            vec![json!({"kind":"usage","input_tokens":"12","output_tokens":"3","candidates":2})]
        } else {
            case["output"]["logs"]
                .as_array()
                .unwrap()
                .iter()
                .filter(|l| l["kind"] == "usage")
                .cloned()
                .collect()
        };
        assert_eq!(actual, expected, "{}", case["id"]);
        provider.shutdown().await;
        replayed += 1;
    }
    assert_eq!(replayed, 83);
}
#[test]
fn actual_source_projection_and_advisory_cases_replay_full_objects() {
    let fixture = source_cases();
    let mut replayed = 0;
    for case in fixture["cases"].as_array().unwrap() {
        match case["kind"].as_str().unwrap() {
            "url" => {
                assert_eq!(
                    ssx_application::rerank::url(case["input"].as_str().unwrap()),
                    case["output"].as_str().unwrap(),
                    "{}",
                    case["id"]
                );
                replayed += 1;
            }
            "text" => {
                assert_eq!(
                    ssx_application::rerank::text(
                        case["input"].as_str().unwrap(),
                        case["limit"].as_u64().unwrap() as usize
                    ),
                    case["output"].as_str().unwrap(),
                    "{}",
                    case["id"]
                );
                replayed += 1;
            }
            "advisory" => {
                use ssx_application::advisories::{Context, Engine};
                let input = &case["input"];
                let strings = |v: &Value| {
                    v.as_array()
                        .unwrap()
                        .iter()
                        .map(|v| v.as_str().unwrap().to_owned())
                        .collect::<Vec<_>>()
                };
                let request = SearchRequest {
                    query: input["request"]["query"].as_str().unwrap().into(),
                    engines: input["request"]["engines"]
                        .as_array()
                        .map(|_| strings(&input["request"]["engines"])),
                    categories: input["request"]["categories"]
                        .as_array()
                        .map(|_| strings(&input["request"]["categories"])),
                    media_type: input["request"]["media_type"].as_str().map(str::to_owned),
                    ..Default::default()
                };
                let scope: ScopeDecision = ssx_json::from_value(input["scope"].clone()).unwrap();
                let context = &input["context"];
                let context = Context {
                    engines: context["engines"]
                        .as_array()
                        .unwrap()
                        .iter()
                        .map(|c| Engine {
                            name: c["name"].as_str().unwrap().into(),
                            display_name: c["display_name"].as_str().unwrap().into(),
                            enabled: c["enabled"].as_bool().unwrap(),
                            credentials_required: c["credentials_required"].as_bool().unwrap(),
                            credentials_configured: c["credentials_configured"].as_bool().unwrap(),
                            sensitive: c["sensitive"].as_bool().unwrap(),
                            categories: strings(&c["categories"]),
                            media_types: strings(&c["media_types"]),
                        })
                        .collect(),
                    hidden: strings(&context["hidden"]).into_iter().collect(),
                    automatic_names: strings(&context["automatic_names"]).into_iter().collect(),
                    routing_configured: context["routing_configured"].as_bool().unwrap(),
                    reranking_configured: context["reranking_configured"].as_bool().unwrap(),
                };
                assert_eq!(
                    json!(ssx_application::advisories::search(
                        &request,
                        &scope,
                        &context,
                        input["result_count"].as_u64().unwrap() as usize
                    )),
                    case["output"],
                    "{}",
                    case["id"]
                );
                replayed += 1;
            }
            _ => (),
        }
    }
    assert_eq!(replayed, 35);
}

struct CapturedAdvice {
    mode: String,
    calls: Mutex<Vec<Value>>,
}
impl Provider for CapturedAdvice {
    fn rerank<'a>(
        &'a self,
        query: &'a str,
        candidates: &'a [Candidate],
    ) -> PortFuture<'a, Option<ssx_application::rerank::Decision>> {
        Box::pin(async move {
            self.calls.lock().unwrap().push(json!({"query":query,"candidates":candidates.iter().map(|c|json!({"id":c.id,"title":c.title,"url":c.url,"snippet":c.snippet})).collect::<Vec<_>>()}));
            let mut ids = candidates.iter().map(|c| c.id.clone()).collect::<Vec<_>>();
            match self.mode.as_str() {
                "reverse" => ids.reverse(),
                "duplicate" => ids = vec![ids[0].clone(); ids.len()],
                "unknown" => ids[0] = "unknown".into(),
                "missing" => {
                    ids.pop();
                }
                _ => return None,
            }
            Some(ssx_application::rerank::Decision { ordered_ids: ids })
        })
    }
    fn cache_identity(&self) -> String {
        "offline-provider-v1".into()
    }
}
#[tokio::test]
async fn actual_source_host_cases_preserve_whole_results_tier_crossing_and_fixed_tail() {
    let fixture = source_cases();
    let mut replayed = 0;
    for case in fixture["cases"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|c| c["kind"] == "host")
    {
        let rows = case["input"]
            .as_array()
            .unwrap()
            .iter()
            .cloned()
            .map(ssx_domain::storage::decode_result)
            .collect::<std::result::Result<Vec<_>, _>>()
            .unwrap();
        let provider = Arc::new(CapturedAdvice {
            mode: case["mode"].as_str().unwrap().into(),
            calls: Mutex::new(vec![]),
        });
        let stage = ssx_infra::rerank::HostReranker::new(provider.clone());
        let decision = stage
            .rerank(
                &SearchRequest {
                    query: "query".into(),
                    ..Default::default()
                },
                &rows,
                tokio::time::Instant::now() + Duration::from_secs(1),
            )
            .await
            .unwrap();
        let results = decision
            .results
            .iter()
            .map(ssx_domain::storage::encode_result)
            .collect::<std::result::Result<Vec<_>, _>>()
            .unwrap();
        assert_eq!(json!(results), case["output"]["results"], "{}", case["id"]);
        assert_eq!(
            json!(*provider.calls.lock().unwrap()),
            case["output"]["calls"],
            "{}",
            case["id"]
        );
        assert_eq!(
            format!("{:?}", decision.state).to_lowercase(),
            case["output"]["state"].as_str().unwrap()
        );
        assert_eq!(&decision.results[40..], &rows[40..]);
        replayed += 1;
    }
    assert_eq!(replayed, 6);
}

struct SocketServer {
    endpoint: String,
    seen: Arc<Mutex<Vec<Vec<u8>>>>,
    disconnected: Arc<AtomicUsize>,
    release: tokio::sync::watch::Sender<bool>,
    stop: tokio::sync::watch::Sender<bool>,
    task: tokio::task::JoinHandle<()>,
}
impl SocketServer {
    async fn start(body: Vec<u8>, headers: Vec<(String, String)>, hold: bool) -> Self {
        use tokio::io::{AsyncReadExt, AsyncWriteExt};
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let endpoint = format!("http://{}", listener.local_addr().unwrap());
        let seen = Arc::new(Mutex::new(vec![]));
        let disconnected = Arc::new(AtomicUsize::new(0));
        let (release, _) = tokio::sync::watch::channel(!hold);
        let (stop, mut stopping) = tokio::sync::watch::channel(false);
        let observed = seen.clone();
        let drops = disconnected.clone();
        let opened = release.clone();
        let task = tokio::spawn(async move {
            let mut tasks = tokio::task::JoinSet::new();
            loop {
                tokio::select! {
                    _=stopping.changed()=>break,
                    accepted=listener.accept()=>{
                        let(mut socket,_)=accepted.unwrap();let body=body.clone();let headers=headers.clone();let seen=observed.clone();let drops=drops.clone();let mut ready=opened.subscribe();
                        tasks.spawn(async move {
                            let mut raw=Vec::new();let mut buffer=[0;4096];
                            let complete=tokio::time::timeout(Duration::from_secs(5),async {
                                loop {
                                    let Some(end)=raw.windows(4).position(|b|b==b"\r\n\r\n").map(|i|i+4) else {
                                        let n=socket.read(&mut buffer).await.unwrap();if n==0{return None;}raw.extend_from_slice(&buffer[..n]);assert!(raw.len()<=128000+16384);continue;
                                    };
                                    let header=String::from_utf8_lossy(&raw[..end]);let length=header.lines().find_map(|l|l.split_once(':').filter(|(n,_)|n.eq_ignore_ascii_case("content-length")).map(|(_,v)|v.trim().parse::<usize>().unwrap())).unwrap_or(0);
                                    assert!(length<=128000);
                                    while raw.len()<end+length {let n=socket.read(&mut buffer).await.unwrap();if n==0{return None;}raw.extend_from_slice(&buffer[..n]);}
                                    return Some(raw);
                                }
                            }).await.unwrap();
                            let Some(raw)=complete else{return;};seen.lock().unwrap().push(raw);
                            if !*ready.borrow_and_update(){tokio::select!{
                                _=async{while !*ready.borrow_and_update(){ready.changed().await.unwrap();}}=>(),
                                n=socket.read(&mut buffer)=>{assert_eq!(n.unwrap(),0);drops.fetch_add(1,Ordering::SeqCst);return;},
                            }}
                            let extra=headers.iter().map(|(n,v)|format!("{n}: {v}\r\n")).collect::<String>();
                            let response=format!("HTTP/1.1 200 OK\r\nContent-Length: {}\r\nConnection: close\r\n{extra}\r\n",body.len());
                            if socket.write_all(response.as_bytes()).await.is_ok(){let _=socket.write_all(&body).await;}
                        });
                    }
                }
            }
            tasks.shutdown().await;
        });
        Self {
            endpoint,
            seen,
            disconnected,
            release,
            stop,
            task,
        }
    }
    async fn wait_requests(&self, count: usize) {
        tokio::time::timeout(Duration::from_secs(3), async {
            while self.seen.lock().unwrap().len() < count {
                tokio::time::sleep(Duration::from_millis(1)).await;
            }
        })
        .await
        .unwrap();
    }
    async fn wait_disconnects(&self, count: usize) {
        tokio::time::timeout(Duration::from_secs(3), async {
            while self.disconnected.load(Ordering::SeqCst) < count {
                tokio::time::sleep(Duration::from_millis(1)).await;
            }
        })
        .await
        .unwrap();
    }
    async fn shutdown(self) {
        self.stop.send_replace(true);
        self.task.await.unwrap();
    }
}
fn socket_provider(server: &SocketServer) -> Arc<JevReranker> {
    Arc::new(JevReranker::with_transport(
        JevConfig::from_environment(&BTreeMap::from([(
            "TYPESAFE_API_KEY".into(),
            "synthetic-offline-key".into(),
        )]))
        .unwrap(),
        AdapterHttp::default(),
        server.endpoint.clone(),
        Arc::new(UsageRecorder::default()),
    ))
}
fn two_answers() -> Vec<u8> {
    br#"{"model":"jev-1.13.0","answers":{"c0":{"type":"score","score":1},"c1":{"type":"score","score":9}}}"#.to_vec()
}
#[tokio::test]
async fn real_socket_provider_bounds_gzip_redirect_free_wire_and_cancellation_release_slots() {
    use std::io::Write;
    let mut encoded = flate2::write::GzEncoder::new(Vec::new(), flate2::Compression::default());
    encoded.write_all(&two_answers()).unwrap();
    let gzip = encoded.finish().unwrap();
    let server = SocketServer::start(
        gzip,
        vec![("Content-Encoding".into(), "gzip".into())],
        false,
    )
    .await;
    let provider = socket_provider(&server);
    assert_eq!(
        provider
            .rerank("public", &candidates())
            .await
            .unwrap()
            .ordered_ids,
        vec!["c1", "c0"]
    );
    server.wait_requests(1).await;
    let raw = server.seen.lock().unwrap()[0].clone();
    let end = raw.windows(4).position(|b| b == b"\r\n\r\n").unwrap() + 4;
    let header = String::from_utf8_lossy(&raw[..end]);
    assert!(header.starts_with("POST / HTTP/1.1\r\n"));
    assert!(
        header
            .to_ascii_lowercase()
            .contains("authorization: bearer synthetic-offline-key")
    );
    assert_eq!(
        raw[end..],
        JevReranker::request_body("public", &candidates()).unwrap()
    );
    provider.shutdown().await;
    server.shutdown().await;
    let server = SocketServer::start(two_answers(), vec![], true).await;
    let provider = socket_provider(&server);
    let mut first = Vec::new();
    for _ in 0..2 {
        let p = provider.clone();
        first.push(tokio::spawn(async move {
            p.rerank("held", &candidates()).await
        }));
    }
    server.wait_requests(2).await;
    let p = provider.clone();
    let queued = tokio::spawn(async move { p.rerank("queued", &candidates()).await });
    tokio::task::yield_now().await;
    queued.abort();
    assert!(queued.await.unwrap_err().is_cancelled());
    assert_eq!(server.seen.lock().unwrap().len(), 2);
    for task in first {
        task.abort();
        assert!(task.await.unwrap_err().is_cancelled());
    }
    server.wait_disconnects(2).await;
    server.release.send_replace(true);
    assert!(
        provider
            .rerank("after-cancel", &candidates())
            .await
            .is_some()
    );
    assert_eq!(server.seen.lock().unwrap().len(), 3);
    provider.shutdown().await;
    server.shutdown().await;
    // A decoded gzip body above the source cap is rejected through actual sockets.
    let mut encoded = flate2::write::GzEncoder::new(Vec::new(), flate2::Compression::default());
    encoded.write_all(&vec![b' '; 2_000_001]).unwrap();
    let server = SocketServer::start(
        encoded.finish().unwrap(),
        vec![("Content-Encoding".into(), "gzip".into())],
        false,
    )
    .await;
    let provider = socket_provider(&server);
    assert!(provider.rerank("oversize", &candidates()).await.is_none());
    assert_eq!(server.seen.lock().unwrap().len(), 1);
    provider.shutdown().await;
    server.shutdown().await;
}

#[tokio::test]
async fn expired_host_deadline_never_polls_or_dispatches_provider() {
    let provider = Arc::new(CapturedAdvice {
        mode: "reverse".into(),
        calls: Mutex::new(vec![]),
    });
    let stage = ssx_infra::rerank::HostReranker::new(provider.clone());
    let rows = vec![SearchResult::default(); 2];
    let decision = stage
        .rerank(
            &SearchRequest {
                query: "public".into(),
                ..Default::default()
            },
            &rows,
            tokio::time::Instant::now() - Duration::from_millis(1),
        )
        .await
        .unwrap();
    assert!(provider.calls.lock().unwrap().is_empty());
    assert_eq!(decision.state, RerankState::Fallback);
    assert_eq!(decision.results, rows);
}
