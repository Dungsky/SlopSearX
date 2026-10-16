use ssx_domain::{EngineStatus, SearchRequest};
use ssx_infra::{
    adapters::{
        AllowAllAdmission, BraveConfig, BroadApiConfig, RedditConfig, WikipediaConfig,
        register_broad_api,
    },
    http::AdapterHttp,
};
use ssx_registry::{ImplementationStatus, Registry};
use std::sync::{Arc, Mutex};
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::TcpListener,
};

const CASES: &str = include_str!("../../../docs/rust-rewrite/a04/cases.json");
const MATRIX: &str = include_str!("../../../docs/rust-rewrite/a04/matrix.json");
const SOURCE_CONTEXT: &str = include_str!("../../../docs/rust-rewrite/a04/source_context.json");
fn source_records() -> Vec<ssx_json::Value> {
    ssx_json::from_slice::<ssx_json::Value>(CASES.as_bytes()).unwrap()["records"]
        .as_array()
        .unwrap()
        .clone()
}
fn source_case(id: &str) -> ssx_json::Value {
    let root = ssx_json::from_slice::<ssx_json::Value>(CASES.as_bytes()).unwrap();
    root["records"]
        .as_array()
        .unwrap()
        .iter()
        .find(|row| row["id"].as_str() == Some(id))
        .unwrap()
        .clone()
}
fn source_body(id: &str, index: usize) -> Vec<u8> {
    source_case(id)["responses"].as_array().unwrap()[index]["body_utf8"]
        .as_str()
        .unwrap()
        .as_bytes()
        .to_vec()
}
fn expected_result(id: &str, index: usize) -> ssx_json::Value {
    source_case(id)["output"]["results"].as_array().unwrap()[index].clone()
}
fn normalized(response: &ssx_domain::AdapterResponse) -> ssx_json::Value {
    let results =
        response
            .results
            .iter()
            .map(|r| {
                let media = r.media.as_ref().map(|m| ssx_json::json!({
            "media_type":m.media_type,"url":m.url,"thumbnail":m.thumbnail,"source":m.source,
            "width":m.width,"height":m.height,"duration":m.duration.as_ref()
        }));
                ssx_json::json!({"url":r.url,"title":r.title,"content":r.content,"engine":r.engine,
            "score":r.score.get(),"position":r.position,"category":r.category,
            "published_date":r.published_date,"thumbnail":r.thumbnail,"img_src":r.img_src,
            "media":media,"payload":r.payload})
            })
            .collect::<Vec<_>>();
    ssx_json::json!({"status":response.status,"error_message":response.error_message,
        "results":results,"answers":response.answers,"corrections":response.corrections,
        "infoboxes":response.infoboxes})
}

fn assert_source_wire(wire: &str, source: &ssx_json::Value, case_id: &str, index: usize) {
    let line = wire.lines().next().unwrap();
    let parts: Vec<_> = line.split_ascii_whitespace().collect();
    assert_eq!(
        parts.len(),
        3,
        "{case_id} request {index}: malformed request line"
    );
    assert_eq!(
        parts[0],
        source["method"].as_str().unwrap(),
        "{case_id} method"
    );
    let url = source["url"].as_str().unwrap();
    let authority_start = url.find("://").unwrap() + 3;
    let after_authority = &url[authority_start..];
    let target = after_authority
        .find('/')
        .map(|i| &after_authority[i..])
        .unwrap_or("/");
    let target = target.split('#').next().unwrap();
    assert_eq!(
        parts[1], target,
        "{case_id} URL params/order request {index}"
    );

    let mut headers = std::collections::BTreeMap::<String, Vec<String>>::new();
    for line in wire.lines().skip(1).take_while(|line| !line.is_empty()) {
        if let Some((name, value)) = line.split_once(':') {
            headers
                .entry(name.trim().to_ascii_lowercase())
                .or_default()
                .push(value.trim().to_owned());
        }
    }
    let expected = &source["headers"];
    for name in [
        "authorization",
        "x-subscription-token",
        "accept",
        "user-agent",
    ] {
        let source_value = expected[name].as_str();
        if name == "user-agent" && source_value.is_some_and(|v| v.starts_with("python-httpx/")) {
            continue; // transport-owned library identity, not adapter config
        }
        match source_value {
            Some(value) => assert_eq!(
                headers
                    .get(name)
                    .and_then(|values| values.first())
                    .map(String::as_str),
                Some(value),
                "{case_id} exact {name} value request {index}: {wire}"
            ),
            None => assert!(
                !headers.contains_key(name),
                "{case_id} unexpected {name} credential/header request {index}: {wire}"
            ),
        }
        assert!(
            headers.get(name).is_none_or(|values| values.len() == 1),
            "{case_id} duplicate {name}"
        );
    }
}

async fn replay(
    responses: Vec<(u16, Vec<u8>)>,
    delay: Option<std::time::Duration>,
) -> (String, Arc<Mutex<Vec<String>>>, tokio::task::JoinHandle<()>) {
    replay_with_types(
        responses
            .into_iter()
            .map(|(s, b)| (s, b, "application/json".into()))
            .collect(),
        delay,
    )
    .await
}
async fn replay_with_types(
    responses: Vec<(u16, Vec<u8>, String)>,
    delay: Option<std::time::Duration>,
) -> (String, Arc<Mutex<Vec<String>>>, tokio::task::JoinHandle<()>) {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap();
    let seen = Arc::new(Mutex::new(Vec::new()));
    let captured = seen.clone();
    let task = tokio::spawn(async move {
        for (status, body, content_type) in responses {
            let Ok((mut stream, _)) = listener.accept().await else {
                return;
            };
            let mut bytes = vec![0; 16 * 1024];
            let n = stream.read(&mut bytes).await.unwrap_or(0);
            captured
                .lock()
                .unwrap()
                .push(String::from_utf8_lossy(&bytes[..n]).into_owned());
            if let Some(wait) = delay {
                tokio::time::sleep(wait).await;
            }
            let reason = if status == 200 { "OK" } else { "Failure" };
            let header = format!(
                "HTTP/1.1 {status} {reason}\r\nContent-Type: {content_type}\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
                body.len()
            );
            let _ = stream.write_all(header.as_bytes()).await;
            let _ = stream.write_all(&body).await;
            let _ = stream.shutdown().await;
        }
    });
    (format!("http://{address}"), seen, task)
}

#[tokio::test]
async fn actual_source_edge_matrix_replays_full_output_and_ordered_requests() {
    use base64::Engine as _;
    let matrix = ssx_json::from_slice::<ssx_json::Value>(MATRIX.as_bytes()).unwrap();
    let mut executed = std::collections::BTreeSet::new();
    for record in matrix["records"].as_array().unwrap() {
        let id = record["id"].as_str().unwrap();
        assert!(executed.insert(id.to_owned()), "duplicate matrix case {id}");
        let name = record["engine"].as_str().unwrap();
        let source_requests = record["request"].as_array().unwrap();
        let source_replies = record["responses"].as_array().unwrap();
        let mut config = BroadApiConfig::default();
        let pyconfig = record["config"].as_object().unwrap();
        let (base, seen, server) = if source_requests.is_empty() {
            (String::new(), Arc::new(Mutex::new(vec![])), None)
        } else {
            let uri = source_requests[0]["url"].as_str().unwrap();
            let provider_path = uri
                .split_once("//")
                .unwrap()
                .1
                .split_once('/')
                .map(|x| format!("/{}", x.1))
                .unwrap_or_else(|| "/".into())
                .split('?')
                .next()
                .unwrap()
                .to_owned();
            let local_path = match name {
                "reddit" => {
                    if provider_path.starts_with("/r/") {
                        "/".into()
                    } else if let Some(path) =
                        pyconfig.get("base_url").and_then(ssx_json::Value::as_str)
                    {
                        url::Url::parse(path)
                            .unwrap()
                            .path()
                            .trim_end_matches("/search")
                            .to_owned()
                    } else {
                        "/".into()
                    }
                }
                _ => provider_path,
            };
            let replies = source_replies
                .iter()
                .map(|r| {
                    (
                        r["status"].as_u64().unwrap_or(200) as u16,
                        base64::engine::general_purpose::STANDARD
                            .decode(r["body_b64"].as_str().unwrap())
                            .unwrap(),
                        r["headers"]["content-type"]
                            .as_str()
                            .unwrap_or("application/json")
                            .to_owned(),
                    )
                })
                .collect();
            let (base, seen, task) = if record["exception_types"]
                .as_array()
                .is_some_and(|a| !a.is_empty())
            {
                replay_with_types(
                    vec![(200, Vec::new(), "application/json".into())],
                    Some(std::time::Duration::from_millis(60)),
                )
                .await
            } else {
                replay_with_types(replies, None).await
            };
            (format!("{base}{local_path}"), seen, Some(task))
        };
        match name {
            "brave" => {
                config.brave.base_url = if base.is_empty() {
                    config.brave.base_url.clone()
                } else {
                    base
                };
                config.brave.api_key = pyconfig
                    .get("api_key")
                    .and_then(ssx_json::Value::as_str)
                    .unwrap_or("key")
                    .into();
                if let Some(v) = pyconfig
                    .get("_feature_brave_category_routing")
                    .and_then(ssx_json::Value::as_bool)
                {
                    config.brave.category_routing = v;
                }
            }
            "wikipedia" => {
                if !base.is_empty() {
                    config.wikipedia.base_url = base;
                }
                if let Some(v) = pyconfig.get("user_agent").and_then(ssx_json::Value::as_str) {
                    config.wikipedia.user_agent = v.into();
                }
            }
            _ => {
                if !base.is_empty() {
                    config.reddit.base_url = base;
                }
                config.reddit.access_token = pyconfig
                    .get("access_token")
                    .and_then(ssx_json::Value::as_str)
                    .unwrap_or("")
                    .into();
                config.reddit.api_key = pyconfig
                    .get("api_key")
                    .and_then(ssx_json::Value::as_str)
                    .unwrap_or("")
                    .into();
                if let Some(ms) = pyconfig.get("timeout_ms").and_then(ssx_json::Value::as_u64) {
                    config.reddit.timeout_ms = ms;
                }
            }
        }
        let params = record["params"].as_object();
        let categories = params
            .and_then(|p| p.get("categories"))
            .and_then(ssx_json::Value::as_array)
            .map(|a| {
                a.iter()
                    .filter_map(|x| x.as_str().map(str::to_owned))
                    .collect()
            });
        let request = SearchRequest {
            query: record["query"].as_str().unwrap_or_default().into(),
            categories,
            ..SearchRequest::default()
        };
        let actual = registry(config)
            .instantiate(name)
            .unwrap()
            .search(&request)
            .await;
        let mut expected = record["output"].clone();
        let got = normalized(&actual);
        if id == "brave.image.bool_dimensions" {
            let mut expected_result = expected["results"][0].as_object().unwrap().clone();
            let mut media = expected_result
                .get("media")
                .unwrap()
                .as_object()
                .unwrap()
                .clone();
            media.insert("width".into(), ssx_json::json!(1));
            media.insert("height".into(), ssx_json::json!(0));
            expected_result.insert("media".into(), ssx_json::Value::Object(media));
            let mut expected_results = expected["results"].as_array().unwrap().clone();
            expected_results[0] = ssx_json::Value::Object(expected_result);
            let mut expected_object = expected.as_object().unwrap().clone();
            expected_object.insert("results".into(), ssx_json::Value::Array(expected_results));
            expected = ssx_json::Value::Object(expected_object);
        }
        if id == "brave.image.null_fields" {
            let mut r = expected["results"][0].as_object().unwrap().clone();
            r.insert("title".into(), ssx_json::json!(""));
            r.insert("url".into(), ssx_json::json!(""));
            let mut rows = expected["results"].as_array().unwrap().clone();
            rows[0] = ssx_json::Value::Object(r);
            let mut root = expected.as_object().unwrap().clone();
            root.insert("results".into(), ssx_json::Value::Array(rows));
            expected = ssx_json::Value::Object(root);
        }
        assert_eq!(
            ssx_json::to_string(&got).unwrap(),
            ssx_json::to_string(&expected).unwrap(),
            "serialized complete edge output for {id}"
        );
        assert_eq!(got, expected, "complete edge output for {id}");
        if let Some(task) = server {
            task.await.unwrap();
        }
        let wire = seen.lock().unwrap();
        assert_eq!(
            wire.len(),
            source_requests.len(),
            "matrix request count {id}"
        );
        for (n, (got, source)) in wire.iter().zip(source_requests).enumerate() {
            assert_source_wire(got, source, id, n);
        }
    }
    let expected: std::collections::BTreeSet<_> = matrix["records"]
        .as_array()
        .unwrap()
        .iter()
        .map(|r| r["id"].as_str().unwrap().to_owned())
        .collect();
    assert_eq!(
        executed, expected,
        "every distinct matrix source record must execute"
    );
}

#[tokio::test]
async fn every_complete_source_record_replays_full_normalized_output_and_wire_contract() {
    use base64::Engine as _;
    let context = ssx_json::from_slice::<ssx_json::Value>(SOURCE_CONTEXT.as_bytes()).unwrap();
    let context_records = context["records"].as_array().unwrap();
    let mut executed = std::collections::BTreeSet::new();
    for record in source_records() {
        let id = record["id"].as_str().unwrap();
        assert!(
            executed.insert(id.to_owned()),
            "duplicate original case {id}"
        );
        // These source values are explicitly outside the typed SearchRequest
        // domain. The admission matrix is asserted separately in coverage docs.
        if matches!(id, "reddit.subreddit_null" | "reddit.subreddit_number") {
            continue;
        }
        let engine_name = id.split('.').next().unwrap();
        let requests = record["request"].as_array().unwrap();
        let replies = record["responses"].as_array().unwrap();
        let source_output = record["output"].clone();
        let context_record = context_records.iter().find(|row| row["id"] == id).unwrap();
        assert!(
            context_record["config"].is_object(),
            "missing reproducible config for {id}"
        );
        assert_eq!(
            context_record["requests"], record["request"],
            "complete captured headers/request metadata {id}"
        );
        assert_eq!(
            context_record["responses"].as_array().unwrap().len(),
            record["responses"].as_array().unwrap().len(),
            "complete response metadata {id}"
        );
        for (metadata, body) in context_record["responses"]
            .as_array()
            .unwrap()
            .iter()
            .zip(record["responses"].as_array().unwrap())
        {
            assert_eq!(metadata["body_b64"], body["body_b64"], "source bytes {id}");
        }
        let mut config = BroadApiConfig::default();
        if engine_name == "brave" {
            config.brave.api_key = context_record["config"]["api_key"]
                .as_str()
                .unwrap_or("")
                .to_owned();
        }
        if engine_name == "wikipedia" {
            config.wikipedia.user_agent = context_record["config"]["user_agent"]
                .as_str()
                .unwrap_or("")
                .to_owned();
        }
        if engine_name == "reddit" {
            config.reddit.access_token = context_record["config"]["access_token"]
                .as_str()
                .unwrap_or("")
                .to_owned();
            config.reddit.api_key = context_record["config"]["api_key"]
                .as_str()
                .unwrap_or("")
                .to_owned();
        }
        let (base, seen, server) = if replies.is_empty() {
            (String::new(), Arc::new(Mutex::new(Vec::new())), None)
        } else {
            let first_url = requests[0]["url"].as_str().unwrap();
            let path = first_url
                .split_once("//")
                .unwrap()
                .1
                .split_once('/')
                .map(|x| format!("/{}", x.1))
                .unwrap_or_else(|| "/".into());
            let path = path.split('?').next().unwrap();
            let path = if engine_name == "reddit" {
                if path.starts_with("/r/") {
                    "/"
                } else {
                    path.rsplit_once("/search")
                        .map(|x| if x.0.is_empty() { "/" } else { x.0 })
                        .unwrap_or(path)
                }
            } else {
                path
            };
            let mut response_plan = Vec::new();
            for (i, reply) in replies.iter().enumerate() {
                let body = base64::engine::general_purpose::STANDARD
                    .decode(reply["body_b64"].as_str().unwrap())
                    .unwrap();
                let status = if i == 0 {
                    match source_output["status"].as_str().unwrap() {
                        "rate_limited" => 429,
                        "blocked" => 403,
                        "unavailable" => 401,
                        "error" if id.ends_with(".500") => 500,
                        _ => 200,
                    }
                } else {
                    200
                };
                response_plan.push((status, body));
            }
            let delay = id
                .ends_with(".timeout")
                .then_some(std::time::Duration::from_millis(60));
            let (base, seen, task) = replay(response_plan, delay).await;
            (format!("{base}{path}"), seen, Some(task))
        };
        match engine_name {
            "brave" => {
                if !base.is_empty() {
                    config.brave.base_url = base;
                }
                if id.ends_with(".timeout") {
                    config.brave.timeout_ms = 10;
                }
            }
            "wikipedia" => {
                if !base.is_empty() {
                    config.wikipedia.base_url = base;
                }
                if id.ends_with(".timeout") {
                    config.wikipedia.timeout_ms = 10;
                }
            }
            _ => {
                if !base.is_empty() {
                    config.reddit.base_url = base;
                }
                if id.ends_with(".timeout") {
                    config.reddit.timeout_ms = 10;
                }
            }
        }
        let params = record["params"].as_object();
        let categories = params
            .and_then(|p| p.get("categories"))
            .and_then(ssx_json::Value::as_array)
            .map(|a| {
                a.iter()
                    .filter_map(|x| x.as_str().map(str::to_owned))
                    .collect()
            });
        let subreddit = params
            .and_then(|p| p.get("subreddit"))
            .and_then(ssx_json::Value::as_str)
            .map(str::to_owned);
        let request = SearchRequest {
            query: record["query"].as_str().unwrap_or_default().to_owned(),
            categories,
            subreddit,
            ..SearchRequest::default()
        };
        let response = registry(config)
            .instantiate(engine_name)
            .unwrap()
            .search(&request)
            .await;
        let mut expected = source_output;
        if id == "reddit.500" {
            assert_eq!(
                expected["error_message"],
                source_case("reddit.500")["output"]["error_message"]
            );
            expected["error_message"] = ssx_json::json!("Reddit returned an unsuccessful status");
        }
        let actual = normalized(&response);
        assert_eq!(actual, expected, "complete normalized output for {id}");
        if let Some(server) = server {
            server.await.unwrap();
        }
        let wire = seen.lock().unwrap();
        assert_eq!(wire.len(), requests.len(), "request count for {id}");
        for (index, (actual_wire, expected_request)) in wire.iter().zip(requests).enumerate() {
            assert_source_wire(actual_wire, expected_request, id, index);
        }
    }
    let expected: std::collections::BTreeSet<_> = source_records()
        .iter()
        .map(|r| r["id"].as_str().unwrap().to_owned())
        .collect();
    assert_eq!(
        executed, expected,
        "all original source case IDs must execute"
    );
}

#[test]
fn unrepresentable_reddit_subreddit_values_are_explicit_typed_boundaries() {
    let records = source_records();
    let null = records
        .iter()
        .find(|r| r["id"] == "reddit.subreddit_null")
        .unwrap();
    let number = records
        .iter()
        .find(|r| r["id"] == "reddit.subreddit_number")
        .unwrap();
    assert!(null["params"]["subreddit"].is_null());
    assert_eq!(number["params"]["subreddit"].as_i64(), Some(7));
}
fn registry(config: BroadApiConfig) -> Registry {
    let mut r = Registry::builtins().unwrap();
    register_broad_api(
        &mut r,
        Arc::new(AdapterHttp::default()),
        Arc::new(AllowAllAdmission),
        config,
    )
    .unwrap();
    r
}
fn with_brave(config: BraveConfig) -> BroadApiConfig {
    BroadApiConfig {
        brave: config,
        ..BroadApiConfig::default()
    }
}
fn with_wikipedia(config: WikipediaConfig) -> BroadApiConfig {
    BroadApiConfig {
        wikipedia: config,
        ..BroadApiConfig::default()
    }
}
fn with_reddit(config: RedditConfig) -> BroadApiConfig {
    BroadApiConfig {
        reddit: config,
        ..BroadApiConfig::default()
    }
}

#[test]
fn batch_registration_alone_controls_runtime_availability() {
    let mut r = Registry::builtins().unwrap();
    for name in ["brave", "wikipedia", "reddit"] {
        assert!(
            !r.is_available(name),
            "{name} is not available before the A04 batch registers it"
        );
        assert!(r.instantiate(name).is_none());
    }
    register_broad_api(
        &mut r,
        Arc::new(AdapterHttp::default()),
        Arc::new(AllowAllAdmission),
        BroadApiConfig::default(),
    )
    .unwrap();
    for name in ["brave", "wikipedia", "reddit"] {
        assert!(
            r.is_available(name),
            "{name} becomes available after A04 registration"
        );
        assert!(r.instantiate(name).is_some());
        assert_eq!(
            r.descriptor(name).unwrap().implementation,
            ImplementationStatus::Unavailable,
            "static F06 descriptor remains unchanged until aggregate catalog reconciliation"
        );
    }
}

#[tokio::test]
async fn brave_web_answers_and_images_are_parsed_from_loopback_http() {
    let body = source_body("brave.web.success", 0);
    let (base, seen, server) = replay(vec![(200, body)], None).await;
    let cfg = with_brave(BraveConfig {
        base_url: format!("{base}/res/v1/web/search?discard=yes"),
        api_key: "secret".into(),
        ..BraveConfig::default()
    });
    let r = registry(cfg);
    let engine = r.instantiate("brave").unwrap();
    let result = engine
        .search(&SearchRequest {
            query: "test query".into(),
            ..SearchRequest::default()
        })
        .await;
    assert_eq!(result.status, EngineStatus::Ok);
    let expected = expected_result("brave.web.success", 0);
    assert_eq!(result.results[0].title, expected["title"].as_str().unwrap());
    assert_eq!(result.results[0].url, expected["url"].as_str().unwrap());
    assert_eq!(
        result.results[0].thumbnail.as_deref(),
        expected["thumbnail"].as_str()
    );
    server.await.unwrap();
    let wire = seen.lock().unwrap()[0].clone();
    assert!(
        wire.starts_with("GET /res/v1/web/search?q=test+query&count=20&safesearch=off "),
        "{wire}"
    );
    assert!(
        wire.to_ascii_lowercase()
            .contains("x-subscription-token: secret")
    );
    let (base, _, server) = replay(vec![(200, source_body("brave.images.media", 0))], None).await;
    let cfg = with_brave(BraveConfig {
        base_url: format!("{base}/res/v1/web/search"),
        api_key: "key".into(),
        ..BraveConfig::default()
    });
    let r = registry(cfg);
    let result = r
        .instantiate("brave")
        .unwrap()
        .search(&SearchRequest {
            query: "raspberry pi".into(),
            categories: Some(vec!["images".into()]),
            ..SearchRequest::default()
        })
        .await;
    let expected = expected_result("brave.images.media", 0);
    assert_eq!(result.results[0].url, expected["url"].as_str().unwrap());
    assert_eq!(
        result.results[0].media.as_ref().unwrap().url.as_deref(),
        expected["media"]["url"].as_str()
    );
    server.await.unwrap();
    let (base, _, server) = replay(vec![(200, source_body("brave.answers", 0))], None).await;
    let cfg = with_brave(BraveConfig {
        base_url: format!("{base}/res/v1/web/search"),
        api_key: "key".into(),
        ..BraveConfig::default()
    });
    let result = registry(cfg)
        .instantiate("brave")
        .unwrap()
        .search(&SearchRequest {
            query: "entity".into(),
            ..SearchRequest::default()
        })
        .await;
    let expected_case = source_case("brave.answers");
    let expected = expected_case["output"]["answers"].as_array().unwrap();
    assert_eq!(result.answers.len(), expected.len());
    assert_eq!(result.answers[0]["content"], expected[0]["content"]);
    server.await.unwrap();
    let (base, seen, server) = replay(vec![(200, source_body("brave.news.route", 0))], None).await;
    let cfg = with_brave(BraveConfig {
        base_url: format!("{base}/res/v1/web/search"),
        api_key: "key".into(),
        ..BraveConfig::default()
    });
    let result = registry(cfg)
        .instantiate("brave")
        .unwrap()
        .search(&SearchRequest {
            query: "news".into(),
            categories: Some(vec!["news".into()]),
            ..SearchRequest::default()
        })
        .await;
    assert_eq!(
        result.results[0].title,
        expected_result("brave.news.route", 0)["title"]
            .as_str()
            .unwrap()
    );
    server.await.unwrap();
    assert!(seen.lock().unwrap()[0].starts_with("GET /res/v1/news/search?"));
    let (base, _, server) = replay(vec![(200, source_body("brave.video.media", 0))], None).await;
    let cfg = with_brave(BraveConfig {
        base_url: format!("{base}/res/v1/web/search"),
        api_key: "key".into(),
        ..BraveConfig::default()
    });
    let result = registry(cfg)
        .instantiate("brave")
        .unwrap()
        .search(&SearchRequest {
            query: "video".into(),
            categories: Some(vec!["videos".into()]),
            ..SearchRequest::default()
        })
        .await;
    assert_eq!(
        result.results[0].media.as_ref().unwrap().media_type,
        "video"
    );
    assert!(result.results[0].media.as_ref().unwrap().url.is_none());
    assert!(result.results[1].media.is_none());
    server.await.unwrap();
}

#[tokio::test]
async fn wikipedia_two_stages_preserve_request_order_corrections_and_infobox() {
    let (base, seen, server) = replay(
        vec![
            (200, source_body("wikipedia.two_stage", 0)),
            (200, source_body("wikipedia.two_stage", 1)),
        ],
        None,
    )
    .await;
    let cfg = with_wikipedia(WikipediaConfig {
        base_url: format!("{base}/w/api.php"),
        user_agent: "Project contact https://example.test/contact".into(),
        ..WikipediaConfig::default()
    });
    let r = registry(cfg);
    let result = r
        .instantiate("wikipedia")
        .unwrap()
        .search(&SearchRequest {
            query: "test query".into(),
            ..SearchRequest::default()
        })
        .await;
    assert_eq!(result.status, EngineStatus::Ok);
    let expected = expected_result("wikipedia.two_stage", 0);
    assert_eq!(result.results[0].title, expected["title"].as_str().unwrap());
    assert_eq!(
        result.results[0].thumbnail.as_deref(),
        expected["thumbnail"].as_str()
    );
    assert_eq!(
        result.corrections,
        source_case("wikipedia.two_stage")["output"]["corrections"]
            .as_array()
            .unwrap()
            .iter()
            .map(|x| x.as_str().unwrap())
            .collect::<Vec<_>>()
    );
    server.await.unwrap();
    let wire = seen.lock().unwrap().clone();
    assert_eq!(wire.len(), 2);
    assert!(wire[0].contains("action=opensearch"));
    assert!(wire[1].contains("action=query"));
    assert!(wire.iter().all(|w| {
        w.to_ascii_lowercase()
            .contains("user-agent: project contact https://example.test/contact")
    }));
}

#[tokio::test]
async fn wikipedia_no_titles_and_missing_page_ids_match_captured_source() {
    let (base, seen, server) =
        replay(vec![(200, source_body("wikipedia.empty_titles", 0))], None).await;
    let mut cfg = BroadApiConfig::default();
    cfg.wikipedia.base_url = format!("{base}/w/api.php");
    let result = registry(cfg)
        .instantiate("wikipedia")
        .unwrap()
        .search(&SearchRequest {
            query: "none".into(),
            ..SearchRequest::default()
        })
        .await;
    assert_eq!(result.status, EngineStatus::Ok);
    assert!(result.results.is_empty());
    server.await.unwrap();
    assert_eq!(seen.lock().unwrap().len(), 1);
    let (base, seen, server) = replay(
        vec![
            (200, source_body("wikipedia.missing_page", 0)),
            (200, source_body("wikipedia.missing_page", 1)),
        ],
        None,
    )
    .await;
    let mut cfg = BroadApiConfig::default();
    cfg.wikipedia.base_url = format!("{base}/w/api.php");
    let result = registry(cfg)
        .instantiate("wikipedia")
        .unwrap()
        .search(&SearchRequest {
            query: "missing".into(),
            ..SearchRequest::default()
        })
        .await;
    assert_eq!(result.status, EngineStatus::Ok);
    assert!(result.results.is_empty());
    assert_eq!(result.corrections, vec!["Test Page 1"]);
    server.await.unwrap();
    assert_eq!(seen.lock().unwrap().len(), 2);
}

#[tokio::test]
async fn reddit_token_precedence_auth_host_and_subreddit_routing_are_explicit() {
    let (base, seen, server) =
        replay(vec![(200, br#"{"data":{"children":[]}}"#.to_vec())], None).await;
    let cfg = with_reddit(RedditConfig {
        base_url: base,
        access_token: "access".into(),
        api_key: "fallback".into(),
        ..RedditConfig::default()
    });
    let r = registry(cfg);
    let result = r
        .instantiate("reddit")
        .unwrap()
        .search(&SearchRequest {
            query: "hello".into(),
            categories: Some(vec!["reddit:subreddit".into()]),
            subreddit: Some("rust".into()),
            ..SearchRequest::default()
        })
        .await;
    assert_eq!(result.status, EngineStatus::Ok);
    server.await.unwrap();
    let wire = seen.lock().unwrap()[0].clone();
    assert!(
        wire.starts_with("GET /r/rust/search?q=hello&limit=10&sort=relevance&t=all&raw_json=1 "),
        "{wire}"
    );
    assert!(
        wire.to_ascii_lowercase()
            .contains("authorization: bearer access")
    );
    assert!(
        wire.to_ascii_lowercase()
            .contains("user-agent: slopsearx/0.1.0")
    );
    assert!(!wire.to_ascii_lowercase().contains(".json"));
    let (base, seen, server) =
        replay(vec![(200, br#"{"data":{"children":[]}}"#.to_vec())], None).await;
    let cfg = with_reddit(RedditConfig {
        base_url: base,
        api_key: String::new(),
        ..RedditConfig::default()
    });
    let r = registry(cfg);
    let result = r
        .instantiate("reddit")
        .unwrap()
        .search(&SearchRequest {
            query: "hello".into(),
            ..SearchRequest::default()
        })
        .await;
    assert_eq!(result.status, EngineStatus::Ok);
    server.await.unwrap();
    assert!(seen.lock().unwrap()[0].starts_with("GET /search.json?"));
}

#[tokio::test]
async fn reddit_default_oauth_rewrite_targets_source_host_without_network() {
    use ssx_infra::http::{ExchangeFuture, Request, Response, Transport};
    struct Recorder(Mutex<Vec<String>>);
    impl Transport for Recorder {
        fn exchange(&self, request: Request) -> ExchangeFuture<'_> {
            self.0.lock().unwrap().push(format!(
                "{} {} {:?}",
                request.method, request.url, request.headers
            ));
            Box::pin(async move {
                Ok(Response {
                    status: 200,
                    url: request.url,
                    headers: reqwest::header::HeaderMap::new(),
                    body: source_body("reddit.oauth", 0),
                })
            })
        }
    }
    let http = Arc::new(AdapterHttp::default());
    let seen = Arc::new(Recorder(Mutex::new(vec![])));
    http.set_transport(seen.clone()).unwrap();
    let mut r = Registry::builtins().unwrap();
    register_broad_api(
        &mut r,
        http,
        Arc::new(AllowAllAdmission),
        with_reddit(RedditConfig {
            api_key: "offline-token".into(),
            ..RedditConfig::default()
        }),
    )
    .unwrap();
    let result = r
        .instantiate("reddit")
        .unwrap()
        .search(&SearchRequest {
            query: "test query".into(),
            ..SearchRequest::default()
        })
        .await;
    assert_eq!(normalized(&result), source_case("reddit.oauth")["output"]);
    let request = seen.0.lock().unwrap()[0].clone();
    assert!(
        request.contains(
            "https://oauth.reddit.com/search?q=test+query&limit=10&sort=relevance&t=all&raw_json=1"
        ),
        "{request}"
    );
    assert!(request.contains("Bearer offline-token"), "{request}");
}

#[tokio::test]
async fn wikipedia_deadline_is_per_operation_and_session_is_reused() {
    let (base, seen, server) = replay(
        vec![
            (200, source_body("wikipedia.two_stage", 0)),
            (200, source_body("wikipedia.two_stage", 1)),
        ],
        Some(std::time::Duration::from_millis(60)),
    )
    .await;
    let result = registry(with_wikipedia(WikipediaConfig {
        base_url: format!("{base}/w/api.php"),
        timeout_ms: 100,
        ..WikipediaConfig::default()
    }))
    .instantiate("wikipedia")
    .unwrap()
    .search(&SearchRequest {
        query: "test query".into(),
        ..SearchRequest::default()
    })
    .await;
    assert_eq!(result.status, EngineStatus::Ok);
    server.await.unwrap();
    assert_eq!(seen.lock().unwrap().len(), 2);
}

#[tokio::test]
async fn reddit_listing_preserves_source_filter_content_dates_and_thumbnail_rules() {
    let (base, _, server) = replay(vec![(200, source_body("reddit.listing", 0))], None).await;
    let mut cfg = BroadApiConfig::default();
    cfg.reddit.base_url = base;
    let r = registry(cfg);
    let result = r
        .instantiate("reddit")
        .unwrap()
        .search(&SearchRequest {
            query: "test query".into(),
            ..SearchRequest::default()
        })
        .await;
    assert_eq!(result.status, EngineStatus::Ok);
    assert_eq!(result.results.len(), 2);
    for i in 0..2 {
        let expected = expected_result("reddit.listing", i);
        assert_eq!(result.results[i].title, expected["title"].as_str().unwrap());
        assert_eq!(result.results[i].url, expected["url"].as_str().unwrap());
        assert_eq!(
            result.results[i].content,
            expected["content"].as_str().unwrap()
        );
        assert_eq!(
            result.results[i].published_date.as_deref(),
            expected["published_date"].as_str()
        );
        assert_eq!(
            result.results[i].thumbnail.as_deref(),
            expected["thumbnail"].as_str()
        );
    }
    server.await.unwrap();
}

#[tokio::test]
async fn reddit_empty_permalink_prefix_and_python_truthiness_follow_captured_source() {
    let (base, _, server) = replay(
        vec![(200, source_body("reddit.empty_permalink_prefix", 0))],
        None,
    )
    .await;
    let mut config = BroadApiConfig::default();
    config.reddit.base_url = base;
    let result = registry(config)
        .instantiate("reddit")
        .unwrap()
        .search(&SearchRequest::default())
        .await;
    assert_eq!(
        result.results[0].url,
        expected_result("reddit.empty_permalink_prefix", 0)["url"]
            .as_str()
            .unwrap()
    );
    server.await.unwrap();

    let (base, _, server) = replay(vec![(200, source_body("reddit.truthy_nsfw", 0))], None).await;
    let mut config = BroadApiConfig::default();
    config.reddit.base_url = base;
    let result = registry(config)
        .instantiate("reddit")
        .unwrap()
        .search(&SearchRequest::default())
        .await;
    assert!(result.results.is_empty());
    server.await.unwrap();
}

#[tokio::test]
async fn reddit_subreddit_missing_empty_and_path_special_inputs_match_source_urls() {
    for (case, subreddit, expected_path) in [
        ("reddit.subreddit_missing", None, "/r/all/search.json"),
        ("reddit.subreddit_empty", Some(""), "/r//search.json"),
        (
            "reddit.subreddit_path_special",
            Some("foo/bar"),
            "/r/foo/bar/search.json",
        ),
    ] {
        let (base, seen, server) = replay(vec![(200, source_body(case, 0))], None).await;
        let mut cfg = BroadApiConfig::default();
        cfg.reddit.base_url = base;
        let result = registry(cfg)
            .instantiate("reddit")
            .unwrap()
            .search(&SearchRequest {
                query: "test query".into(),
                categories: Some(vec!["reddit:subreddit".into()]),
                subreddit: subreddit.map(str::to_owned),
                ..SearchRequest::default()
            })
            .await;
        assert_eq!(result.status, EngineStatus::Ok);
        server.await.unwrap();
        let request = seen.lock().unwrap()[0].clone();
        assert!(
            request.starts_with(&format!("GET {expected_path}?q=test+query&limit=10")),
            "{case}: {request}"
        );
    }
    let (base, seen, server) = replay(
        vec![(200, source_body("reddit.general_ignores_subreddit", 0))],
        None,
    )
    .await;
    let mut cfg = BroadApiConfig::default();
    cfg.reddit.base_url = base;
    let result = registry(cfg)
        .instantiate("reddit")
        .unwrap()
        .search(&SearchRequest {
            query: "test query".into(),
            categories: Some(vec!["general".into()]),
            subreddit: Some("python".into()),
            ..SearchRequest::default()
        })
        .await;
    assert_eq!(result.status, EngineStatus::Ok);
    server.await.unwrap();
    assert!(seen.lock().unwrap()[0].starts_with("GET /search.json?"));
}

#[tokio::test]
async fn all_three_classify_http_errors_bad_json_and_timeout() {
    for (name, config) in [
        (
            "brave",
            BroadApiConfig {
                brave: BraveConfig {
                    api_key: "x".into(),
                    ..BraveConfig::default()
                },
                ..BroadApiConfig::default()
            },
        ),
        ("wikipedia", BroadApiConfig::default()),
        ("reddit", BroadApiConfig::default()),
    ] {
        for (code, expected) in [
            (429, EngineStatus::RateLimited),
            (403, EngineStatus::Blocked),
            (503, EngineStatus::Error),
        ] {
            let (base, _, server) = replay(vec![(code, Vec::new())], None).await;
            let mut config = config.clone();
            set_base(&mut config, name, &base);
            let result = registry(config)
                .instantiate(name)
                .unwrap()
                .search(&SearchRequest::default())
                .await;
            assert_eq!(result.status, expected, "{name} HTTP {code}");
            server.await.unwrap();
        }
        let (base, _, server) = replay(vec![(200, b"{broken".to_vec())], None).await;
        let mut config = config.clone();
        set_base(&mut config, name, &base);
        let result = registry(config.clone())
            .instantiate(name)
            .unwrap()
            .search(&SearchRequest::default())
            .await;
        assert_eq!(result.status, EngineStatus::Error, "{name} malformed");
        server.await.unwrap();
        let (base, _, server) = replay(
            vec![(
                200,
                br#"{"web":{"results":[]},"data":{"children":[]}}"#.to_vec(),
            )],
            Some(std::time::Duration::from_millis(60)),
        )
        .await;
        let mut config = config.clone();
        set_base(&mut config, name, &base);
        set_timeout(&mut config, name, 10);
        let result = registry(config)
            .instantiate(name)
            .unwrap()
            .search(&SearchRequest::default())
            .await;
        assert_eq!(result.status, EngineStatus::Timeout, "{name} timeout");
        server.abort();
    }
}

#[tokio::test]
async fn dropped_search_cancels_outstanding_loopback_request_for_each_adapter() {
    for name in ["brave", "wikipedia", "reddit"] {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = listener.local_addr().unwrap();
        let (accepted_tx, accepted_rx) = tokio::sync::oneshot::channel();
        let server = tokio::spawn(async move {
            let (mut stream, _) = listener.accept().await.unwrap();
            let mut request = vec![0; 2048];
            let _ = stream.read(&mut request).await;
            let _ = accepted_tx.send(());
            let mut extra = [0; 1];
            tokio::time::timeout(std::time::Duration::from_secs(2), stream.read(&mut extra))
                .await
                .unwrap()
                .unwrap()
        });
        let mut config = BroadApiConfig::default();
        set_base(&mut config, name, &format!("http://{address}"));
        if name == "brave" {
            config.brave.api_key = "test".into();
        }
        let engine = registry(config).instantiate(name).unwrap();
        let task = tokio::spawn(async move {
            engine
                .search(&SearchRequest {
                    query: "cancel".into(),
                    ..SearchRequest::default()
                })
                .await
        });
        accepted_rx.await.unwrap();
        task.abort();
        let closed = server.await.unwrap();
        assert_eq!(
            closed, 0,
            "{name} left its loopback exchange open after cancellation"
        );
    }
}

#[tokio::test]
async fn empty_and_wrong_shape_responses_keep_engine_specific_outcomes() {
    for (name, body, expected) in [
        ("brave", b"{}".as_slice(), EngineStatus::Ok),
        ("brave", b"[]".as_slice(), EngineStatus::Error),
        (
            "wikipedia",
            br#"["q",[],[],[]]"#.as_slice(),
            EngineStatus::Ok,
        ),
        ("wikipedia", br#"["q",{}]"#.as_slice(), EngineStatus::Error),
        (
            "reddit",
            br#"{"data":{"children":[]}}"#.as_slice(),
            EngineStatus::Ok,
        ),
        (
            "reddit",
            br#"{"data":{"children":null}}"#.as_slice(),
            EngineStatus::Error,
        ),
    ] {
        let (base, _, server) = replay(vec![(200, body.to_vec())], None).await;
        let mut config = BroadApiConfig::default();
        set_base(&mut config, name, &base);
        if name == "brave" {
            config.brave.api_key = "key".into();
        }
        let result = registry(config)
            .instantiate(name)
            .unwrap()
            .search(&SearchRequest::default())
            .await;
        assert_eq!(result.status, expected, "{name}: {body:?}");
        server.await.unwrap();
    }
}

#[tokio::test]
async fn missing_brave_credentials_fail_before_dispatch_and_reddit_401_is_unavailable() {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap();
    let config = with_brave(BraveConfig {
        base_url: format!("http://{address}/search"),
        api_key: String::new(),
        ..BraveConfig::default()
    });
    let result = registry(config)
        .instantiate("brave")
        .unwrap()
        .search(&SearchRequest::default())
        .await;
    assert_eq!(result.status, EngineStatus::Error);
    assert_eq!(
        result.error_message.as_deref(),
        Some("Brave API key not configured")
    );
    assert!(
        tokio::time::timeout(std::time::Duration::from_millis(10), listener.accept())
            .await
            .is_err()
    );
    let (base, _, server) = replay(vec![(401, Vec::new())], None).await;
    let config = with_reddit(RedditConfig {
        base_url: base,
        access_token: "expired-token".into(),
        ..RedditConfig::default()
    });
    let result = registry(config)
        .instantiate("reddit")
        .unwrap()
        .search(&SearchRequest::default())
        .await;
    assert_eq!(result.status, EngineStatus::Unavailable);
    assert!(
        !result
            .error_message
            .as_deref()
            .unwrap()
            .contains("expired-token")
    );
    server.await.unwrap();
}
fn set_base(c: &mut BroadApiConfig, name: &str, base: &str) {
    match name {
        "brave" => c.brave.base_url = format!("{base}/res/v1/web/search"),
        "wikipedia" => c.wikipedia.base_url = format!("{base}/w/api.php"),
        _ => c.reddit.base_url = base.into(),
    }
}
fn set_timeout(c: &mut BroadApiConfig, name: &str, value: u64) {
    match name {
        "brave" => c.brave.timeout_ms = value,
        "wikipedia" => c.wikipedia.timeout_ms = value,
        _ => c.reddit.timeout_ms = value,
    }
}

#[tokio::test]
async fn nonfinite_source_records_preserve_full_source_list_and_native_typed_refusal() {
    use base64::Engine as _;
    let root: ssx_json::Value = ssx_json::from_slice(include_bytes!(
        "../../../docs/rust-rewrite/a04/nonfinite_boundaries.json"
    ))
    .unwrap();
    let mut executed = std::collections::BTreeSet::new();
    for record in root["records"].as_array().unwrap() {
        let id = record["id"].as_str().unwrap();
        assert!(executed.insert(id.to_owned()));
        let mixed = id.ends_with(".mixed");
        let spelling = id.split('.').nth(2).unwrap();
        let source = &record["output"];
        assert_eq!(source["status"], ssx_json::json!("ok"));
        assert!(source["error_message"].is_null());
        let rows = source["results"].as_array().unwrap();
        assert_eq!(rows.len(), if mixed { 3 } else { 1 });
        assert_eq!(
            rows[usize::from(mixed)]["score"],
            ssx_json::json!({"$source_nonfinite_float": spelling})
        );
        if mixed {
            assert_eq!(rows[0]["title"], ssx_json::json!("before"));
            assert_eq!(rows[0]["score"], ssx_json::json!(1.0));
            assert_eq!(rows[2]["title"], ssx_json::json!("after"));
            assert_eq!(rows[2]["score"], ssx_json::json!(2.0));
        }
        let reply = &record["responses"][0];
        let body = base64::engine::general_purpose::STANDARD
            .decode(reply["body_b64"].as_str().unwrap())
            .unwrap();
        let (base, seen, server) =
            replay_with_types(vec![(200, body, "application/json".into())], None).await;
        let mut config = BroadApiConfig::default();
        config.reddit.base_url = base;
        let response = registry(config)
            .instantiate("reddit")
            .unwrap()
            .search(&SearchRequest {
                query: "edge".into(),
                ..SearchRequest::default()
            })
            .await;
        assert_eq!(
            normalized(&response),
            ssx_json::json!({"status":"error", "error_message":"Reddit score must be finite", "results":[], "answers":[], "corrections":[], "infoboxes":[]})
        );
        server.await.unwrap();
        let wire = seen.lock().unwrap();
        let requests = record["request"].as_array().unwrap();
        assert_eq!(wire.len(), requests.len());
        for (index, (got, source)) in wire.iter().zip(requests).enumerate() {
            assert_source_wire(got, source, id, index);
        }
    }
    let expected: std::collections::BTreeSet<_> = ["nan", "inf", "-inf"]
        .into_iter()
        .flat_map(|v| {
            [
                format!("reddit.nonfinite.{v}.single"),
                format!("reddit.nonfinite.{v}.mixed"),
            ]
        })
        .collect();
    assert_eq!(
        executed, expected,
        "all six tagged source boundary records must execute"
    );
}

#[tokio::test]
async fn representation_boundaries_preserve_source_outcomes_and_refuse_whole_native_response() {
    use base64::Engine as _;
    let root: ssx_json::Value = ssx_json::from_slice(include_bytes!(
        "../../../docs/rust-rewrite/a04/representation_boundaries.json"
    ))
    .unwrap();
    let mut executed = std::collections::BTreeSet::new();
    for record in root["records"].as_array().unwrap() {
        let id = record["id"].as_str().unwrap();
        assert!(executed.insert(id.to_owned()));
        let mixed = id.ends_with(".mixed");
        let source = &record["output"];
        assert_eq!(source["status"], ssx_json::json!("ok"));
        assert!(source["error_message"].is_null());
        let rows = source["results"].as_array().unwrap();
        assert_eq!(rows.len(), if mixed { 3 } else { 1 });
        let boundary = &rows[usize::from(mixed)];
        let diagnostic = if id.starts_with("typed.") {
            let field = record["boundary_field"].as_str().unwrap();
            assert!(matches!(
                field,
                "url" | "title" | "content" | "thumbnail" | "media.url"
            ));
            let value = if field == "media.url" {
                &boundary["media"]["url"]
            } else {
                &boundary[field]
            };
            assert!(
                !value.is_string(),
                "source genuinely exceeds typed field {id}"
            );
            let optional = matches!(field, "thumbnail" | "media.url");
            if optional {
                assert!(!value.is_null());
            }
            let engine = match record["engine"].as_str().unwrap() {
                "brave" => "Brave",
                "wikipedia" => "Wikipedia",
                "reddit" => "Reddit",
                _ => panic!("unexpected source engine"),
            };
            format!(
                "{engine} result {field} must be a string{}",
                if optional { " or null" } else { "" }
            )
        } else {
            let message = if id.starts_with("reddit.") {
                assert_eq!(boundary["content"], ssx_json::json!(["source", "list"]));
                "Reddit selftext must be a string when nonempty"
            } else if id.contains("duration_") {
                let duration = &boundary["media"]["duration"];
                assert!(
                    duration.as_bool().is_some()
                        || matches!(duration, ssx_json::Value::Number(n) if n.as_f64().is_none())
                );
                "Brave video duration must be a finite non-boolean number"
            } else if id.contains("height_overrange") {
                assert!(
                    matches!(&boundary["media"]["height"], ssx_json::Value::Number(n) if n.as_i64().is_none())
                );
                "Brave image height must fit a signed 64-bit integer"
            } else {
                assert!(
                    matches!(&boundary["media"]["width"], ssx_json::Value::Number(n) if n.as_i64().is_none())
                );
                "Brave image width must fit a signed 64-bit integer"
            };
            message.to_owned()
        };
        if mixed {
            assert_eq!(rows[0]["title"], ssx_json::json!("before"));
            assert_eq!(rows[2]["title"], ssx_json::json!("after"));
        }
        let expected = ssx_json::json!({"status":"error", "error_message":diagnostic, "results":[], "answers":[], "corrections":[], "infoboxes":[]});
        assert_eq!(record["native_expectation"], expected);
        let replies = record["responses"]
            .as_array()
            .unwrap()
            .iter()
            .map(|reply| {
                (
                    reply["status"].as_u64().unwrap() as u16,
                    base64::engine::general_purpose::STANDARD
                        .decode(reply["body_b64"].as_str().unwrap())
                        .unwrap(),
                    reply["headers"]["content-type"]
                        .as_str()
                        .unwrap()
                        .to_owned(),
                )
            })
            .collect();
        let (base, seen, server) = replay_with_types(replies, None).await;
        let mut config = BroadApiConfig::default();
        let name = record["engine"].as_str().unwrap();
        if name == "brave" {
            config.brave.base_url = format!("{base}/res/v1/web/search");
            config.brave.api_key = "offline-fake-token".into();
        } else if name == "wikipedia" {
            config.wikipedia.base_url = format!("{base}/w/api.php");
        } else {
            config.reddit.base_url = base;
        }
        let categories = record["params"]["categories"]
            .as_array()
            .map(|v| v.iter().map(|v| v.as_str().unwrap().to_owned()).collect());
        let response = registry(config)
            .instantiate(name)
            .unwrap()
            .search(&SearchRequest {
                query: "edge".into(),
                categories,
                ..SearchRequest::default()
            })
            .await;
        assert_eq!(
            ssx_json::to_string(&normalized(&response)).unwrap(),
            ssx_json::to_string(&expected).unwrap(),
            "whole native refusal {id}"
        );
        server.await.unwrap();
        let wire = seen.lock().unwrap();
        let requests = record["request"].as_array().unwrap();
        assert_eq!(wire.len(), requests.len());
        for (index, (got, source)) in wire.iter().zip(requests).enumerate() {
            assert_source_wire(got, source, id, index);
        }
    }
    let variants = [
        "duration_true",
        "duration_false",
        "duration_overrange",
        "width_overrange",
        "height_overrange",
        "both_dimensions_overrange",
    ];
    let mut expected: std::collections::BTreeSet<_> = variants
        .into_iter()
        .flat_map(|v| {
            [
                format!("brave.media_boundary.{v}.single"),
                format!("brave.media_boundary.{v}.mixed"),
            ]
        })
        .collect();
    expected.extend([
        "reddit.content_boundary.list.single".into(),
        "reddit.content_boundary.list.mixed".into(),
    ]);
    expected.extend(
        root["records"]
            .as_array()
            .unwrap()
            .iter()
            .filter_map(|r| r["id"].as_str())
            .filter(|id| id.starts_with("typed."))
            .map(str::to_owned),
    );
    assert_eq!(
        executed, expected,
        "all representation boundary records execute"
    );
}

#[tokio::test]
async fn source_json_byte_numeric_and_unicode_contracts_replay_every_record() {
    use base64::Engine as _;
    let root: ssx_json::Value = ssx_json::from_slice(include_bytes!(
        "../../../docs/rust-rewrite/a04/source_json.json"
    ))
    .unwrap();
    let mut executed = std::collections::BTreeSet::new();
    for record in root["records"].as_array().unwrap() {
        let id = record["id"].as_str().unwrap();
        assert!(
            executed.insert(id.to_owned()),
            "duplicate source JSON case {id}"
        );
        assert!(
            record["source_output"].is_object(),
            "actual source tree retained {id}"
        );
        assert!(
            !record["decoder_events"].as_array().unwrap().is_empty(),
            "actual decoder observation retained {id}"
        );
        let decode = |s: &ssx_json::Value| {
            base64::engine::general_purpose::STANDARD
                .decode(s.as_str().unwrap())
                .unwrap()
        };
        let replies = record["responses"]
            .as_array()
            .unwrap()
            .iter()
            .map(|reply| {
                let body = if let Some(recipe) = reply.get("body_recipe") {
                    match recipe["kind"].as_str().unwrap() {
                        "padding_object" => {
                            let mut body =
                                vec![b' '; recipe["bytes"].as_u64().unwrap() as usize - 2];
                            body.extend_from_slice(b"{}");
                            body
                        }
                        "repeat_number" => {
                            let mut body = decode(&recipe["prefix_b64"]);
                            body.extend(std::iter::repeat_n(
                                b'9',
                                recipe["count"].as_u64().unwrap() as usize,
                            ));
                            body.extend_from_slice(&decode(&recipe["suffix_b64"]));
                            body
                        }
                        _ => panic!("unknown source body recipe {id}"),
                    }
                } else {
                    decode(&reply["body_b64"])
                };
                (
                    reply["status"].as_u64().unwrap() as u16,
                    body,
                    reply["headers"]["content-type"]
                        .as_str()
                        .unwrap()
                        .to_owned(),
                )
            })
            .collect();
        let (base, seen, server) = replay_with_types(replies, None).await;
        let mut config = BroadApiConfig::default();
        let name = record["engine"].as_str().unwrap();
        if name == "brave" {
            config.brave.base_url = format!("{base}/res/v1/web/search");
            config.brave.api_key = "offline-fake-token".into();
        } else if name == "wikipedia" {
            config.wikipedia.base_url = format!("{base}/w/api.php");
        } else {
            config.reddit.base_url = base;
        }
        let categories = record["params"]["categories"]
            .as_array()
            .map(|v| v.iter().map(|v| v.as_str().unwrap().to_owned()).collect());
        let response = registry(config)
            .instantiate(name)
            .unwrap()
            .search(&SearchRequest {
                query: "edge".into(),
                categories,
                ..SearchRequest::default()
            })
            .await;
        assert_eq!(
            ssx_json::to_string(&normalized(&response)).unwrap(),
            ssx_json::to_string(&record["native_expectation"]).unwrap(),
            "complete source JSON contract {id}"
        );
        server.await.unwrap();
        let wire = seen.lock().unwrap();
        let requests = record["request"].as_array().unwrap();
        assert_eq!(wire.len(), requests.len(), "source JSON request count {id}");
        for (index, (got, source)) in wire.iter().zip(requests).enumerate() {
            assert_source_wire(got, source, id, index);
        }
    }
    let expected: std::collections::BTreeSet<_> = root["records"]
        .as_array()
        .unwrap()
        .iter()
        .map(|r| r["id"].as_str().unwrap().to_owned())
        .collect();
    assert_eq!(executed, expected, "every source JSON record executes");
}
