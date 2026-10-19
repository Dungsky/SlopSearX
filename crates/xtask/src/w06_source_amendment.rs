//! Exact, additive reconciliation for the one W07-owned source added after W06.
use crate::{Result, command};
use serde::Deserialize;
use serde_json::Value;
use sha2::{Digest, Sha256};
use std::{fs, path::Path};

const MANIFEST_PATH: &str =
    "docs/rust-rewrite/w06/source-amendments/current-source-amendment-v1.json";
const SOURCE_PATH: &str = "crates/ssx-application/src/lib.rs";
const PARENT_ARCHIVE_PATH: &str =
    "docs/rust-rewrite/w06/source-amendments/de04-pre-w07/crates/ssx-application/src/lib.rs";
const MANIFEST_SHA256: &str = "efc0c4f1a767e4850800f48d632ddb2c6827d2b8c6c9ddb96003785f31200d30";
const HISTORICAL_SHA256: &str = "a7102f8c75244e74680cf8773c527d21fb2dcd177a29f0bd27abeef2beeb880e";
const PARENT_REVISION: &str = "de04ab2e4263616dfd22ad6c9dc45aeb94f9bea4";
const PARENT_GIT_BLOB: &str = "14df2ec4ba0f1f03e18bb7071c5646536a45a773";
const CURRENT_GIT_BLOB: &str = "45dbed9d4d1f3ee84709c1f73026e19ac9dcb64d";
const ALLOWED_APPEND: &[u8] = b"\npub mod lineage;\npub mod receipts;\n";

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Amendment {
    schema_version: u32,
    plan_id: String,
    purpose: String,
    source_path: String,
    historical_pin: HistoricalPin,
    successor: Successor,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct HistoricalPin {
    sha256: String,
    bytes: usize,
    git_revision: String,
    git_blob: String,
    archive_path: String,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Successor {
    parent_revision: String,
    parent_archive_path: String,
    parent_sha256: String,
    parent_bytes: usize,
    parent_git_blob: String,
    allowed_append_utf8: String,
    allowed_append_bytes: usize,
    allowed_append_sha256: String,
    sha256: String,
    bytes: usize,
    git_blob: String,
}

fn sha256(bytes: &[u8]) -> String {
    Sha256::digest(bytes)
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect()
}
fn reject(message: &str) -> Result<()> {
    Err(format!("W06 source amendment: {message}"))
}

fn validate_materials(
    manifest_bytes: &[u8],
    historical_expected: &str,
    historical_archive: &[u8],
    parent_archive: &[u8],
    current: &[u8],
) -> Result<()> {
    if sha256(manifest_bytes) != MANIFEST_SHA256 {
        return reject("manifest identity changed");
    }
    let amendment: Amendment = serde_json::from_slice(manifest_bytes)
        .map_err(|_| "W06 source amendment: malformed manifest")?;
    let historical = amendment.historical_pin;
    let successor = amendment.successor;
    if amendment.schema_version != 1
        || amendment.plan_id != "W06"
        || amendment.purpose.is_empty()
        || amendment.source_path != SOURCE_PATH
        || historical_expected != HISTORICAL_SHA256
        || historical.sha256 != HISTORICAL_SHA256
        || historical.bytes != 5525
        || historical.git_revision != PARENT_REVISION
        || historical.git_blob != PARENT_GIT_BLOB
        || historical.archive_path != PARENT_ARCHIVE_PATH
    {
        return reject("historical pin or amendment scope changed");
    }
    if sha256(historical_archive) != historical.sha256
        || historical_archive.len() != historical.bytes
    {
        return reject("archived pre-W07 source does not match frozen pin");
    }
    if successor.parent_revision != PARENT_REVISION
        || successor.parent_archive_path != PARENT_ARCHIVE_PATH
        || successor.parent_sha256 != HISTORICAL_SHA256
        || successor.parent_bytes != 5525
        || successor.parent_git_blob != PARENT_GIT_BLOB
        || successor.allowed_append_utf8.as_bytes() != ALLOWED_APPEND
        || successor.allowed_append_bytes != ALLOWED_APPEND.len()
        || successor.allowed_append_sha256 != sha256(ALLOWED_APPEND)
    {
        return reject("parent or allowed delta changed");
    }
    if sha256(parent_archive) != successor.parent_sha256
        || parent_archive.len() != successor.parent_bytes
        || historical_archive != parent_archive
    {
        return reject("pre-W07 archive does not match the frozen pin");
    }
    if successor.sha256 != "1ff0647013d66e6711587bbd103659959ab860bd3bd2d2b42d10b7a640a5544e"
        || successor.bytes != 5561
        || successor.git_blob != CURRENT_GIT_BLOB
    {
        return reject("successor identity changed");
    }
    let mut expected = Vec::with_capacity(parent_archive.len() + ALLOWED_APPEND.len());
    expected.extend_from_slice(parent_archive);
    expected.extend_from_slice(ALLOWED_APPEND);
    if current != expected
        || sha256(current) != successor.sha256
        || current.len() != successor.bytes
    {
        return reject("live source is not the exact reviewed successor");
    }
    Ok(())
}

pub(super) fn verify(
    root: &Path,
    historical_expected: &str,
    records: &mut Vec<Value>,
) -> Result<()> {
    let manifest = fs::read(root.join(MANIFEST_PATH))
        .map_err(|error| format!("W06 source amendment manifest: {error}"))?;
    let archive = fs::read(root.join(PARENT_ARCHIVE_PATH))
        .map_err(|error| format!("W06 source amendment archive: {error}"))?;
    let current =
        fs::read(root.join(SOURCE_PATH)).map_err(|error| format!("W06 amended source: {error}"))?;
    validate_materials(&manifest, historical_expected, &archive, &archive, &current)?;
    let parent_spec = format!("{PARENT_REVISION}:{SOURCE_PATH}");
    let current_spec = format!("HEAD:{SOURCE_PATH}");
    let parent_blob = command(root, "git", &["rev-parse", &parent_spec], records)?;
    let current_blob = command(root, "git", &["rev-parse", &current_spec], records)?;
    if parent_blob.trim() != PARENT_GIT_BLOB || current_blob.trim() != CURRENT_GIT_BLOB {
        return reject("historical or current Git blob identity changed");
    }
    negative_controls(&manifest, historical_expected, &archive, &current)
}

fn negative_controls(
    manifest: &[u8],
    historical_expected: &str,
    archive: &[u8],
    current: &[u8],
) -> Result<()> {
    let must_reject = |label: &str,
                       manifest: &[u8],
                       expected: &str,
                       frozen: &[u8],
                       parent: &[u8],
                       live: &[u8]| {
        if validate_materials(manifest, expected, frozen, parent, live).is_ok() {
            return reject(&format!("negative control was accepted: {label}"));
        }
        Ok(())
    };
    let mut altered_manifest = manifest.to_vec();
    altered_manifest[0] = b' ';
    must_reject(
        "manifest byte",
        &altered_manifest,
        historical_expected,
        archive,
        archive,
        current,
    )?;
    must_reject(
        "historical pin",
        manifest,
        "0".repeat(64).as_str(),
        archive,
        archive,
        current,
    )?;
    let mut altered_archive = archive.to_vec();
    altered_archive[0] ^= 1;
    must_reject(
        "archived source byte",
        manifest,
        historical_expected,
        &altered_archive,
        archive,
        current,
    )?;
    must_reject(
        "parent archive byte",
        manifest,
        historical_expected,
        archive,
        &altered_archive,
        current,
    )?;
    let mut extra_source = current.to_vec();
    extra_source.extend_from_slice(b"\npub mod unreviewed;\n");
    must_reject(
        "unreviewed source addition",
        manifest,
        historical_expected,
        archive,
        archive,
        &extra_source,
    )
}
