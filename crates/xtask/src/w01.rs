//! Source-bound record/accounting replay plus mandatory native disposable Valkey.
use crate::{Result, command};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use std::{fs, path::Path};
const BASELINE: &str = "cfd7a0af61a22cfd040a7fcd41ad950f82005959";
// Reviewed identities live in native code, independently of artifact metadata.
const CORPUS_SHA256: &str = "c3e5e026e59f84a93017f434042eba929aaa6de269098f642c00ee3eecdb2159";
const EXPORTER_SHA256: &str = "373ff210c0c8512323a52be86d5dc036583f9e2a458563c78e02115068800a0d";
fn digest(bytes: &[u8]) -> String {
    Sha256::digest(bytes)
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect()
}
fn validate_artifacts(corpus: &[u8], exporter: &[u8]) -> Result<Value> {
    if digest(corpus) != CORPUS_SHA256 || digest(exporter) != EXPORTER_SHA256 {
        return Err("W01 reviewed oracle artifact identity mismatch".into());
    }
    let value: Value = serde_json::from_slice(corpus).map_err(|e| e.to_string())?;
    if value["schema_version"] != 1
        || value["plan_id"] != "W01"
        || value["baseline"] != BASELINE
        || value["python"] != "3.14.7"
        || value["exporter_sha256"] != EXPORTER_SHA256
    {
        return Err("W01 oracle provenance mismatch".into());
    }
    Ok(value)
}
pub fn check(root: &Path, records: &mut Vec<Value>) -> Result<()> {
    let corpus =
        fs::read(root.join("crates/xtask/fixtures/w01/corpus.json")).map_err(|e| e.to_string())?;
    let exporter =
        fs::read(root.join("docs/rust-rewrite/w01/oracle/export.py")).map_err(|e| e.to_string())?;
    let data = validate_artifacts(&corpus, &exporter)?;
    let sources = data["sources"].as_array().ok_or("missing W01 sources")?;
    for source in sources {
        let path = source["path"].as_str().ok_or("missing W01 source path")?;
        let object = format!("{BASELINE}:{path}");
        let git_blob = command(root, "git", &["rev-parse", &object], records)?;
        if Some(git_blob.trim()) != source["git_blob"].as_str() {
            return Err(format!("W01 source blob drift: {path}"));
        }
        let bytes = fs::read(root.join(path)).map_err(|e| e.to_string())?;
        if Some(digest(&bytes).as_str()) != source["sha256"].as_str() {
            return Err(format!(
                "W01 current source drift requires reviewed recapture: {path}"
            ));
        }
    }
    command(
        root,
        "cargo",
        &["test", "--locked", "-p", "ssx-domain", "--test", "w01"],
        records,
    )?;
    // F07 verifies exact accepted substrate scripts and real native fault contracts.
    crate::f07::check(root, records)?;
    if !records.iter().any(|r| {
        r["version"]
            .as_str()
            .is_some_and(|v| v.starts_with("Valkey server v=8.1.10 "))
    }) {
        return Err("W01 requires official disposable Valkey8.1.10".into());
    }
    command(
        root,
        "cargo",
        &[
            "test",
            "--locked",
            "-p",
            "ssx-infra",
            "--test",
            "w01",
            "--",
            "--test-threads=1",
        ],
        records,
    )?;
    records.push(json!({"plan_id":"W01","status":"passed","oracle_sha256":CORPUS_SHA256,"exporter_sha256":EXPORTER_SHA256,"baseline":BASELINE,"case_count":10,"providers":0,"authority":"native private Valkey8.1.10 processes; source-bound record/accounting replay; no worker execution"}));
    Ok(())
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn missing_corrupt_and_self_redeclared_oracle_evidence_fails() {
        let corpus = include_bytes!("../fixtures/w01/corpus.json");
        let exporter = include_bytes!("../../../docs/rust-rewrite/w01/oracle/export.py");
        assert!(validate_artifacts(corpus, exporter).is_ok());
        assert!(validate_artifacts(&[], exporter).is_err());
        assert!(validate_artifacts(corpus, &[]).is_err());
        let mut altered = corpus.to_vec();
        altered.push(b' ');
        assert!(validate_artifacts(&altered, exporter).is_err());
        let mut value: Value = serde_json::from_slice(corpus).unwrap();
        value["cases"] = json!([]);
        value["exporter_sha256"] = json!(digest(exporter));
        assert!(validate_artifacts(&serde_json::to_vec(&value).unwrap(), exporter).is_err());
    }
}
