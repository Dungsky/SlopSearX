//! Narrow shared validator for exact A13 historical-source amendments.
use serde_json::Value;
use sha2::{Digest, Sha256};
use std::{fs, path::Path};

const MANIFEST_PATH: &str =
    "docs/rust-rewrite/a13/source-amendments/a13-http-source-amendment-v1.json";
const MANIFEST_SHA256: &str = "78fa6adfe1c3718c8d58060f5e395f1a15375b867e813433d868def03a6b5db9";
const AMENDED_PATHS: [&str; 4] = [
    "crates/ssx-infra/src/http.rs",
    "crates/ssx-infra/tests/a13_dehashed.rs",
    "crates/ssx-infra/tests/a13_remaining_fixture/mod.rs",
    "crates/xtask/src/a11.rs",
];

fn digest(bytes: &[u8]) -> String {
    Sha256::digest(bytes)
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect()
}

fn require(ok: bool, message: &str) -> Result<(), String> {
    if ok { Ok(()) } else { Err(message.into()) }
}

#[cfg(unix)]
fn mode(path: &Path) -> Result<String, String> {
    use std::os::unix::fs::PermissionsExt;
    Ok(format!(
        "{:04o}",
        fs::metadata(path)
            .map_err(|error| error.to_string())?
            .permissions()
            .mode()
            & 0o777
    ))
}

#[cfg(not(unix))]
fn mode(_path: &Path) -> Result<String, String> {
    Ok("0644".into())
}

fn validate_scope(manifest: &Value) -> Result<(), String> {
    require(
        manifest["kind"] == "A13_CURRENT_SOURCE_AMENDMENT_V1"
            && manifest["amended_source_paths"] == serde_json::json!(AMENDED_PATHS),
        "A13 amendment path scope changed",
    )?;
    let amendments = manifest["amendments"]
        .as_array()
        .ok_or("A13 amendment entries absent")?;
    require(
        amendments.len() == AMENDED_PATHS.len()
            && amendments
                .iter()
                .map(|entry| entry["source_path"].as_str().unwrap_or(""))
                .eq(AMENDED_PATHS),
        "A13 amendment membership changed",
    )
}

struct PinCheck<'a> {
    amendment: &'a Value,
    container_path: &'a str,
    container_sha: &'a str,
    container_bytes: &'a [u8],
    source_path: &'a str,
    historical_sha: &'a str,
    historical_bytes: Option<usize>,
    current_bytes: &'a [u8],
    archive_bytes: &'a [u8],
    current_mode: &'a str,
    archive_mode: &'a str,
}

fn verify_payload(check: PinCheck<'_>) -> Result<(), String> {
    let PinCheck {
        amendment,
        container_path,
        container_sha,
        container_bytes,
        source_path,
        historical_sha,
        historical_bytes,
        current_bytes,
        archive_bytes,
        current_mode,
        archive_mode,
    } = check;
    require(
        amendment["source_path"] == source_path,
        "A13 amendment source path differs",
    )?;
    let pins = amendment["historical_pins"]
        .as_array()
        .ok_or("A13 historical pins absent")?;
    let matching = pins
        .iter()
        .filter(|pin| pin["container_path"] == container_path)
        .collect::<Vec<_>>();
    require(
        matching.len() == 1,
        "A13 historical container binding differs",
    )?;
    let pin = matching[0];
    let archived_len = amendment["archive"]["bytes"]
        .as_u64()
        .ok_or("A13 archive byte count absent")? as usize;
    require(
        pin["container_sha256"] == container_sha
            && digest(container_bytes) == container_sha
            && pin["source_sha256"] == historical_sha
            && pin["source_bytes"].as_u64() == Some(archived_len as u64)
            && historical_bytes.is_none_or(|expected| expected == archived_len)
            && pin["source_mode"] == amendment["archive"]["mode"],
        "A13 historical source binding differs",
    )?;
    let archived_sha = amendment["archive"]["sha256"]
        .as_str()
        .ok_or("A13 archive digest absent")?;
    let archived_len = amendment["archive"]["bytes"]
        .as_u64()
        .ok_or("A13 archive byte count absent")? as usize;
    let successor_sha = amendment["successor"]["sha256"]
        .as_str()
        .ok_or("A13 successor digest absent")?;
    let successor_len = amendment["successor"]["bytes"]
        .as_u64()
        .ok_or("A13 successor byte count absent")? as usize;
    require(
        amendment["archive"]["mode"] == archive_mode
            && amendment["successor"]["mode"] == current_mode
            && amendment["successor"]["path"] == source_path
            && historical_sha == archived_sha
            && archive_bytes.len() == archived_len
            && digest(archive_bytes) == archived_sha
            && current_bytes.len() == successor_len
            && digest(current_bytes) == successor_sha,
        "A13 archive or reviewed successor differs",
    )
}

pub fn verify(
    root: &Path,
    container_path: &str,
    container_sha: &str,
    source_path: &str,
    historical_sha: &str,
    historical_bytes: Option<usize>,
) -> Result<(), String> {
    let container = fs::read(root.join(container_path))
        .map_err(|error| format!("{container_path}: {error}"))?;
    let manifest_bytes = fs::read(root.join(MANIFEST_PATH)).map_err(|error| error.to_string())?;
    require(
        digest(&manifest_bytes) == MANIFEST_SHA256,
        "A13 source amendment manifest changed",
    )?;
    let manifest: Value =
        serde_json::from_slice(&manifest_bytes).map_err(|error| error.to_string())?;
    validate_scope(&manifest)?;
    let current_path = root.join(source_path);
    let current = fs::read(&current_path).map_err(|error| format!("{source_path}: {error}"))?;
    let amendments = manifest["amendments"].as_array().unwrap();
    if let Some(amendment) = amendments
        .iter()
        .find(|entry| entry["source_path"] == source_path)
    {
        let archive_path = amendment["archive"]["path"]
            .as_str()
            .ok_or("A13 archive path absent")?;
        let archive_file = root.join(archive_path);
        let archive = fs::read(&archive_file).map_err(|error| error.to_string())?;
        return verify_payload(PinCheck {
            amendment,
            container_path,
            container_sha,
            container_bytes: &container,
            source_path,
            historical_sha,
            historical_bytes,
            current_bytes: &current,
            archive_bytes: &archive,
            current_mode: &mode(&current_path)?,
            archive_mode: &mode(&archive_file)?,
        });
    }
    require(
        !AMENDED_PATHS.contains(&source_path)
            && digest(&container) == container_sha
            && historical_bytes.is_none_or(|length| current.len() == length)
            && digest(&current) == historical_sha,
        "A13 unamended source pin differs",
    )
}

/// Exercise the verifier against in-memory manifest, container, archive, and
/// successor mutations without changing any source or evidence file.
#[cfg(test)]
pub fn rejects_mutants(root: &Path) -> Result<(), String> {
    let manifest_bytes = fs::read(root.join(MANIFEST_PATH)).map_err(|error| error.to_string())?;
    let mut manifest: Value =
        serde_json::from_slice(&manifest_bytes).map_err(|error| error.to_string())?;
    validate_scope(&manifest)?;
    let path = AMENDED_PATHS[0];
    let amendment = manifest["amendments"]
        .as_array()
        .unwrap()
        .first()
        .unwrap()
        .clone();
    let pin = &amendment["historical_pins"][0];
    let container_path = pin["container_path"].as_str().unwrap();
    let container_sha = pin["container_sha256"].as_str().unwrap();
    let container = fs::read(root.join(container_path)).map_err(|error| error.to_string())?;
    let current = fs::read(root.join(path)).map_err(|error| error.to_string())?;
    let archive_path = amendment["archive"]["path"].as_str().unwrap();
    let archive_file = root.join(archive_path);
    let archive = fs::read(&archive_file).map_err(|error| error.to_string())?;
    let old_sha = pin["source_sha256"].as_str().unwrap();
    let old_bytes = pin["source_bytes"].as_u64().unwrap() as usize;
    let current_mode = mode(&root.join(path))?;
    let archive_mode = mode(&archive_file)?;
    let accepts = |entry: &Value,
                   path: &str,
                   container_sha: &str,
                   container: &[u8],
                   current: &[u8],
                   archive: &[u8]| {
        verify_payload(PinCheck {
            amendment: entry,
            container_path,
            container_sha,
            container_bytes: container,
            source_path: path,
            historical_sha: old_sha,
            historical_bytes: Some(old_bytes),
            current_bytes: current,
            archive_bytes: archive,
            current_mode: &current_mode,
            archive_mode: &archive_mode,
        })
        .is_ok()
    };
    require(
        accepts(
            &amendment,
            path,
            container_sha,
            &container,
            &current,
            &archive,
        ),
        "A13 amendment positive control failed",
    )?;
    let mut changed_successor = amendment.clone();
    changed_successor["successor"]["sha256"] = serde_json::json!("changed-successor");
    require(
        !accepts(
            &changed_successor,
            path,
            container_sha,
            &container,
            &current,
            &archive,
        ),
        "A13 successor mutation was accepted",
    )?;
    let mut changed_archive = amendment.clone();
    changed_archive["archive"]["sha256"] = serde_json::json!("changed-archive");
    require(
        !accepts(
            &changed_archive,
            path,
            container_sha,
            &container,
            &current,
            &archive,
        ),
        "A13 archive mutation was accepted",
    )?;
    let mut changed_archive_bytes = archive.clone();
    changed_archive_bytes.push(0);
    require(
        !accepts(
            &amendment,
            path,
            container_sha,
            &container,
            &current,
            &changed_archive_bytes,
        ),
        "A13 archive byte mutation was accepted",
    )?;
    let mut changed_container = container.clone();
    changed_container.push(0);
    require(
        !accepts(
            &amendment,
            path,
            container_sha,
            &changed_container,
            &current,
            &archive,
        ),
        "A13 container mutation was accepted",
    )?;
    let mut changed_current = current.clone();
    changed_current.push(0);
    require(
        !accepts(
            &amendment,
            path,
            container_sha,
            &container,
            &changed_current,
            &archive,
        ),
        "A13 source mutation was accepted",
    )?;
    require(
        !accepts(
            &amendment,
            "crates/ssx-infra/src/search.rs",
            container_sha,
            &container,
            &current,
            &archive,
        ),
        "A13 path substitution was accepted",
    )?;
    manifest["amended_source_paths"]
        .as_array_mut()
        .unwrap()
        .push(serde_json::json!("crates/ssx-infra/src/search.rs"));
    require(
        validate_scope(&manifest).is_err(),
        "A13 unlisted amendment was accepted",
    )
}
