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
                    let leads = operation["lead_ids"].as_array().map(|items| {
                        items
                            .iter()
                            .map(|v| LeadId::new(v.as_str().unwrap().into()).unwrap())
                            .collect::<Vec<_>>()
                    });
                    job.finish_attempt(index, leads.as_deref(), now).unwrap();
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
fn malformed_authority_and_historical_edits_fail_closed() {
    for value in [
        ssx_json::json!([]),
        ssx_json::json!({"job_id":"x","state":"unknown"}),
        ssx_json::json!({"job_id":"x","queries":[{"result_count":-1}]}),
        ssx_json::json!({"job_id":"x","queries":"not-list"}),
    ] {
        assert!(ResearchJob::from_payload(value).is_err());
    }
    let mut job = ResearchJob::from_payload(
        ssx_json::json!({"job_id":"x","tenant":"org:team","queries":[{"engines":["a"]}]}),
    )
    .unwrap();
    job.initialize_budget(ResearchCeilings {
        queries: 2,
        engines_per_query: 1,
        results: 2,
    })
    .unwrap();
    job.reserve_attempt(0, AttemptId::new("attempt-a").unwrap(), time(1.0))
        .unwrap();
    job.queries[0].state = "done".into();
    job.queries[0].cursor = Some("snap".into());
    let lead = LeadId::new(format!("lead-v1-{}", "a".repeat(64))).unwrap();
    job.finish_attempt(0, Some(&[lead.clone(), lead]), time(2.0))
        .unwrap();
    let evidence = job.queries[0].attempts[0].clone();
    assert!(job.finish_attempt(0, None, time(3.0)).is_err());
    let mut next = job.clone();
    next.queries[0].attempts[0].cursor = Some("rewritten".into());
    assert!(job.validate_transition(&next).is_err());
    let mut next = job.clone();
    next.budget_used.insert("results".into(), 0);
    assert!(job.validate_transition(&next).is_err());
    let mut next = job.clone();
    next.budget_limits.insert("attempts".into(), 3);
    assert!(job.validate_transition(&next).is_err());
    assert_eq!(job.queries[0].attempts[0], evidence);
    let prior = job.clone();
    assert!(
        job.initialize_budget(ResearchCeilings {
            queries: u64::MAX,
            engines_per_query: 2,
            results: 3
        })
        .is_err()
    );
    assert_eq!(prior, job);
}
#[test]
fn coverage_classification_source_matrix() {
    let corpus: Value =
        ssx_json::from_str(include_str!("../../xtask/fixtures/w01/corpus.json")).unwrap();
    let cases = corpus["coverage_cases"].as_array().unwrap();
    assert_eq!(cases.len(), 56);
    for case in cases {
        let p = &case["input"];
        let coverage = classify_coverage(
            p["engine"].as_str().unwrap().into(),
            p["dispatched"].as_bool().unwrap(),
            p["status"].as_str(),
            p["result_count"].as_u64().unwrap(),
            p["credential_missing"].as_bool().unwrap(),
        );
        assert_eq!(ssx_json::to_value(&coverage).unwrap(), case["coverage"]);
        assert_eq!(
            ssx_json::to_value(summarize_coverage(&[coverage]).unwrap()).unwrap(),
            case["summary"]
        );
    }
}
