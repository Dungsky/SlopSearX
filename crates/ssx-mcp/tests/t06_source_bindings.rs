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
    let native: serde_json::Value = serde_json::from_str(include_str!(
        "../../../docs/rust-rewrite/t06/native_fixture_bindings.json"
    ))
    .unwrap();
    let paths: std::collections::BTreeSet<_> = native["files"]
        .as_array()
        .unwrap()
        .iter()
        .map(|row| row["file"].as_str().unwrap())
        .collect();
    assert_eq!(
        paths,
        std::collections::BTreeSet::from([
            "crates/ssx-mcp/src/bin/search-qualification.rs",
            "crates/ssx-mcp/tests/t06_support/fixture.rs",
            "crates/ssx-mcp/tests/t06_support/observer.rs",
            "crates/ssx-mcp/tests/t06_support/server.rs",
            "crates/ssx-infra/src/search.rs",
            "crates/ssx-infra/src/search_deadline_drain_tests.rs",
            "crates/ssx-mcp/src/opaque.rs",
            "crates/ssx-mcp/tests/t06_support/deadline_clock.rs",
            "crates/ssx-mcp/tests/t06_support/wall_guard.rs",
            "crates/ssx-mcp/tests/t06_transport.rs",
        ])
    );
    for row in native["files"].as_array().unwrap() {
        let name = row["file"].as_str().unwrap();
        assert_eq!(
            Sha256::digest(std::fs::read(root.join(name)).unwrap())
                .iter()
                .map(|b| format!("{b:02x}"))
                .collect::<String>(),
            row["sha256"].as_str().unwrap(),
            "reviewed native fixture drift: {name}"
        );
    }
    assert_eq!(unicode_normalization::UNICODE_VERSION, (16, 0, 0));
}
