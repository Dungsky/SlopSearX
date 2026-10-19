//! Exact original/current binding for two reviewed storage-policy documents.
//! Coverage files and the historical A13 code-amendment chain stay immutable.
use sha2::{Digest, Sha256};

struct Binding {
    path: &'static str,
    before: &'static str,
    after: &'static str,
    archive_path: &'static str,
}
const BINDINGS: &[Binding] = &[
    Binding {
        path: "AGENTS.md",
        before: "d710c84ebad4582dc607cc3bdb806a5a5253259e4cabd5c515f51e55c3588af8",
        after: "7d681485699a897464cf248d22fa83bd81e7b85d96a0163f6c9dabc019f988f6",
        archive_path: "docs/rust-rewrite/storage-document-amendment/AGENTS.before.md",
    },
    Binding {
        path: "CONTRIBUTING.md",
        before: "2df6be141a3b06c662a4062af79eb074248ea70f297a7bf4c65432c10feba681",
        after: "5526fb36792e2e14c7fab4f476b557c63e3d9e8aaab324078152d732e9dd3016",
        archive_path: "docs/rust-rewrite/storage-document-amendment/CONTRIBUTING.before.md",
    },
];
fn digest(bytes: &[u8]) -> String {
    Sha256::digest(bytes)
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect()
}
pub fn is_path(path: &str) -> bool {
    BINDINGS.iter().any(|binding| binding.path == path)
}
pub fn verify(
    path: &str,
    original: &str,
    original_bytes: Option<usize>,
    current: &[u8],
    archive: &[u8],
) -> Result<(), String> {
    let binding = BINDINGS
        .iter()
        .find(|binding| binding.path == path)
        .ok_or("Unreviewed storage document path")?;
    if original != binding.before
        || digest(archive) != binding.before
        || original_bytes.is_some_and(|bytes| bytes != archive.len())
    {
        return Err("Storage document original/archive binding differs".into());
    }
    if digest(current) != binding.after {
        return Err("Reviewed storage document successor differs".into());
    }
    Ok(())
}
pub fn archive_path(path: &str) -> Result<&'static str, String> {
    BINDINGS
        .iter()
        .find(|binding| binding.path == path)
        .map(|binding| binding.archive_path)
        .ok_or_else(|| "Unreviewed storage document path".into())
}
