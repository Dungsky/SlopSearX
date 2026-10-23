//! Exact header-marker child of recorded d448 A13; frozen qualification remains immutable.
use serde_json::Value;
use std::path::Path;
pub(super) const SOURCE: &str = "crates/xtask/src/a13.rs";
const CAPSULE: &str = "docs/rust-rewrite/a06/source-amendments/a13-marker-successor-v1.json";
const CAPSULE_SHA: &str = "19db726ff5ba2b6a6868d9ab14a06d046fdc6d33d0388e11f04ea676e00d61e8";
fn document(raw: &[u8]) -> Result<Value, String> {
    super::need(
        super::digest(raw) == CAPSULE_SHA,
        "A13 header marker capsule changed",
    )?;
    let doc: Value = serde_json::from_slice(raw).map_err(|e| e.to_string())?;
    super::need(
        doc["schema"] == "A13_EXACT_HEADER_MARKER_SUCCESSOR_V1"
            && doc["source_parent"] == "d44802020d321ba6eb5441dc8f53f3c5cbf286af"
            && doc["prior"]["path"] == SOURCE
            && doc["current"]["path"] == SOURCE,
        "A13 header marker closed source scope",
    )?;
    Ok(doc)
}
fn load(root: &Path) -> Result<Value, String> {
    let path = root.join(CAPSULE);
    super::need(
        super::permissions(&path)? == "0644",
        "A13 header marker capsule mode",
    )?;
    document(&std::fs::read(path).map_err(|e| e.to_string())?)
}
fn validate(
    doc: &Value,
    expected: &Value,
    prior: &[u8],
    current: &[u8],
    mode: &str,
) -> Result<(), String> {
    super::need(
        doc["prior"] == *expected,
        "A13 header marker exact d448 caller",
    )?;
    super::material(&doc["prior"], prior, "0644")?;
    super::material(&doc["current"], current, mode)?;
    super::need(
        super::edits(prior, &doc["current_edits"])? == current,
        "A13 header marker exact reconstruction",
    )
}
pub(super) fn archived_prior(root: &Path, expected: &Value) -> Result<Vec<u8>, String> {
    let doc = load(root)?;
    let prior = super::read(root, &doc["archive"])?;
    let current = super::read(root, &doc["current"])?;
    validate(
        &doc,
        expected,
        &prior,
        &current,
        &super::permissions(&root.join(SOURCE))?,
    )?;
    Ok(prior)
}
pub(super) fn effective_current(root: &Path, expected: &Value) -> Result<Value, String> {
    archived_prior(root, expected)?;
    Ok(load(root)?["current"].clone())
}
#[cfg(test)]
pub(super) fn library_source(root: &Path, expected: &Value) -> Result<(), String> {
    let doc = load(root)?;
    super::need(
        expected == &serde_json::json!({"path":SOURCE,"sha256":doc["prior"]["sha256"]}),
        "A13 marker prior exact A13 predecessor",
    )?;
    archived_prior(root, &doc["prior"])?;
    Ok(())
}
#[cfg(test)]
pub(super) fn controls(root: &Path) -> Result<(), String> {
    let doc = load(root)?;
    let prior = super::read(root, &doc["archive"])?;
    let current = super::read(root, &doc["current"])?;
    validate(&doc, &doc["prior"], &prior, &current, "0644")?;
    library_source(
        root,
        &serde_json::json!({"path":SOURCE,"sha256":doc["prior"]["sha256"]}),
    )?;
    let mut wrong = doc["prior"].clone();
    wrong["path"] = serde_json::json!("wrong-caller.rs");
    super::need(
        validate(&doc, &wrong, &prior, &current, "0644").is_err(),
        "A13 wrong inventory caller accepted",
    )?;
    super::need(
        validate(&doc, &doc["prior"], &prior, &prior, "0644").is_err(),
        "A13 old inventory accepted as current",
    )?;
    super::need(
        validate(&doc, &doc["prior"], &prior, &current, "0755").is_err(),
        "A13 changed inventory source mode accepted",
    )?;
    let mut changed = current.clone();
    changed.push(b'\n');
    super::need(
        validate(&doc, &doc["prior"], &prior, &changed, "0644").is_err(),
        "A13 changed inventory source accepted",
    )?;
    let mut changed_prior = prior.clone();
    changed_prior.push(b'\n');
    super::need(
        validate(&doc, &doc["prior"], &changed_prior, &current, "0644").is_err(),
        "A13 edited inventory archive accepted",
    )?;
    let mut changed_doc = doc.clone();
    changed_doc["current"]["sha256"] = serde_json::json!(super::digest(&changed));
    changed_doc["current"]["bytes"] = serde_json::json!(changed.len());
    super::need(
        document(&serde_json::to_vec(&changed_doc).map_err(|e| e.to_string())?).is_err(),
        "A13 rehashed inventory capsule accepted",
    )?;
    super::need(
        library_source(root, &serde_json::json!({"path":SOURCE,"sha256":"wrong"})).is_err(),
        "A06 changed frozen library caller accepted",
    )?;
    Ok(())
}
