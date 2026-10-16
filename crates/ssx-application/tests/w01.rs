use ssx_domain::{AttemptId, WallClockTimestamp, research::*};
use ssx_json::Value;
fn time(v: f64) -> WallClockTimestamp {
    WallClockTimestamp::new(v).unwrap()
}
#[test]
fn source_bound_research_record_and_budget_goldens() {
    let corpus: Value =
        ssx_json::from_str(include_str!("../../xtask/fixtures/w01/corpus.json")).unwrap();
    assert_eq!(corpus["cases"].as_array().unwrap().len(), 10);
    for case in corpus["cases"].as_array().unwrap() {
        let name = case["name"].as_str().unwrap();
        let mut job = ResearchJob::from_payload(case["input"].clone()).unwrap();
        assert_eq!(job.to_payload().unwrap(), case["initial"], "initial {name}");
        for (operation, expected) in case["operations"]
            .as_array()
            .unwrap()
            .iter()
            .zip(case["outputs"].as_array().unwrap())
        {
            let now = time(operation["now"].as_f64().unwrap_or(100.0));
            let index = operation["index"].as_u64().unwrap_or(0) as usize;
            match operation["kind"].as_str().unwrap() {
                "initialize" => {
                    let p = operation["policy"].as_array().unwrap();
                    job.initialize_budget(ResearchCeilings {
                        queries: p[0].as_u64().unwrap(),
                        engines_per_query: p[1].as_u64().unwrap(),
                        results: p[2].as_u64().unwrap(),
                    })
                    .unwrap();
                }
                "reserve" => {
                    let accepted = job
                        .reserve_attempt(
                            index,
                            AttemptId::new(operation["attempt_id"].as_str().unwrap_or("unused-id"))
                                .unwrap(),
                            now,
                        )
                        .unwrap();
                    assert_eq!(accepted, !operation["attempt_id"].is_null(), "{name}");
                }
                "finish" => {
                    let mut query = ssx_json::to_value(&job.queries[index]).unwrap();
                    for (key, value) in operation["outcome"].as_object().unwrap() {
                        query[key.as_str()] = value.clone();
                    }
                    job.queries[index] = ssx_json::from_value(query).unwrap();
                    let response =
                        operation["urls"]
                            .as_array()
                            .map(|items| ssx_domain::SearchResponse {
                                results: items
                                    .iter()
                                    .map(|url| ssx_domain::SearchResult {
                                        url: url.as_str().unwrap().into(),
                                        ..Default::default()
                                    })
                                    .collect(),
                                ..Default::default()
                            });
                    ssx_application::research::finish_attempt(
                        &mut job,
                        index,
                        response.as_ref(),
                        now,
                    )
                    .unwrap();
                }
                "recover" => {
                    let mut generated = operation["generated_ids"].as_array().unwrap().iter();
                    job.recover_orphan_attempts(now, || {
                        AttemptId::new(generated.next().unwrap().as_str().unwrap()).unwrap()
                    })
                    .unwrap();
                }
                "retry" => job.reset_retryable_queries(),
                _ => panic!("unowned operation"),
            }
            assert_eq!(
                job.to_payload().unwrap(),
                expected["job"],
                "job {name} {}",
                operation["kind"]
            );
            assert_eq!(
                ssx_json::to_value(job.budget_summary()).unwrap(),
                expected["summary"],
                "summary {name}"
            );
            assert_eq!(
                ResearchJob::from_payload(job.to_payload().unwrap()).unwrap(),
                job,
                "round trip {name}"
            );
        }
    }
}

#[test]
fn source_captured_url_identities_and_settlement() {
    let data: Value =
        ssx_json::from_str(include_str!("../../xtask/fixtures/w01/identity.json")).unwrap();
    assert_eq!(data["identities"].as_array().unwrap().len(), 382);
    for case in data["identities"].as_array().unwrap() {
        let url = case["url"].as_str().unwrap();
        assert_eq!(
            ssx_application::normalise_url(url),
            case["normalized"].as_str().unwrap()
        );
        assert_eq!(
            ssx_application::research::lead_id(url).as_str(),
            case["expected"].as_str().unwrap(),
            "{url:?}"
        );
    }
    for case in data["settlements"].as_array().unwrap() {
        let mut job = ResearchJob::from_payload(case["input"].clone()).unwrap();
        let response = case["urls"]
            .as_array()
            .map(|urls| ssx_domain::SearchResponse {
                results: urls
                    .iter()
                    .map(|url| ssx_domain::SearchResult {
                        url: url.as_str().unwrap().into(),
                        ..Default::default()
                    })
                    .collect(),
                ..Default::default()
            });
        ssx_application::research::finish_attempt(&mut job, 0, response.as_ref(), time(100.0))
            .unwrap();
        assert_eq!(
            job.to_payload().unwrap(),
            case["expected"],
            "{}",
            case["name"]
        );
        let settled = job.clone();
        assert!(
            ssx_application::research::finish_attempt(&mut job, 0, response.as_ref(), time(101.0))
                .is_err()
        );
        assert_eq!(job, settled);
    }
}

#[test]
fn lead_identity_changes_only_with_normalized_url() {
    use ssx_application::research::lead_id;
    assert_eq!(
        lead_id("https://example.com/p?utm_source=x"),
        lead_id("https://example.com/p")
    );
    assert_ne!(
        lead_id("https://EXAMPLE.com/p"),
        lead_id("https://example.com/p")
    );
    assert_ne!(
        lead_id("https://example.com/p#f"),
        lead_id("https://example.com/p")
    );
    assert_ne!(
        lead_id("https://example.com/p?a=1"),
        lead_id("https://example.com/p?a=2")
    );
    assert_ne!(
        lead_id("https://example.com/Case"),
        lead_id("https://example.com/case")
    );
    assert_eq!(
        lead_id("https://example.com/p;"),
        lead_id("https://example.com/p")
    );
    assert_ne!(
        lead_id("https://example.com/p;x"),
        lead_id("https://example.com/p")
    );
}
