//! Actual factory timer policy; original MockTransport successes stay immutable.
#[path = "support/a06_case.rs"]
mod a06_case;
use ssx_application::SearchContext;
use ssx_domain::{EngineStatus, SearchRequest};
use ssx_infra::adapters::AllowAllAdmission;
use ssx_infra::{
    adapters::{
        arxiv_expat::AtomPool,
        arxiv_runtime::{self, ArxivConfig},
        openalex_runtime::{self, OpenAlexConfig},
        scholarly_config::Scalar,
        semantic_runtime::{self, SemanticConfig},
    },
    http::{AdapterHttp, ExchangeFuture, Request, Transport},
};
use ssx_json::Value;
use std::{
    num::NonZeroUsize,
    path::PathBuf,
    sync::{
        Arc,
        atomic::{AtomicUsize, Ordering},
    },
    time::{Duration, Instant},
};
struct Stall {
    started: AtomicUsize,
    dropped: AtomicUsize,
}
struct Active<'a>(&'a AtomicUsize);
impl Drop for Active<'_> {
    fn drop(&mut self) {
        self.0.fetch_add(1, Ordering::SeqCst);
    }
}
impl Transport for Stall {
    fn exchange(&self, _: Request) -> ExchangeFuture<'_> {
        Box::pin(async move {
            self.started.fetch_add(1, Ordering::SeqCst);
            let _active = Active(&self.dropped);
            std::future::pending().await
        })
    }
}
fn scalar(v: &Value) -> Option<Scalar> {
    Some(match v {
        Value::Bool(v) => Scalar::Boolean(*v),
        Value::String(v) => Scalar::Text(Arc::from(v.as_str())),
        Value::Number(v) if !v.is_f64() => Scalar::Integer(v.as_str().parse().ok()?),
        Value::Number(v) => Scalar::Float(v.as_str().parse().ok()?),
        Value::Object(v) => Scalar::Float(match v.get("$source_nonfinite_float")?.as_str()? {
            "nan" => f64::NAN,
            "inf" => f64::INFINITY,
            "-inf" => f64::NEG_INFINITY,
            _ => return None,
        }),
        _ => return None,
    })
}
#[tokio::test]
async fn all_42_reachable_timeout_scalars_follow_reviewed_whole_operation_policy() {
    let corpus: Value = ssx_json::from_str(include_str!(
        "../../../docs/rust-rewrite/a06/source-config.json"
    ))
    .unwrap();
    let mut ids = std::collections::BTreeSet::new();
    for row in corpus["records"].as_array().unwrap() {
        let id = row["id"].as_str().unwrap();
        if !id.contains(".timeout_ms.")
            || row["native_reachability"]
                .as_str()
                .unwrap()
                .starts_with("direct-source-only")
        {
            continue;
        }
        assert!(ids.insert(id));
        let timer = scalar(&row["config"]["timeout_ms"]).unwrap();
        let millis = match &timer {
            Scalar::Boolean(v) => f64::from(u8::from(*v)),
            Scalar::Integer(v) => *v as f64,
            Scalar::Float(v) => *v,
            Scalar::Text(_) => f64::NAN,
        };
        for with_context in [false, true] {
            let pool = AtomPool::new(
                PathBuf::from("/nonexistent-timeout-no-parser-dispatch"),
                NonZeroUsize::new(1).unwrap(),
            )
            .unwrap();
            let transport = Arc::new(Stall {
                started: AtomicUsize::new(0),
                dropped: AtomicUsize::new(0),
            });
            let http = Arc::new(AdapterHttp::default());
            http.set_transport(transport.clone()).unwrap();
            let mut registry = ssx_registry::Registry::builtins().unwrap();
            let admission = Arc::new(AllowAllAdmission);
            let name = row["engine"].as_str().unwrap();
            match name {
                "arxiv" => arxiv_runtime::register(
                    &mut registry,
                    http.clone(),
                    admission,
                    ArxivConfig {
                        timeout_ms: timer.clone(),
                        ..Default::default()
                    },
                    pool.clone(),
                )
                .unwrap(),
                "openalex" => openalex_runtime::register(
                    &mut registry,
                    http.clone(),
                    admission,
                    OpenAlexConfig {
                        timeout_ms: timer.clone(),
                        ..Default::default()
                    },
                    pool.clone(),
                )
                .unwrap(),
                "semanticscholar" => semantic_runtime::register(
                    &mut registry,
                    http.clone(),
                    admission,
                    SemanticConfig {
                        timeout_ms: timer.clone(),
                        ..Default::default()
                    },
                    pool.clone(),
                )
                .unwrap(),
                _ => panic!("unexpected engine"),
            }
            let engine = registry.instantiate(name).unwrap();
            let request = SearchRequest {
                query: "offline config".into(),
                ..Default::default()
            };
            let response = if with_context {
                engine
                    .search_with_context(
                        &request,
                        SearchContext {
                            deadline: Instant::now() + Duration::from_millis(30),
                        },
                    )
                    .await
            } else {
                engine.search(&request).await
            };
            let text = matches!(timer, Scalar::Text(_));
            let nonpositive = matches!(
                millis.partial_cmp(&0.),
                Some(std::cmp::Ordering::Less | std::cmp::Ordering::Equal)
            );
            let finite_required = !text && !millis.is_finite() && !nonpositive && !with_context;
            let expected_dispatch = usize::from(!text && !finite_required && !nonpositive);
            assert_eq!(
                transport.started.load(Ordering::SeqCst),
                expected_dispatch,
                "{id} context={with_context}"
            );
            assert_eq!(
                transport.dropped.load(Ordering::SeqCst),
                expected_dispatch,
                "pending exchange released {id}"
            );
            assert!(response.results.is_empty());
            if text {
                assert_eq!(response.status, EngineStatus::Error, "{id}");
                assert_eq!(
                    response.error_message.as_deref(),
                    row["output"]["error_message"].as_str(),
                    "{id}"
                );
            } else if finite_required {
                assert_eq!(response.status, EngineStatus::Error, "{id}");
                assert!(
                    response
                        .error_message
                        .as_deref()
                        .unwrap()
                        .contains("requires a finite execution deadline")
                );
            } else {
                assert_eq!(response.status, EngineStatus::Timeout, "{id}");
                assert_eq!(
                    row["output"]["status"].as_str(),
                    Some("ok"),
                    "immutable mock result differs explicitly"
                );
            }
            assert_eq!(pool.available_slots(), 1);
            assert_eq!(pool.available_operations(), 1);
            pool.shutdown().await.unwrap();
            http.shutdown_and_wait().await;
            a06_case::completed(
                "ssx-infra/a06_timeout_config/all_42_reachable_timeout_scalars_follow_reviewed_whole_operation_policy",
                "docs/rust-rewrite/a06/source-config.json",
                id,
                if with_context {
                    "finite_context"
                } else {
                    "direct"
                },
                if text {
                    "exact_operand_error"
                } else if finite_required {
                    "finite_policy_refusal"
                } else {
                    "deadline_policy"
                },
            );
        }
    }
    assert_eq!(ids.len(), 42);
    let ledger: Value = ssx_json::from_str(include_str!(
        "../../../docs/rust-rewrite/a06/timeout-config-classification.json"
    ))
    .unwrap();
    assert_eq!(
        ids,
        ledger["unique_ids"]
            .as_array()
            .unwrap()
            .iter()
            .map(|v| v.as_str().unwrap())
            .collect()
    );
}
