//! Executable T01 qualification, independent of Python at check/runtime boundaries.
use crate::{Result, command, load_json};
use serde_json::Value;
use std::path::Path;
pub fn check(root: &Path, records: &mut Vec<Value>) -> Result<()> {
    let fixture = load_json(&root.join("crates/ssx-mcp/fixtures/python-sse.json"))?;
    if fixture["schema_version"] != 1
        || fixture["baseline"] != "7b2d876307189d7a62000d1f289b3ef4b65eaff6"
        || fixture["versions"]["fastmcp"] != "4.0.11"
        || fixture["versions"]["mcp"] != "2.3.0"
        || !fixture["python"]
            .as_str()
            .is_some_and(|p| p.starts_with("3.14.7 "))
    {
        return Err("T01 oracle identity missing or changed".into());
    }
    let sources = fixture["sources"]
        .as_object()
        .ok_or("T01 source provenance missing")?;
    for path in [
        "slopsearx/mcp/server.py",
        "slopsearx/mcp/harness.py",
        "slopsearx/mcp/security.py",
        "uv.lock",
    ] {
        let pin = command(
            root,
            "git",
            &[
                "rev-parse",
                &format!("7b2d876307189d7a62000d1f289b3ef4b65eaff6:{path}"),
            ],
            records,
        )?;
        if sources.get(path).and_then(Value::as_str) != Some(pin.trim()) {
            return Err(format!("T01 oracle source pin differs: {path}"));
        }
    }
    for (path, key) in [
        ("docs/rust-rewrite/replay/oracle/export_mcp.py", "git_blob"),
        (
            "docs/rust-rewrite/replay/oracle/export.py",
            "f04_helper_git_blob",
        ),
    ] {
        let actual = command(root, "git", &["hash-object", "--", path], records)?;
        if fixture["oracle"][key].as_str() != Some(actual.trim()) {
            return Err(format!("T01 exporter provenance differs: {path}"));
        }
    }
    let listed = command(
        root,
        "cargo",
        &["test", "--locked", "-p", "ssx-mcp", "--", "--list"],
        records,
    )?;
    let ignored = command(
        root,
        "cargo",
        &[
            "test",
            "--locked",
            "-p",
            "ssx-mcp",
            "--",
            "--ignored",
            "--list",
        ],
        records,
    )?;
    for name in [
        "harness::lifecycle_tests::sdk_close_lock_timeout_retains_alias_admission_until_actual_removal",
        "opaque::tests::rejected_requests_do_not_leak_admission_capacity",
        "opaque::tests::adjacent_large_request_ids_do_not_alias_and_sdk_ids_restore_exactly",
        "opaque::tests::newline_free_frame_is_rejected_before_eof_or_unbounded_allocation",
        "opaque::tests::compaction_preserves_strings_numbers_and_reserved_objects",
        "descriptor_comparison_rejects_nested_schema_defaults_nullable_and_metadata_mutation",
        "complete_captured_public_catalog_roundtrips_through_pinned_sdk_and_http",
        "typed_sdk_large_number_limitation_is_characterized_and_not_mistaken_for_parity",
        "unknown_schema_metadata_objects_and_refs_are_preserved_without_numeric_reinterpretation",
        "legacy_request_cancellation_releases_inflight_work_and_sessions",
        "real_stdio_subprocess_preserves_complete_contract_and_clean_shutdown",
        "real_streamable_http_bearer_and_stateless_contract",
        "real_stdio_gateway_forwards_to_remote_and_owns_session",
        "real_legacy_sse_matches_observed_python_status_and_payloads",
        "legacy_custom_paths_suffixes_method_content_type_and_expiry",
        "real_http_discovery_limits_and_startup_deadlines_leave_no_ready_gateway",
        "real_http_request_disconnect_cancels_handler_and_releases_state",
        "legacy_unsupported_version_negotiates_and_jsonrpc_errors_remain_errors",
        "opaque_large_integer_and_reserved_object_survive_real_http_stdio_sse_and_gateway",
        "selected_raw_gateway_owns_stateful_session_and_closes_it",
        "selected_raw_gateway_request_cancellation_reaches_remote_handler",
        "selected_raw_gateway_accepts_incremental_sse_and_rejects_bad_remote_version",
        "selected_raw_gateway_real_discovery_limits_and_deadline_fail_closed",
        "selected_stdio_source_lifecycle_and_adjacent_big_ids",
        "legacy_source_lifecycle_matches_captured_phase_errors",
        "real_http_and_legacy_preserve_adjacent_large_request_ids",
        "invalid_http_session_flood_does_not_consume_accepted_session_capacity",
        "rejected_delete_retains_actual_sdk_session_aliases_and_capacity",
        "typed_gateway_cancels_tools_resources_and_prompts_upstream",
        "stateful_idle_expiry_reclaims_sdk_aliases_and_permits_but_keeps_active_work",
        "sdk_termination_and_abrupt_host_or_router_drop_drain_actual_session_state",
        "malformed_call_shapes_do_not_poison_stateful_request_admission",
        "unbounded_integer_echo_retains_source_tokens_and_rejects_bool_fractional_and_failure",
        "typed_and_raw_catalogs_use_resource_uri_identity",
        "raw_gateway_uses_negotiated_older_protocol_on_every_http_operation",
    ] {
        if !listed.lines().any(|line| line == format!("{name}: test")) {
            return Err(format!("missing T01 acceptance case: {name}"));
        }
        if ignored.lines().any(|line| line == format!("{name}: test")) {
            return Err(format!("ignored mandatory T01 acceptance case: {name}"));
        }
    }
    command(
        root,
        "cargo",
        &["test", "--locked", "-p", "ssx-mcp"],
        records,
    )?;
    Ok(())
}
