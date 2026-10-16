//! Source-bound HTML adapter qualification. No Python or provider calls.
use crate::{Result, command};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use std::{collections::BTreeSet, path::Path};
const CORPUS: &str = "4a83fb3db44894679213ba96941915dc12ad0bf00ad5cbfc691fb0410a86fc2e";
const REVIEW: &str = "63bb5265fbc7e2228b121d44823a476b544a1968bbfd823791f50cdad515e81e";
const REVIEW_2: &str = "fa247104d9829f24ec5d3ddd62c1c47cfb640e9dedd5e7edd8823fc4900302d2";
const COVERAGE: &str = "f2da311992169f60c10377251a7986ffb8695fe273b8867971fffea380b9ccb4";
const UNICODE_BOUNDARY: &str = "69e4594300a116e547a3eeca1b98a47a3f57dd3eade381bf083c1c7c57e97b4c";
fn digest(bytes: &[u8]) -> String {
    Sha256::digest(bytes)
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect()
}
fn pinned(root: &Path, path: &str, pin: &str) -> Result<Value> {
    let bytes = std::fs::read(root.join(path)).map_err(|e| e.to_string())?;
    if digest(&bytes) != pin {
        return Err(format!("A05 pinned evidence drift: {path}"));
    };
    serde_json::from_slice(&bytes).map_err(|e| e.to_string())
}
pub fn check(root: &Path, records: &mut Vec<Value>) -> Result<()> {
    let corpus = pinned(root, "docs/rust-rewrite/a05/corpus.json", CORPUS)?;
    let coverage = pinned(root, "docs/rust-rewrite/a05/coverage.json", COVERAGE)?;
    let review = pinned(root, "docs/rust-rewrite/a05/corpus-review.json", REVIEW)?;
    if review["original_corpus_sha256"] != CORPUS
        || review["source_files"] != corpus["source_files"]
        || review["source_commit"] != corpus["source_commit"]
        || review["plan_id"] != "A05"
        || coverage["review_corpus_sha256"] != REVIEW
    {
        return Err("A05 review corpus identity invalid".into());
    }
    let review_2 = pinned(root, "docs/rust-rewrite/a05/corpus-review-2.json", REVIEW_2)?;
    if review_2["source_files"] != corpus["source_files"]
        || review_2["source_commit"] != corpus["source_commit"]
        || review_2["plan_id"] != "A05"
        || coverage["review_2_corpus_sha256"] != REVIEW_2
    {
        return Err("A05 second review corpus identity invalid".into());
    }
    if corpus["plan_id"] != "A05"
        || coverage["plan_id"] != "A05"
        || corpus["source_commit"] != "01876639fb8d87123f12293c02821981cff3f8b9"
        || coverage["corpus_sha256"] != CORPUS
    {
        return Err("A05 evidence identity invalid".into());
    }
    for source in corpus["source_files"]
        .as_array()
        .ok_or("missing source pins")?
    {
        let path = source["path"].as_str().ok_or("missing path")?;
        let bytes = std::fs::read(root.join(path)).map_err(|e| e.to_string())?;
        if Some(digest(&bytes).as_str()) != source["sha256"].as_str() {
            return Err(format!("A05 source drift: {path}"));
        }
    }
    let mut cases = corpus["cases"].as_array().ok_or("missing corpus")?.clone();
    if cases.len() != 115 {
        return Err("A05 original corpus changed".into());
    }
    let appended = review["cases"].as_array().ok_or("missing review corpus")?;
    if appended.len() != 43 {
        return Err("A05 review corpus incomplete".into());
    }
    cases.extend(appended.iter().cloned());
    let second = review_2["cases"]
        .as_array()
        .ok_or("missing second review corpus")?;
    if second.len() != 28 {
        return Err("A05 second review corpus incomplete".into());
    }
    cases.extend(second.iter().cloned());
    let unicode = pinned(
        root,
        "docs/rust-rewrite/a05/surrogate-pipeline-observations.json",
        UNICODE_BOUNDARY,
    )?;
    if unicode["source_commit"] != corpus["source_commit"]
        || unicode["source_files"] != corpus["source_files"]
        || coverage["unicode_boundary_corpus_sha256"] != UNICODE_BOUNDARY
    {
        return Err("A05 Unicode boundary identity invalid".into());
    }
    let unicode_cases = unicode["cases"].as_array().ok_or("missing Unicode cases")?;
    if unicode_cases.len() != 44 {
        return Err("A05 Unicode boundary corpus incomplete".into());
    }
    cases.extend(unicode_cases.iter().cloned());
    for (file, pin, count) in [
        (
            "long-query-observations.json",
            "7cafe30cb6f30fc045e6212b5680e2ddefd0d2435a7e6e770ec928d1728116d2",
            9,
        ),
        (
            "nul-version-observations.json",
            "e2b2f7f7585a4e26b05f7bd915596396299b4305e30d691651af4b98f1c765fa",
            180,
        ),
        (
            "nul-expanded-observations.json",
            "a91ae76db2474398bcc3ffce492a0a36c01c4788287fc9c59890e75453460737",
            168,
        ),
    ] {
        let packet = pinned(root, &format!("docs/rust-rewrite/a05/{file}"), pin)?;
        if packet["source_commit"] != corpus["source_commit"]
            || packet["source_files"] != corpus["source_files"]
        {
            return Err("A05 NUL source identity invalid".into());
        }
        let rows = packet["cases"].as_array().ok_or("missing NUL cases")?;
        if rows.len() != count {
            return Err("A05 NUL case inventory incomplete".into());
        }
        cases.extend(rows.iter().cloned());
    }
    let ids: BTreeSet<_> = cases.iter().filter_map(|v| v["id"].as_str()).collect();
    let frozen_ids: BTreeSet<_> = coverage["case_ids"]
        .as_array()
        .ok_or("missing cases inventory")?
        .iter()
        .filter_map(Value::as_str)
        .collect();
    if ids.is_empty() || ids.len() != cases.len() || ids != frozen_ids {
        return Err("A05 duplicate/missing corpus case".into());
    }
    let expected: BTreeSet<_> = coverage["native_tests"]
        .as_array()
        .ok_or("missing native inventory")?
        .iter()
        .filter_map(Value::as_str)
        .collect();
    if expected.len() != 13 {
        return Err("A05 native inventory incomplete".into());
    }
    let mappings = coverage["source_tests"]
        .as_array()
        .ok_or("missing source mapping")?;
    let mut source_tests = BTreeSet::new();
    for row in mappings {
        let name = row["source_test"].as_str().ok_or("missing source test")?;
        if !source_tests.insert(name)
            || !expected.contains(
                row["native_test"]
                    .as_str()
                    .ok_or("missing native successor")?,
            )
        {
            return Err("A05 source mapping invalid".into());
        }
        for id in row["corpus_cases"]
            .as_array()
            .ok_or("missing mapped cases")?
        {
            if !ids.contains(id.as_str().ok_or("invalid mapped case")?) {
                return Err("A05 unknown mapped case".into());
            }
        }
    }
    let listed = command(
        root,
        "cargo",
        &[
            "test",
            "--locked",
            "-p",
            "ssx-infra",
            "--test",
            "a05_html",
            "--",
            "--list",
        ],
        records,
    )?;
    let actual: BTreeSet<_> = listed
        .lines()
        .filter_map(|l| l.strip_suffix(": test"))
        .collect();
    if actual != expected {
        return Err("A05 native tests missing/extra".into());
    }
    let output = command(
        root,
        "cargo",
        &[
            "test",
            "--locked",
            "-p",
            "ssx-infra",
            "--test",
            "a05_html",
            "--",
            "--nocapture",
        ],
        records,
    )?;
    let completed: BTreeSet<_> = output
        .lines()
        .filter_map(|l| l.strip_prefix("A05_CASE "))
        .collect();
    if completed != ids
        || !output.lines().any(|l| {
            l.starts_with(
                "test result: ok. 13 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out;",
            )
        })
    {
        return Err("A05 required native cases did not execute successfully".into());
    }
    let expected_units: BTreeSet<_> = coverage["native_unit_tests"]
        .as_array()
        .ok_or("missing A05 unit inventory")?
        .iter()
        .filter_map(Value::as_str)
        .collect();
    if expected_units.len() != 1 {
        return Err("A05 unit inventory incomplete".into());
    }
    let units_list = command(
        root,
        "cargo",
        &[
            "test",
            "--locked",
            "-p",
            "ssx-infra",
            "--lib",
            "html_budget::tests::",
            "--",
            "--list",
        ],
        records,
    )?;
    let listed_units: BTreeSet<_> = units_list
        .lines()
        .filter_map(|line| line.strip_suffix(": test"))
        .collect();
    if listed_units != expected_units {
        return Err("A05 unit tests missing/extra".into());
    }
    let units_output = command(
        root,
        "cargo",
        &[
            "test",
            "--locked",
            "-p",
            "ssx-infra",
            "--lib",
            "html_budget::tests::",
        ],
        records,
    )?;
    let passed_units: BTreeSet<_> = units_output
        .lines()
        .filter_map(|line| line.strip_prefix("test ")?.strip_suffix(" ... ok"))
        .collect();
    if passed_units != expected_units || !units_output.contains("1 passed; 0 failed; 0 ignored;") {
        return Err("A05 required units did not execute".into());
    }
    qualify_helper_linkage(root, records)?;
    for evidence in coverage["codec_evidence"]
        .as_array()
        .ok_or("missing codec evidence")?
    {
        let path = evidence["path"]
            .as_str()
            .ok_or("missing codec evidence path")?;
        let bytes = std::fs::read(root.join(path)).map_err(|e| e.to_string())?;
        if Some(digest(&bytes).as_str()) != evidence["sha256"].as_str() {
            return Err(format!("A05 codec evidence drift: {path}"));
        }
    }
    let expected_codec_tests: BTreeSet<_> = coverage["codec_native_tests"]
        .as_array()
        .ok_or("missing codec native tests")?
        .iter()
        .filter_map(Value::as_str)
        .collect();
    if expected_codec_tests.len() != 16 {
        return Err("A05 codec test inventory incomplete".into());
    }
    let codec_list = command(
        root,
        "cargo",
        &[
            "test",
            "--locked",
            "-p",
            "ssx-source-codecs",
            "--tests",
            "--",
            "--list",
        ],
        records,
    )?;
    let listed: BTreeSet<_> = codec_list
        .lines()
        .filter_map(|line| line.strip_suffix(": test"))
        .collect();
    if listed != expected_codec_tests {
        return Err("A05 codec native tests missing/extra".into());
    }
    let codec_output = command(
        root,
        "cargo",
        &[
            "test",
            "--locked",
            "-p",
            "ssx-source-codecs",
            "--tests",
            "--",
            "--nocapture",
        ],
        records,
    )?;
    let passed: BTreeSet<_> = codec_output
        .lines()
        .filter_map(|line| line.strip_prefix("test ")?.strip_suffix(" ... ok"))
        .collect();
    if passed != expected_codec_tests
        || codec_output
            .lines()
            .filter(|line| line.starts_with("test result:"))
            .any(|line| !line.contains("0 failed; 0 ignored;"))
    {
        return Err("A05 required codec tests did not execute successfully".into());
    }
    let policy: Value = serde_json::from_slice(
        &std::fs::read(root.join("docs/rust-rewrite/a05/source-codec-policy.json"))
            .map_err(|e| e.to_string())?,
    )
    .map_err(|e| e.to_string())?;
    let mut expected_codec_cases = BTreeSet::new();
    for row in policy["canonical_codecs"]
        .as_array()
        .ok_or("missing codec policy rows")?
    {
        let name = row["canonical"].as_str().ok_or("missing canonical codec")?;
        expected_codec_cases.insert(format!("{name}/bytes0..255"));
        for probe in row["applicability_probes"]
            .as_array()
            .ok_or("missing codec probes")?
        {
            expected_codec_cases.insert(format!(
                "{name}/{}",
                probe["input"].as_str().ok_or("missing codec probe name")?
            ));
        }
    }
    let completed_codec_cases: BTreeSet<_> = codec_output
        .lines()
        .filter_map(|line| line.strip_prefix("A05_CODEC "))
        .map(str::to_owned)
        .collect();
    if expected_codec_cases.len() != 459 || completed_codec_cases != expected_codec_cases {
        return Err("A05 source codec policy cases did not all execute".into());
    }
    let expected_aliases: BTreeSet<_> = policy["canonical_codecs"]
        .as_array()
        .ok_or("missing codec rows")?
        .iter()
        .flat_map(|row| {
            row["labels"]
                .as_array()
                .into_iter()
                .flatten()
                .filter_map(Value::as_str)
        })
        .collect();
    let completed_aliases: BTreeSet<_> = codec_output
        .lines()
        .filter_map(|line| line.strip_prefix("A05_CODEC_ALIAS "))
        .collect();
    if expected_aliases.len() != 455 || completed_aliases != expected_aliases {
        return Err("A05 source codec aliases did not all execute".into());
    }
    let registry_list = command(
        root,
        "cargo",
        &[
            "test",
            "--locked",
            "-p",
            "ssx-registry",
            "--lib",
            "--",
            "--list",
        ],
        records,
    )?;
    let registry_expected: BTreeSet<_> = [
        "tests::builtins_are_complete_and_unported_engines_are_unavailable",
        "tests::duplicate_descriptor_and_invalid_specialist_card_fail_closed",
        "tests::fixture_adapter_registers_through_the_engine_owned_seam",
        "tests::filter_consumption_and_enforcement_are_distinct_layered_metadata",
    ]
    .into_iter()
    .collect();
    let registry_actual: BTreeSet<_> = registry_list
        .lines()
        .filter_map(|line| line.strip_suffix(": test"))
        .collect();
    if registry_actual != registry_expected {
        return Err("A05 registry conformance inventory changed".into());
    }
    let registry_output = command(
        root,
        "cargo",
        &["test", "--locked", "-p", "ssx-registry", "--lib"],
        records,
    )?;
    if !registry_output.lines().any(|line| {
        line.starts_with(
            "test result: ok. 4 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out;",
        )
    }) {
        return Err("A05 registry conformance cases did not all execute".into());
    }
    records.push(json!({"contract":"A05","corpus_sha256":CORPUS,"coverage_sha256":COVERAGE,"review_corpus_sha256":REVIEW,"review_2_corpus_sha256":REVIEW_2,"executed_cases":completed,"native_tests":expected,"source_tests":source_tests,"scope":"offline HTML adapters through F06 factories/F08 sessions; explicit host-only cookie correction; no live provider or deployment claims","status":"passed"}));
    Ok(())
}

fn qualify_helper_linkage(root: &Path, records: &mut Vec<Value>) -> Result<()> {
    let metadata = command(
        root,
        "cargo",
        &["metadata", "--locked", "--no-deps", "--format-version", "1"],
        records,
    )?;
    let metadata: Value = serde_json::from_str(&metadata).map_err(|e| e.to_string())?;
    let helper = std::path::PathBuf::from(
        metadata["target_directory"]
            .as_str()
            .ok_or("missing target directory")?,
    )
    .join("debug/ssx-html-parser");
    let helper = helper.to_str().ok_or("non-UTF8 helper path")?;
    if cfg!(target_os = "macos") {
        let linked = command(root, "otool", &["-L", helper], records)?;
        let actual: BTreeSet<_> = linked
            .lines()
            .skip(1)
            .filter_map(|line| line.split_whitespace().next())
            .collect();
        let approved:BTreeSet<_>=["/usr/lib/libiconv.2.dylib","/usr/lib/libSystem.B.dylib","/System/Library/Frameworks/Security.framework/Versions/A/Security","/System/Library/Frameworks/SystemConfiguration.framework/Versions/A/SystemConfiguration","/System/Library/Frameworks/CoreFoundation.framework/Versions/A/CoreFoundation"].into_iter().collect();
        if actual != approved {
            return Err("A05 helper has unexpected platform dylib closure".into());
        }
    } else if cfg!(target_os = "linux") {
        let linked = command(root, "ldd", &[helper], records)?;
        if linked.contains("not found") || linked.contains("libiconv") || linked.contains("libxml2")
        {
            return Err("A05 helper has external/missing parser or iconv library".into());
        }
        for line in linked.lines() {
            if let Some((_, path)) = line.split_once("=>") {
                let path = path.split_whitespace().next().ok_or("invalid ldd path")?;
                if !["/lib/", "/lib64/", "/usr/lib/"]
                    .iter()
                    .any(|prefix| path.starts_with(prefix))
                {
                    return Err("A05 helper has non-platform ELF dependency".into());
                }
            }
        }
        let symbols = command(root, "readelf", &["--dyn-syms", "--wide", helper], records)?;
        let iconv: Vec<_> = symbols
            .lines()
            .filter(|line| line.contains("iconv"))
            .collect();
        if iconv.is_empty() || iconv.iter().any(|line| !line.contains("@GLIBC_")) {
            return Err("A05 helper must import iconv only from glibc".into());
        }
    } else {
        return Err("A05 helper linkage target unqualified".into());
    }
    Ok(())
}
