//! Finite pure-A06 successors of accepted main. Historical manifests stay immutable.
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use std::{fs, path::Path};
const MANIFEST: &str = "docs/rust-rewrite/a06/source-amendments/shared-successor-v1.json";
const MANIFEST_SHA: &str = "7b2846032c62d17cc4b56dd9f53f56e8fcab2140bd4166443c65ee5d2c166da0";
const BASELINE: &str = "d5615ace90b827f31789ccf04c72aaad4516e8f7";
const A13: &str = "crates/xtask/src/a13.rs";
const SHA_PAIRS: &[(&str, &str)] = &[
    ("docs/rust-rewrite/a09/coverage.json", "Cargo.lock"),
    (
        "docs/rust-rewrite/a09/coverage.json",
        "crates/ssx-application/src/lib.rs",
    ),
    (
        "docs/rust-rewrite/a09/coverage.json",
        "crates/ssx-infra/src/adapters/mod.rs",
    ),
    (
        "docs/rust-rewrite/a09/coverage.json",
        "crates/ssx-infra/src/adapters/source_payload.rs",
    ),
    (
        "docs/rust-rewrite/a09/coverage.json",
        "crates/ssx-infra/tests/a09_catalog_media.rs",
    ),
    (
        "docs/rust-rewrite/a09/coverage.json",
        "crates/xtask/src/main.rs",
    ),
    (
        "docs/rust-rewrite/a09/coverage.json",
        "docs/rust-rewrite/a09/README.md",
    ),
    ("docs/rust-rewrite/a10/qualification.json", "Cargo.lock"),
    ("docs/rust-rewrite/a10/qualification.json", "Cargo.toml"),
    (
        "docs/rust-rewrite/a10/qualification.json",
        "crates/ssx-infra/src/adapters/clinicaltrials.rs",
    ),
    (
        "docs/rust-rewrite/a10/qualification.json",
        "crates/ssx-infra/src/adapters/mod.rs",
    ),
    (
        "docs/rust-rewrite/a10/qualification.json",
        "crates/ssx-infra/src/adapters/source_json.rs",
    ),
    (
        "docs/rust-rewrite/a10/qualification.json",
        "crates/ssx-infra/src/adapters/source_payload.rs",
    ),
    (
        "docs/rust-rewrite/a10/qualification.json",
        "crates/ssx-infra/src/http/monotonic.rs",
    ),
    ("docs/rust-rewrite/a11/qualification.json", "Cargo.lock"),
    ("docs/rust-rewrite/a11/qualification.json", "Cargo.toml"),
    (
        "docs/rust-rewrite/a11/qualification.json",
        "crates/ssx-infra/Cargo.toml",
    ),
    (
        "docs/rust-rewrite/a11/qualification.json",
        "crates/ssx-infra/src/adapters/clinicaltrials.rs",
    ),
    (
        "docs/rust-rewrite/a11/qualification.json",
        "crates/ssx-infra/src/adapters/mod.rs",
    ),
    (
        "docs/rust-rewrite/a11/qualification.json",
        "crates/ssx-infra/src/adapters/source_json.rs",
    ),
    (
        "docs/rust-rewrite/a11/qualification.json",
        "crates/ssx-infra/src/adapters/source_payload.rs",
    ),
    (
        "docs/rust-rewrite/a11/qualification.json",
        "crates/ssx-infra/src/http/monotonic.rs",
    ),
    (
        "docs/rust-rewrite/a11/qualification.json",
        "crates/ssx-json/src/lib.rs",
    ),
    ("docs/rust-rewrite/a12/qualification-v1.json", "Cargo.lock"),
    ("docs/rust-rewrite/a12/qualification-v1.json", "Cargo.toml"),
    (
        "docs/rust-rewrite/a12/qualification-v1.json",
        "crates/ssx-application/src/lib.rs",
    ),
    (
        "docs/rust-rewrite/a12/qualification-v1.json",
        "crates/ssx-infra/src/adapters/mod.rs",
    ),
    (
        "docs/rust-rewrite/a12/qualification-v1.json",
        "crates/ssx-infra/src/adapters/source_json.rs",
    ),
    (
        "docs/rust-rewrite/a12/qualification-v1.json",
        "crates/ssx-infra/src/adapters/source_payload.rs",
    ),
    (
        "docs/rust-rewrite/a12/qualification-v1.json",
        "crates/xtask/src/main.rs",
    ),
    (
        "docs/rust-rewrite/a13/dehashed-source-map-v1.json",
        "Cargo.lock",
    ),
    (
        "docs/rust-rewrite/a13/dehashed-source-map-v1.json",
        "crates/ssx-infra/Cargo.toml",
    ),
    (
        "docs/rust-rewrite/a13/dehashed-source-map-v1.json",
        "crates/ssx-infra/src/adapters/clinicaltrials.rs",
    ),
    (
        "docs/rust-rewrite/a13/dehashed-source-map-v1.json",
        "crates/ssx-infra/src/adapters/mod.rs",
    ),
    (
        "docs/rust-rewrite/a13/dehashed-source-map-v1.json",
        "crates/ssx-infra/src/adapters/source_json.rs",
    ),
    (
        "docs/rust-rewrite/a13/dehashed-source-map-v1.json",
        "crates/ssx-infra/src/adapters/source_payload.rs",
    ),
    ("docs/rust-rewrite/a13/qualification-v1.json", "Cargo.lock"),
    (
        "docs/rust-rewrite/a13/qualification-v1.json",
        "crates/ssx-infra/Cargo.toml",
    ),
    (
        "docs/rust-rewrite/a13/qualification-v1.json",
        "crates/ssx-infra/src/adapters/clinicaltrials.rs",
    ),
    (
        "docs/rust-rewrite/a13/qualification-v1.json",
        "crates/ssx-infra/src/adapters/mod.rs",
    ),
    (
        "docs/rust-rewrite/a13/qualification-v1.json",
        "crates/ssx-infra/src/adapters/source_json.rs",
    ),
    (
        "docs/rust-rewrite/a13/qualification-v1.json",
        "crates/ssx-infra/src/adapters/source_payload.rs",
    ),
    (
        "docs/rust-rewrite/a13/qualification-v1.json",
        "crates/ssx-json/src/lib.rs",
    ),
    (
        "docs/rust-rewrite/a13/qualification-v1.json",
        "docs/rust-rewrite/f08/parity.md",
    ),
    (
        "docs/rust-rewrite/a13/remaining/source-map.json",
        "Cargo.lock",
    ),
    (
        "docs/rust-rewrite/a13/remaining/source-map.json",
        "crates/ssx-infra/Cargo.toml",
    ),
    (
        "docs/rust-rewrite/a13/remaining/source-map.json",
        "crates/ssx-infra/src/adapters/clinicaltrials.rs",
    ),
    (
        "docs/rust-rewrite/a13/remaining/source-map.json",
        "crates/ssx-infra/src/adapters/mod.rs",
    ),
    (
        "docs/rust-rewrite/a13/remaining/source-map.json",
        "crates/ssx-infra/src/adapters/source_json.rs",
    ),
    (
        "docs/rust-rewrite/a13/remaining/source-map.json",
        "crates/ssx-infra/src/adapters/source_payload.rs",
    ),
    (
        "docs/rust-rewrite/a13/remaining/source-map.json",
        "crates/ssx-json/src/lib.rs",
    ),
    (
        "docs/rust-rewrite/a13/remaining/source-map.json",
        "docs/rust-rewrite/f08/parity.md",
    ),
    ("docs/rust-rewrite/o01/coverage.json", "Cargo.lock"),
    (
        "docs/rust-rewrite/o01/coverage.json",
        "crates/ssx-application/src/lib.rs",
    ),
    (
        "docs/rust-rewrite/o01/coverage.json",
        "crates/ssx-infra/src/lib.rs",
    ),
    ("docs/rust-rewrite/w06/qualification.json", "Cargo.lock"),
    (
        "docs/rust-rewrite/w06/qualification.json",
        "crates/ssx-application/src/lib.rs",
    ),
    (
        "docs/rust-rewrite/w06/qualification.json",
        "crates/ssx-infra/Cargo.toml",
    ),
    (
        "docs/rust-rewrite/w06/qualification.json",
        "crates/ssx-infra/src/lib.rs",
    ),
];
const GIT_PAIRS: &[(&str, &str)] = &[
    (
        "docs/rust-rewrite/a14/cases.json",
        "crates/ssx-infra/src/adapters/clinicaltrials.rs",
    ),
    (
        "docs/rust-rewrite/a14/cases.json",
        "crates/ssx-infra/src/adapters/source_json.rs",
    ),
    (
        "docs/rust-rewrite/a14/cases.json",
        "crates/ssx-infra/src/adapters/source_payload.rs",
    ),
];
const TRANSITION_PATHS: &[&str] = &[
    "Cargo.lock",
    "Cargo.toml",
    "crates/ssx-application/src/lib.rs",
    "crates/ssx-infra/Cargo.toml",
    "crates/ssx-infra/src/adapters/clinicaltrials.rs",
    "crates/ssx-infra/src/adapters/mod.rs",
    "crates/ssx-infra/src/adapters/source_json.rs",
    "crates/ssx-infra/src/adapters/source_payload.rs",
    "crates/ssx-infra/src/http.rs",
    "crates/ssx-infra/src/http/monotonic.rs",
    "crates/ssx-infra/src/lib.rs",
    "crates/ssx-infra/tests/a09_catalog_media.rs",
    "crates/ssx-infra/tests/a13_dehashed.rs",
    "crates/ssx-json/src/lib.rs",
    "crates/xtask/src/a04.rs",
    "crates/xtask/src/a13.rs",
    "crates/xtask/src/main.rs",
    "docs/rust-rewrite/a09/README.md",
    "docs/rust-rewrite/f08/parity.md",
];
fn need(ok: bool, message: &str) -> Result<(), String> {
    if ok { Ok(()) } else { Err(message.into()) }
}
fn digest(raw: &[u8]) -> String {
    Sha256::digest(raw)
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect()
}
fn raw_mode(mode: u32) -> String {
    format!("{:04o}", mode & 0o7777)
}
#[cfg(unix)]
fn permissions(path: &Path) -> Result<String, String> {
    use std::os::unix::fs::PermissionsExt;
    Ok(raw_mode(
        fs::symlink_metadata(path)
            .map_err(|e| e.to_string())?
            .permissions()
            .mode(),
    ))
}
#[cfg(not(unix))]
fn permissions(_: &Path) -> Result<String, String> {
    Err("A06 source mode proof requires Unix".into())
}
fn text<'a>(row: &'a Value, key: &str) -> Result<&'a str, String> {
    row[key]
        .as_str()
        .ok_or_else(|| format!("A06 missing {key}"))
}
fn material(row: &Value, raw: &[u8], mode: &str) -> Result<(), String> {
    need(
        row["sha256"] == digest(raw)
            && row["bytes"].as_u64() == Some(raw.len() as u64)
            && row["mode"] == mode,
        "A06 exact source material differs",
    )
}
fn read(root: &Path, row: &Value) -> Result<Vec<u8>, String> {
    let path = root.join(text(row, "path")?);
    let stat = fs::symlink_metadata(&path).map_err(|e| e.to_string())?;
    need(
        stat.is_file() && !stat.file_type().is_symlink(),
        "A06 source must be regular, not symlink",
    )?;
    let raw = fs::read(&path).map_err(|e| e.to_string())?;
    material(row, &raw, &permissions(&path)?)?;
    Ok(raw)
}
fn edits(before: &[u8], changes: &Value) -> Result<Vec<u8>, String> {
    let rows = changes.as_array().ok_or("A06 ordered edits absent")?;
    need(!rows.is_empty(), "A06 ordered edits absent")?;
    let mut output = Vec::new();
    let mut end = 0;
    for row in rows {
        let at = row["at"]
            .as_u64()
            .and_then(|n| usize::try_from(n).ok())
            .ok_or("A06 edit offset")?;
        let remove = text(row, "remove_utf8")?.as_bytes();
        let insert = text(row, "insert_utf8")?.as_bytes();
        let next = at.checked_add(remove.len()).ok_or("A06 edit overflow")?;
        need(
            at >= end
                && next <= before.len()
                && &before[at..next] == remove
                && (!remove.is_empty() || !insert.is_empty()),
            "A06 edit preimage/order differs",
        )?;
        output.extend_from_slice(&before[end..at]);
        output.extend_from_slice(insert);
        end = next;
    }
    output.extend_from_slice(&before[end..]);
    Ok(output)
}
fn transition(
    row: &Value,
    before: &[u8],
    actual: &[u8],
    mode: &str,
    reviewed_edits_sha: &str,
) -> Result<(), String> {
    need(
        row["baseline"]["path"] == row["current"]["path"]
            && row["archive"]["source_path"] == row["baseline"]["path"]
            && row["archive"]["sha256"] == row["baseline"]["sha256"]
            && row["archive"]["bytes"] == row["baseline"]["bytes"]
            && row["archive"]["mode"] == row["baseline"]["mode"],
        "A06 predecessor linkage differs",
    )?;
    material(&row["baseline"], before, "0644")?;
    material(&row["current"], actual, mode)?;
    need(
        digest(&serde_json::to_vec(&row["edits"]).map_err(|e| e.to_string())?)
            == reviewed_edits_sha,
        "A06 reviewed edit digest differs",
    )?;
    need(
        edits(before, &row["edits"])? == actual,
        "A06 reconstruction differs from actual current",
    )
}
fn document(raw: &[u8]) -> Result<Value, String> {
    need(digest(raw) == MANIFEST_SHA, "A06 shared sidecar changed")?;
    let doc: Value = serde_json::from_slice(raw).map_err(|e| e.to_string())?;
    need(
        doc["kind"] == "PURE_A06_SHARED_SOURCE_SUCCESSOR_V1"
            && doc["baseline_revision"] == BASELINE,
        "A06 shared sidecar scope differs",
    )?;
    scope(&doc)?;
    Ok(doc)
}
fn scope(doc: &Value) -> Result<(), String> {
    use std::collections::BTreeSet;
    let paths = doc["transitions"]
        .as_array()
        .ok_or("A06 transitions absent")?
        .iter()
        .map(|r| text(&r["current"], "path"))
        .collect::<Result<Vec<_>, _>>()?;
    need(
        paths.len() == TRANSITION_PATHS.len()
            && paths.iter().copied().collect::<BTreeSet<_>>()
                == TRANSITION_PATHS.iter().copied().collect(),
        "A06 exact transition membership differs",
    )?;
    let pairs = doc["bindings"]
        .as_array()
        .ok_or("A06 bindings absent")?
        .iter()
        .map(|r| {
            Ok((
                text(&r["container"], "path")?,
                text(r, "source_path")?,
                text(r, "digest_kind")?,
            ))
        })
        .collect::<Result<Vec<_>, String>>()?;
    let expected = SHA_PAIRS
        .iter()
        .map(|(c, p)| (*c, *p, "sha256"))
        .chain(GIT_PAIRS.iter().map(|(c, p)| (*c, *p, "git_blob")))
        .collect::<BTreeSet<_>>();
    need(
        pairs.len() == expected.len() && pairs.into_iter().collect::<BTreeSet<_>>() == expected,
        "A06 exact caller membership differs",
    )
}
fn load(root: &Path) -> Result<Value, String> {
    document(&fs::read(root.join(MANIFEST)).map_err(|e| e.to_string())?)
}
fn source<'a>(doc: &'a Value, path: &str) -> Result<&'a Value, String> {
    let rows = doc["transitions"]
        .as_array()
        .ok_or("A06 transitions absent")?;
    let matching: Vec<_> = rows
        .iter()
        .filter(|r| r["current"]["path"] == path)
        .collect();
    need(
        matching.len() == 1,
        "A06 missing/duplicate source transition",
    )?;
    Ok(matching[0])
}
fn verify_transition(root: &Path, row: &Value) -> Result<(Vec<u8>, Vec<u8>), String> {
    let before = read(root, &row["archive"])?;
    let actual = read(root, &row["current"])?;
    transition(
        row,
        &before,
        &actual,
        &permissions(&root.join(text(&row["current"], "path")?))?,
        text(row, "edits_sha256")?,
    )?;
    Ok((before, actual))
}
fn frozen(root: &Path, doc: &Value) -> Result<(), String> {
    for row in doc["immutable_predecessors"]
        .as_array()
        .ok_or("A06 predecessor authority absent")?
    {
        read(root, row)?;
    }
    Ok(())
}
fn pair<'a>(
    doc: &'a Value,
    container: &str,
    path: &str,
    kind: &str,
) -> Result<Option<&'a Value>, String> {
    let rows = doc["bindings"].as_array().ok_or("A06 bindings absent")?;
    let found: Vec<_> = rows
        .iter()
        .filter(|r| {
            r["container"]["path"] == container
                && r["source_path"] == path
                && r["digest_kind"] == kind
        })
        .collect();
    need(found.len() <= 1, "A06 duplicate caller binding")?;
    Ok(found.first().copied())
}
fn caller(
    binding: &Value,
    container_sha: &str,
    raw: &[u8],
    path: &str,
    expected: &str,
    bytes: Option<usize>,
) -> Result<(), String> {
    need(
        binding["source_path"] == path
            && binding["container"]["sha256"] == container_sha
            && digest(raw) == container_sha
            && binding["expected_digest"] == expected
            && binding["expected_bytes"] == json!(bytes),
        "A06 caller typed tuple differs",
    )?;
    let doc: Value = serde_json::from_slice(raw).map_err(|e| e.to_string())?;
    let old = doc
        .pointer(text(binding, "pointer")?)
        .ok_or("A06 original frozen row absent")?;
    if binding["row_shape"] == "object" {
        need(
            old["path"] == path
                && old["sha256"] == expected
                && old
                    .get("bytes")
                    .and_then(Value::as_u64)
                    .and_then(|n| usize::try_from(n).ok())
                    == bytes,
            "A06 original frozen object differs",
        )?;
    } else {
        need(
            old == expected && bytes.is_none(),
            "A06 original frozen scalar differs",
        )?;
    }
    Ok(())
}
pub(crate) fn selected(container: &str, path: &str) -> bool {
    SHA_PAIRS.contains(&(container, path))
}
pub(crate) fn try_verify(
    root: &Path,
    container: &str,
    container_sha: &str,
    path: &str,
    expected: &str,
    bytes: Option<usize>,
) -> Option<Result<(), String>> {
    if !selected(container, path) {
        return None;
    }
    let doc = match load(root) {
        Ok(doc) => doc,
        Err(e) => return Some(Err(e)),
    };
    let binding = match pair(&doc, container, path, "sha256") {
        Ok(Some(b)) => b,
        Ok(None) => return None,
        Err(e) => return Some(Err(e)),
    };
    Some((|| {
        let raw = read(root, &binding["container"])?;
        caller(binding, container_sha, &raw, path, expected, bytes)?;
        frozen(root, &doc)?;
        let (_, actual) = verify_transition(root, source(&doc, path)?)?;
        eprintln!(
            "A06_SHARED_CURRENT_PROOF source={path} current_sha256={} prior_binding={expected}",
            digest(&actual)
        );
        Ok(())
    })())
}
// This entry is used by xtask A14; the same source is also compiled by the standalone A13 test crate.
#[allow(dead_code)]
pub(crate) fn try_git<F>(
    root: &Path,
    container: &str,
    container_sha: &str,
    path: &str,
    expected: &str,
    actual_blob: &str,
    mut archived_blob: F,
) -> Option<Result<(), String>>
where
    F: FnMut(&str) -> Result<String, String>,
{
    if !GIT_PAIRS.contains(&(container, path)) {
        return None;
    }
    let doc = match load(root) {
        Ok(doc) => doc,
        Err(e) => return Some(Err(e)),
    };
    let binding = match pair(&doc, container, path, "git_blob") {
        Ok(Some(b)) => b,
        Ok(None) => return None,
        Err(e) => return Some(Err(e)),
    };
    Some((|| {
        caller(
            binding,
            container_sha,
            &read(root, &binding["container"])?,
            path,
            expected,
            None,
        )?;
        frozen(root, &doc)?;
        let row = source(&doc, path)?;
        verify_transition(root, row)?;
        let accepted = &binding["accepted_archive"];
        read(root, accepted)?;
        let archived_blob = archived_blob(text(accepted, "path")?)?;
        need(
            archived_blob.trim() == expected && row["current"]["git_blob"] == actual_blob,
            "A06 exact accepted/current Git blob differs",
        )?;
        Ok(())
    })())
}
/// Effective current rows are derived from immutable V3, never written over it.
pub(crate) fn compose_scoped(root: &Path, previous: &Value) -> Result<Value, String> {
    let doc = load(root)?;
    frozen(root, &doc)?;
    let original: Value = serde_json::from_slice(&read(root, &doc["scoped_predecessor"])?)
        .map_err(|e| e.to_string())?;
    need(original == *previous, "A06 scoped predecessor differs")?;
    let mut next = previous.clone();
    for row in next["successors"]
        .as_array_mut()
        .ok_or("A06 scoped successors absent")?
    {
        let path = text(&row["current"], "path")?.to_owned();
        if TRANSITION_PATHS.contains(&path.as_str()) {
            let replacement = source(&doc, &path)?;
            need(
                row["current"] == replacement["baseline"],
                "A06 scoped current predecessor differs",
            )?;
            let (_, actual) = verify_transition(root, replacement)?;
            let historical = read(root, &row["historical191"])?;
            need(
                edits(&historical, &replacement["scoped_edits"])? == actual,
                "A06 complete scoped reconstruction differs",
            )?;
            row["current"] = replacement["current"].clone();
            row["edits"] = json!(replacement["scoped_edits"].as_array().ok_or("A06 scoped edits absent")?.iter().map(|edit| {
                Ok(json!({"at":edit["at"],"remove":text(edit,"remove_utf8")?.as_bytes(),"insert":text(edit,"insert_utf8")?.as_bytes()}))
            }).collect::<Result<Vec<Value>, String>>()?);
        }
    }
    for row in next["additional_current_materials"]
        .as_array_mut()
        .ok_or("A06 scoped current materials absent")?
    {
        let path = text(row, "path")?;
        if TRANSITION_PATHS.contains(&path) {
            let replacement = source(&doc, path)?;
            need(
                *row == replacement["baseline"],
                "A06 additional predecessor differs",
            )?;
            verify_transition(root, replacement)?;
            *row = replacement["current"].clone();
        }
    }
    Ok(next)
}
/// Explicit old accepted A13 material for predecessor relation/negative controls.
/// Actual new source is independently read and reconstructed before returning it.
pub(crate) fn a13_baseline(root: &Path, requested: &Value) -> Result<(Value, Vec<u8>), String> {
    let doc = load(root)?;
    let row = source(&doc, A13)?;
    need(
        *requested == row["baseline"] || *requested == row["current"],
        "A06 A13 predecessor/current request differs",
    )?;
    let (before, _) = verify_transition(root, row)?;
    Ok((row["baseline"].clone(), before))
}
#[cfg(test)]
pub(crate) fn current_http(root: &Path, actual: &[u8]) -> Result<bool, String> {
    let doc = load(root)?;
    let row = source(&doc, "crates/ssx-infra/src/http.rs")?;
    Ok(row["current"]["sha256"] == digest(actual))
}
#[cfg(test)]
pub(crate) fn controls(root: &Path) -> Result<(), String> {
    let doc = load(root)?;
    frozen(root, &doc)?;
    scope(&doc)?;
    need(
        try_verify(
            Path::new("/missing-A06-control"),
            "unlisted.json",
            "wrong",
            "unlisted.rs",
            "wrong",
            None,
        )
        .is_none(),
        "A06 unlisted caller did not retain original dispatch",
    )?;
    for field in ["transitions", "bindings"] {
        let mut missing = doc.clone();
        missing[field].as_array_mut().unwrap().pop();
        need(scope(&missing).is_err(), "A06 missing scope entry accepted")?;
        let mut duplicated = doc.clone();
        let first = duplicated[field][0].clone();
        duplicated[field].as_array_mut().unwrap().push(first);
        need(
            scope(&duplicated).is_err(),
            "A06 duplicated scope entry accepted",
        )?;
    }
    for row in doc["transitions"]
        .as_array()
        .ok_or("A06 control transitions absent")?
    {
        let (before, actual) = verify_transition(root, row)?;
        let sha = text(row, "edits_sha256")?;
        need(
            transition(row, &before, &before, "0644", sha).is_err(),
            "A06 predecessor accepted as current",
        )?;
        for mode in ["0755", "0600", &raw_mode(0o104644)] {
            need(
                transition(row, &before, &actual, mode, sha).is_err(),
                "A06 changed mode accepted",
            )?;
        }
        need(raw_mode(0o104644) == "4644", "A06 special mode bits lost")?;
        need(
            material(&row["current"], &actual, &raw_mode(0o100600)).is_err(),
            "A06 raw 0600 mode accepted",
        )?;
        let mut changed = actual.clone();
        changed.push(0);
        let mut claim = row.clone();
        claim["current"]["sha256"] = json!(digest(&changed));
        claim["current"]["bytes"] = json!(changed.len());
        need(
            transition(&claim, &before, &changed, "0644", sha).unwrap_err()
                == "A06 reconstruction differs from actual current",
            "A06 rehashed unrelated current did not reach reconstruction guard",
        )?;
        for mutation in 0..4 {
            let mut bad = row.clone();
            match mutation {
                0 => {
                    bad["edits"].as_array_mut().unwrap().pop();
                }
                1 => {
                    let first = bad["edits"][0].clone();
                    bad["edits"].as_array_mut().unwrap().insert(0, first);
                }
                2 => {
                    bad["edits"][0]["at"] = json!(before.len() + 1);
                }
                _ => {
                    bad["archive"]["source_path"] = json!("wrong.rs");
                }
            }
            let structural_sha =
                digest(&serde_json::to_vec(&bad["edits"]).map_err(|e| e.to_string())?);
            need(
                transition(&bad, &before, &actual, "0644", &structural_sha).is_err(),
                "A06 causal structural mutation accepted",
            )?;
        }
    }
    for binding in doc["bindings"]
        .as_array()
        .ok_or("A06 control bindings absent")?
    {
        let path = text(binding, "source_path")?;
        let hash = text(&binding["container"], "sha256")?;
        let old = text(binding, "expected_digest")?;
        let count = binding["expected_bytes"]
            .as_u64()
            .and_then(|n| usize::try_from(n).ok());
        let raw = read(root, &binding["container"])?;
        caller(binding, hash, &raw, path, old, count)?;
        if binding["digest_kind"] == "sha256" {
            try_verify(
                root,
                text(&binding["container"], "path")?,
                hash,
                path,
                old,
                count,
            )
            .ok_or("A06 real selected route absent")??;
        }
        for (bad_hash, bad_source, bad_old, bad_count) in [
            ("wrong", path, old, count),
            (hash, "wrong.rs", old, count),
            (hash, path, "wrong", count),
            (
                hash,
                path,
                old,
                if count.is_some() { None } else { Some(0) },
            ),
        ] {
            need(
                caller(binding, bad_hash, &raw, bad_source, bad_old, bad_count).is_err(),
                "A06 caller mutation accepted",
            )?;
        }
    }
    eprintln!(
        "A06_SHARED_TRANSITION_CONTROLS actual_current=true typed_callers_archives_order_scope_modes_and_rehashed_mutations=passed"
    );
    Ok(())
}
