//! Source-bound A08 engine qualification. No interpreter or provider invocation.
use crate::{Result, command};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use std::{
    collections::{BTreeMap, BTreeSet},
    fs,
    path::Path,
};
const COVERAGE_SHA: &str = "92d7bc5549dc6860fdb92da59ef880242ca630db639c5bd707f9f553e6bb436b";
fn digest(bytes: &[u8]) -> String {
    Sha256::digest(bytes)
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect()
}
fn require(ok: bool, message: &str) -> Result<()> {
    if ok { Ok(()) } else { Err(message.into()) }
}
fn read(root: &Path, path: &str) -> Result<Value> {
    serde_json::from_slice(&fs::read(root.join(path)).map_err(|e| e.to_string())?)
        .map_err(|e| e.to_string())
}
fn ids(pack: &Value) -> Result<Vec<String>> {
    let mut seen = BTreeSet::new();
    let mut ids = vec![];
    for row in pack["records"]
        .as_array()
        .ok_or("A08 missing source records")?
    {
        let id = row["id"]
            .as_str()
            .filter(|s| !s.is_empty())
            .ok_or("A08 empty source ID")?;
        require(seen.insert(id.to_owned()), "A08 duplicate source ID")?;
        require(
            row["responses"].is_array() && row["requests"].is_array() && row["output"].is_object(),
            "A08 source request/outcome tuple missing",
        )?;
        ids.push(id.into())
    }
    require(!ids.is_empty(), "A08 empty source pack")?;
    Ok(ids)
}
fn execution(output: &str, expected: &BTreeSet<String>) -> Result<()> {
    let passed: BTreeSet<_> = output
        .lines()
        .filter_map(|s| {
            s.strip_prefix("test ")?
                .strip_suffix(" ... ok")
                .map(str::to_owned)
        })
        .collect();
    require(
        passed == *expected
            && !output
                .lines()
                .any(|s| s.starts_with("test ") && s.ends_with(" ... ignored")),
        "A08 listed tests missing, ignored, or not passed",
    )
}
fn completed_cases(output: &str, expected: &[Value]) -> Result<()> {
    let actual = output
        .lines()
        .filter_map(|line| line.strip_prefix("A08_CASE "))
        .map(|line| serde_json::from_str::<Value>(line).map_err(|e| e.to_string()))
        .collect::<Result<Vec<_>>>()?;
    require(
        actual == expected,
        "A08 completed case IDs/kinds missing, extra, duplicate, or reordered",
    )
}
fn binding(value: &Value, available: &BTreeMap<(String, String), BTreeSet<String>>) -> Result<()> {
    let package = value["package"].as_str().ok_or("A08 package missing")?;
    let target = value["target"].as_str().ok_or("A08 target missing")?;
    let test = value["qualified_name"]
        .as_str()
        .filter(|s| !s.is_empty())
        .ok_or("A08 test name missing")?;
    require(
        available
            .get(&(package.into(), target.into()))
            .is_some_and(|names| names.contains(test)),
        "A08 exact native package/target/test identity missing",
    )
}
pub fn check(root: &Path, records: &mut Vec<Value>) -> Result<()> {
    let coverage_bytes =
        fs::read(root.join("docs/rust-rewrite/a08/coverage.json")).map_err(|e| e.to_string())?;
    require(
        digest(&coverage_bytes) == COVERAGE_SHA,
        "A08 coverage identity changed; audit and repin",
    )?;
    let coverage: Value = serde_json::from_slice(&coverage_bytes).map_err(|e| e.to_string())?;
    require(
        coverage["plan_id"] == "A08"
            && coverage["baseline"] == "2f78b9d02cfe62c9829fcfb38b85cd632a889de0",
        "A08 plan/baseline differs",
    )?;
    for kind in ["source_inventory", "artifacts"] {
        let mut seen = BTreeSet::new();
        for pin in coverage[kind]
            .as_array()
            .ok_or("A08 pin inventory missing")?
        {
            let path = pin["path"].as_str().ok_or("A08 pin path missing")?;
            require(seen.insert(path), "A08 duplicate pinned artifact")?;
            let bytes = fs::read(root.join(path)).map_err(|e| format!("{path}: {e}"))?;
            require(
                pin["sha256"].as_str() == Some(digest(&bytes).as_str()),
                &format!("A08 source/artifact changed: {path}"),
            )?;
            records.push(json!({"A08_pin":path,"sha256":digest(&bytes)}));
        }
    }
    let disposition = read(root, "docs/rust-rewrite/inventory/python-disposition.json")?;
    let invocations = read(root, "docs/rust-rewrite/inventory/python-invocations.json")?;
    let receipts = read(root, "docs/rust-rewrite/a08/oracle-runs.json")?;
    let receipt_rows = receipts["records"]
        .as_array()
        .ok_or("A08 actual oracle receipts missing")?;
    require(
        receipt_rows.len() == 4,
        "A08 exactly four temporary producers must be inventoried",
    )?;
    for receipt in receipt_rows {
        let path = receipt["producer_path"]
            .as_str()
            .ok_or("A08 producer path missing")?;
        let hash = digest(&fs::read(root.join(path)).map_err(|e| e.to_string())?);
        require(
            receipt["exit_code"] == 0 && receipt["producer_sha256"] == hash,
            "A08 source capture is not an actual successful exact producer run",
        )?;
        let entry = disposition["files"]
            .as_array()
            .ok_or("F01 dispositions missing")?
            .iter()
            .find(|r| r["path"] == path)
            .ok_or("A08 producer missing global F01 disposition")?;
        require(
            entry["sha256"] == hash
                && entry["migration_owner"] == "A08"
                && entry["migration_owners"]
                    .as_array()
                    .is_some_and(|v| v.iter().any(|x| x == "G08")),
            "A08 producer hash/retirement inventory differs",
        )?;
        require(
            invocations["developer_oracle_invocations"]
                .as_array()
                .is_some_and(|rows| {
                    rows.iter().any(|r| {
                        r["example_path"] == path
                            && r["source_sha256"] == hash
                            && r["retirement_owner"] == "G08"
                    })
                }),
            "A08 actual oracle invocation missing global retirement inventory",
        )?;
        for output in receipt["outputs"]
            .as_array()
            .ok_or("A08 capture outputs missing")?
        {
            let p = output["path"].as_str().ok_or("A08 output path missing")?;
            require(
                output["sha256"] == digest(&fs::read(root.join(p)).map_err(|e| e.to_string())?),
                "A08 capture output no longer matches execution receipt",
            )?;
        }
    }
    let unicode = read(root, "docs/rust-rewrite/a08/case16.json")?;
    require(
        unicode["records"]
            .as_array()
            .is_some_and(|rows| rows.len() == 11468)
            && unicode["unicode"] == "16.0.0",
        "A08 actual whole-string Unicode controls missing",
    )?;
    require(
        unicode["data_sha256"]
            == digest(
                &fs::read(root.join("docs/rust-rewrite/a08/case16-data.json"))
                    .map_err(|e| e.to_string())?,
            ),
        "A08 runtime Unicode data differs from source producer",
    )?;
    let source = read(root, "docs/rust-rewrite/a08/source.json")?;
    let regression = read(root, "docs/rust-rewrite/a08/regressions.json")?;
    let mut ordered = ids(&source)?;
    ordered.extend(ids(&regression)?);
    let unique: BTreeSet<_> = ordered.iter().cloned().collect();
    require(
        unique.len() == 382 && ordered.len() == 382,
        "A08 missing/duplicate primary controls",
    )?;
    require(
        coverage["ordered_case_ids"] == json!(ordered),
        "A08 ordered source controls changed",
    )?;
    let original = regression["results"]
        .as_array()
        .ok_or("A08 original test execution missing")?;
    require(
        original.len() == 47 && original.iter().all(|r| r["outcome"] == "passed"),
        "A08 original 47 parameter instances did not pass",
    )?;
    let functions: BTreeSet<_> = original
        .iter()
        .map(|r| {
            r["node"]
                .as_str()
                .unwrap_or("")
                .split('[')
                .next()
                .unwrap_or("")
        })
        .collect();
    require(
        functions.len() == 44 && !functions.contains(""),
        "A08 original 44 function mapping differs",
    )?;
    let mappings = coverage["regression_mapping"]
        .as_array()
        .ok_or("A08 regression mapping missing")?;
    let mapped: BTreeSet<_> = mappings.iter().filter_map(|r| r["node"].as_str()).collect();
    require(
        mapped == functions,
        "A08 unmapped or invented original function",
    )?;
    let differences = read(root, "docs/rust-rewrite/a08/differences.json")?;
    let mut difference_ids = BTreeSet::new();
    let mut superseded = 0;
    for row in differences["records"]
        .as_array()
        .ok_or("A08 differences missing")?
    {
        let id = row["id"].as_str().ok_or("A08 difference ID missing")?;
        require(
            unique.contains(id) && difference_ids.insert(id),
            "A08 unknown/duplicate difference",
        )?;
        require(
            [
                "native-library-diagnostic-wording",
                "finite-scalar-domain-refusal",
                "mock-timer-superseded-by-real-loopback",
                "F06-typed-config-rejection-supersession",
            ]
            .contains(&row["category"].as_str().unwrap_or("")),
            "A08 unreviewed difference category",
        )?;
        if row["category"] == "F06-typed-config-rejection-supersession" {
            superseded += 1;
            require(
                row["native_output"]["adapter_instantiated"] == false,
                "A08 malformed key must stop before instantiation",
            )?;
        }
    }
    require(
        superseded == 2 && difference_ids.len() == 72,
        "A08 exact differences/supersessions changed",
    )?;
    let mut available = BTreeMap::new();
    for (package, target) in [
        ("ssx-domain", "lib"),
        ("ssx-infra", "lib"),
        ("ssx-infra", "a08_regulatory"),
        ("ssx-infra", "a08_factories"),
        ("ssx-mcp", "a08_consumers"),
    ] {
        let mut args = vec!["test", "--offline", "--locked", "-p", package];
        if target == "lib" {
            args.push("--lib")
        } else {
            args.extend(["--test", target])
        }
        args.extend(["--", "--list"]);
        let output = command(root, "cargo", &args, records)?;
        let tests = output
            .lines()
            .filter_map(|line| line.strip_suffix(": test").map(str::to_owned))
            .collect::<BTreeSet<_>>();
        require(!tests.is_empty(), "A08 native test target empty")?;
        available.insert((package.into(), target.into()), tests);
    }
    for mapped in mappings {
        for key in mapped["native_cases"]
            .as_array()
            .ok_or("A08 native mapping missing")?
        {
            let key = key.as_str().ok_or("A08 malformed binding")?;
            binding(
                coverage["native_case_bindings"]
                    .get(key)
                    .ok_or("A08 binding missing")?,
                &available,
            )?;
        }
    }
    for (_, value) in coverage["native_case_bindings"]
        .as_object()
        .ok_or("A08 bindings missing")?
    {
        binding(value, &available)?;
    }
    let rejection_output = command(
        root,
        "cargo",
        &["test", "--offline", "--locked", "-p", "xtask", "a08::tests"],
        records,
    )?;
    execution(
        &rejection_output,
        &BTreeSet::from([
            "a08::tests::duplicate_missing_and_reordered_source_id_controls_reject".into(),
            "a08::tests::ignored_missing_and_wrong_exact_native_execution_reject".into(),
            "a08::tests::missing_duplicate_extra_reordered_or_wrong_kind_case_markers_reject"
                .into(),
            "a08::tests::wrong_package_target_and_qualified_name_reject".into(),
        ]),
    )?;
    let expected_cases:Vec<_>=ordered.iter().map(|id|json!({"id":id,"kind":if differences["records"].as_array().unwrap().iter().any(|r|r["id"]==*id&&r["category"]=="F06-typed-config-rejection-supersession"){"loader"}else{"adapter"}})).collect();
    let mut completed_adapter = 0;
    let mut completed_loader = 0;
    for ((package, target), expected) in &available {
        let mut args = vec!["test", "--offline", "--locked", "-p", package.as_str()];
        if target == "lib" {
            args.push("--lib")
        } else {
            args.extend(["--test", target.as_str()])
        }
        args.extend(["--", "--format", "pretty"]);
        if target == "a08_regulatory" {
            args.extend(["--nocapture", "--test-threads=2"]);
        }
        let output = command(root, "cargo", &args, records)?;
        execution(&output, expected)?;
        if target == "a08_regulatory" {
            completed_cases(&output, &expected_cases)?;
            for case in &expected_cases {
                if case["kind"] == "loader" {
                    completed_loader += 1
                } else {
                    completed_adapter += 1
                }
            }
        }
    }
    records.push(json!({"A08":{"source_controls":344,"original_invocations":38,"actual_adapter_replays":completed_adapter,"typed_loader_supersessions":completed_loader,"original_functions":44,"original_parameter_instances":47,"whole_string_unicode_controls":11468,"network_timeout_controls":12,"python_executed_or_required":false,"boundaries":"caller-owned F08/admission; actual compact JSON only; T03/G01 full portal HTML and T02/T06 public transport remain separate; no global RSS or deployment claim"}}));
    Ok(())
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn duplicate_missing_and_reordered_source_id_controls_reject() {
        let row = json!({"id":"one","responses":[],"requests":[],"output":{}});
        assert!(ids(&json!({"records":[]})).is_err());
        assert!(ids(&json!({"records":[row.clone(),row.clone()]})).is_err());
        assert!(ids(&json!({"records":[{"id":"one"}]})).is_err());
        let a = ids(
            &json!({"records":[row,json!({"id":"two","responses":[],"requests":[],"output":{}})]}),
        )
        .unwrap();
        let mut b = a.clone();
        b.reverse();
        assert!(require(a == b, "A08 ordered source controls changed").is_err());
    }
    #[test]
    fn ignored_missing_and_wrong_exact_native_execution_reject() {
        let expected = BTreeSet::from(["module::required".into()]);
        execution("test module::required ... ok\n", &expected).unwrap();
        for out in [
            "test module::required ... ignored\n",
            "test other::required ... ok\n",
            "test result: ok. 0 passed; 0 failed; 0 ignored\n",
        ] {
            assert!(execution(out, &expected).is_err());
        }
    }
    #[test]
    fn missing_duplicate_extra_reordered_or_wrong_kind_case_markers_reject() {
        let a = json!({"id":"ordinary","kind":"adapter"});
        let b = json!({"id":"raw-key","kind":"loader"});
        let expected = vec![a.clone(), b.clone()];
        completed_cases(&format!("\nA08_CASE {a}\nA08_CASE {b}\n"), &expected).unwrap();
        for actual in [
            vec![a.clone()],
            vec![a.clone(), a.clone(), b.clone()],
            vec![b.clone(), a.clone()],
            vec![a.clone(), json!({"id":"raw-key","kind":"adapter"})],
            vec![a.clone(), b.clone(), json!({"id":"extra","kind":"adapter"})],
        ] {
            let text = actual
                .iter()
                .map(|v| format!("\nA08_CASE {v}\n"))
                .collect::<String>();
            assert!(completed_cases(&text, &expected).is_err());
        }
    }
    #[test]
    fn wrong_package_target_and_qualified_name_reject() {
        let available = BTreeMap::from([(
            ("ssx-infra".into(), "a08_regulatory".into()),
            BTreeSet::from(["all_engine_source_replay".into()]),
        )]);
        for value in [
            json!({"package":"ssx-infra","target":"a08_regulatory","qualified_name":"wrong"}),
            json!({"package":"ssx-mcp","target":"a08_regulatory","qualified_name":"all_engine_source_replay"}),
            json!({"package":"ssx-infra","target":"wrong","qualified_name":"all_engine_source_replay"}),
        ] {
            assert!(binding(&value, &available).is_err());
        }
    }
}
