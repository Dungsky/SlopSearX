//! Native source-drift gate: reads frozen bytes, never executes Python.
use sha2::{Digest, Sha256};
use std::path::Path;
#[test]
fn original_profiles_strategies_tool_surface_and_observations_match_reviewed_source_bindings() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let bindings: serde_json::Value = serde_json::from_str(include_str!(
        "../../../docs/rust-rewrite/t06/source_bindings_v2.json"
    ))
    .unwrap();
    for key in ["sources", "artifacts"] {
        for row in bindings[key].as_array().unwrap() {
            let name = row["file"].as_str().unwrap();
            let bytes = std::fs::read(root.join(name)).unwrap();
            assert_eq!(
                Sha256::digest(bytes)
                    .iter()
                    .map(|b| format!("{b:02x}"))
                    .collect::<String>(),
                row["sha256"].as_str().unwrap(),
                "reviewed source/capture drift: {name}"
            );
        }
    }
    for required in [
        "slopsearx/capabilities.py",
        "slopsearx/research.py",
        "slopsearx/jev.py",
        "slopsearx/mcp/security.py",
    ] {
        assert!(
            bindings["sources"]
                .as_array()
                .unwrap()
                .iter()
                .any(|row| row["file"] == required),
            "missing static profiles/strategies/topology binding"
        );
    }
    assert_eq!(unicode_normalization::UNICODE_VERSION, (16, 0, 0));
}
