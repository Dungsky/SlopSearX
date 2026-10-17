//! Native envelope restoration; actual handler text is the only restoration source.
#[path = "t06_support/wire_handler.rs"]
mod wire_handler;
use serde_json::{json, value::RawValue};
use ssx_mcp::{
    Result,
    opaque::{Frames, Overlay},
    output_limits::OutputLimits,
};
use std::collections::BTreeMap;
fn request(id: u64) -> String {
    format!(
        r#"{{"jsonrpc":"2.0","id":{id},"method":"tools/call","params":{{"name":"native_wire_probe","arguments":{{}}}}}}"#
    )
}
fn response(id: u64, result: &str) -> String {
    format!(r#"{{"jsonrpc":"2.0","id":{id},"result":{result},"literal":18446744073709551617}}"#)
}
#[test]
fn unknown_result_and_unmarked_metadata_remain_literal() -> Result<()> {
    for result in [
        "null",
        "[1,null]",
        "18446744073709551617",
        r#""scalar""#,
        r#"{"_meta":null}"#,
        r#"{"_meta":[1],"structuredContent":null}"#,
    ] {
        let original = response(5, result);
        assert_eq!(Overlay::default().response(&original)?, original);
    }
    Ok(())
}
#[test]
fn marker_is_scrubbed_from_every_result_path_and_only_trusted_objects_restore() -> Result<()> {
    let catalogs = wire_handler::WireHandler::catalogs();
    for admitted in [false, true] {
        for marker in [false, true] {
            for error in [false, true] {
                let mut overlay = Overlay::default();
                if admitted {
                    overlay.request(&catalogs, &request(1))?;
                }
                let result = json!({"_meta":{"ssx/native-structured-text":marker,"literal":[null,"number"]},"isError":error,"structuredContent":{},"content":[{"type":"text","text":"{\"a\":18446744073709551617,\"b\":18446744073709551618,\"null\":null}"}]});
                let raw = overlay.response(&response(1, &result.to_string()))?;
                assert!(!raw.contains("ssx/native-structured-text"));
                assert!(raw.contains("literal"));
                let fields: BTreeMap<String, Box<RawValue>> = serde_json::from_str(&raw)?;
                let result: BTreeMap<String, Box<RawValue>> =
                    serde_json::from_str(fields["result"].get())?;
                assert_eq!(
                    result["structuredContent"]
                        .get()
                        .contains("18446744073709551618"),
                    admitted && marker && !error
                );
                assert!(!overlay.has_pending());
            }
        }
    }
    for content in [
        json!([]),
        json!([{"type":"image","data":"a","mimeType":"image/png"}]),
        json!([{"type":"text","text":"{}"},{"type":"text","text":"{}"}]),
        json!([{"type":"text","text":"null"}]),
        json!([{"type":"text","text":"malformed"}]),
    ] {
        let mut overlay = Overlay::default();
        overlay.request(&catalogs, &request(7))?;
        let result = json!({"_meta":{"ssx/native-structured-text":true},"structuredContent":{},"content":content});
        assert!(overlay.response(&response(7, &result.to_string())).is_err());
        assert!(!overlay.has_pending());
    }
    Ok(())
}
fn verify_lossless(raw: &str, expected: &str) -> Result<()> {
    assert!(!raw.contains("ssx/native-structured-text"));
    let fields: BTreeMap<String, Box<RawValue>> = serde_json::from_str(raw)?;
    let result: BTreeMap<String, Box<RawValue>> = serde_json::from_str(fields["result"].get())?;
    assert_eq!(result["structuredContent"].get(), expected);
    let content: Vec<serde_json::Value> = serde_json::from_str(result["content"].get())?;
    assert_eq!(content[0]["text"].as_str(), Some(expected));
    Ok(())
}
#[test]
fn large_deep_native_output_uses_raw_envelope_without_default_document_parser() -> Result<()> {
    for kind in ["ordinary", "quotes", "controls"] {
        let h = wire_handler::WireHandler::new(kind);
        let mut overlay = Overlay::default();
        overlay.request(&wire_handler::WireHandler::catalogs(), &request(1))?;
        let result = json!({"content":[{"type":"text","text":h.payload}],"structuredContent":{},"isError":false,"_meta":{"ssx/native-structured-text":true}});
        let restored = overlay.response(&response(1, &result.to_string()))?;
        assert!(restored.len() > 4 * 1024 * 1024);
        assert!(restored.len() < 32 * 1024 * 1024);
        verify_lossless(&restored, &h.payload)?;
    }
    Ok(())
}
#[tokio::test]
async fn typed_output_budget_preserves_default_and_rejects_one_byte_over() -> Result<()> {
    assert_eq!(OutputLimits::HTTP.bytes(), 4 * 1024 * 1024);
    assert_eq!(OutputLimits::LEGACY.bytes(), 16 * 1024 * 1024);
    assert!(OutputLimits::new(0).is_err());
    assert!(OutputLimits::new(64 * 1024 * 1024 + 1).is_err());
    let limits = OutputLimits::new(32 * 1024 * 1024)?;
    let at = vec![b'x'; limits.bytes() - 1];
    let mut at = at;
    at.push(b'\n');
    assert_eq!(
        Frames::with_limit(at.as_slice(), limits)
            .next()
            .await?
            .unwrap()
            .len(),
        limits.bytes() - 1
    );
    let over = vec![b'x'; limits.bytes() + 1];
    let error = Frames::with_limit(over.as_slice(), limits)
        .next()
        .await
        .unwrap_err()
        .to_string();
    assert!(error.contains(&limits.bytes().to_string()));
    assert!(
        Frames::new(over.as_slice())
            .next()
            .await
            .unwrap_err()
            .to_string()
            .contains("4194304")
    );
    Ok(())
}
fn init_request() -> String {
    r#"{"jsonrpc":"2.0","id":0,"method":"initialize","params":{"protocolVersion":"2025-11-25","capabilities":{},"clientInfo":{"name":"native-wire","version":"1"}}}"#.into()
}
async fn http_send(
    client: &reqwest::Client,
    url: &str,
    request: String,
) -> Result<reqwest::Response> {
    Ok(client
        .post(url)
        .header("accept", "application/json,text/event-stream")
        .header("mcp-protocol-version", "2025-11-25")
        .header("content-type", "application/json")
        .body(request)
        .send()
        .await?)
}
#[tokio::test]
async fn actual_http_native_large_output_and_one_over_profile_fail_closed() -> Result<()> {
    use ssx_mcp::{
        harness::{Host, http_router_for_with_limits},
        oauth::NetworkAuth,
    };
    use tokio_util::sync::CancellationToken;
    for kind in ["ordinary", "quotes", "controls"] {
        let h = wire_handler::WireHandler::new(kind);
        let cancel = CancellationToken::new();
        let auth = NetworkAuth::Static(ssx_domain::oauth::Secret::new("wire-token".into())?);
        let host = Host::start(
            http_router_for_with_limits(
                h.clone(),
                wire_handler::WireHandler::catalogs(),
                cancel.clone(),
                OutputLimits::new(32 * 1024 * 1024)?,
            )
            .map(|r| auth.mount(r)),
            cancel,
        )
        .await?;
        let url = format!("http://{}/mcp", host.address);
        let client = reqwest::Client::builder().no_proxy().build()?;
        assert_eq!(
            http_send(&client, &url, init_request()).await?.status(),
            401
        );
        let mut headers = reqwest::header::HeaderMap::new();
        headers.insert(reqwest::header::AUTHORIZATION, "Bearer wire-token".parse()?);
        let client = reqwest::Client::builder()
            .no_proxy()
            .default_headers(headers)
            .build()?;
        assert_eq!(
            http_send(&client, &url, init_request()).await?.status(),
            200
        );
        let response = http_send(&client, &url, request(1)).await?;
        assert_eq!(response.status(), 200);
        let raw = response.text().await?;
        verify_lossless(&raw, &h.payload)?;
        host.shutdown().await?;
    }
    // A profile below the exact output closes at the transport; marker bytes never escape.
    let h = wire_handler::WireHandler::new("ordinary");
    let cancel = CancellationToken::new();
    let host = Host::start(
        http_router_for_with_limits(
            h,
            wire_handler::WireHandler::catalogs(),
            cancel.clone(),
            OutputLimits::new(1024 * 1024)?,
        ),
        cancel,
    )
    .await?;
    let client = reqwest::Client::builder().no_proxy().build()?;
    let url = format!("http://{}/mcp", host.address);
    http_send(&client, &url, init_request()).await?;
    let response = http_send(&client, &url, request(1)).await?;
    assert_eq!(response.status(), 502);
    assert!(
        !response
            .text()
            .await?
            .contains("ssx/native-structured-text")
    );
    host.shutdown().await?;
    Ok(())
}
#[tokio::test]
async fn actual_stdio_native_large_output_with_empty_process_environment() -> Result<()> {
    use tokio::io::{AsyncBufReadExt, AsyncWriteExt};
    for kind in ["ordinary", "quotes", "controls"] {
        let h = wire_handler::WireHandler::new(kind);
        let mut child = tokio::process::Command::new(env!("CARGO_BIN_EXE_wire-qualification"))
            .args(["stdio", kind])
            .env_clear()
            .env("PATH", "")
            .stdin(std::process::Stdio::piped())
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::inherit())
            .kill_on_drop(true)
            .spawn()?;
        let mut input = child.stdin.take().unwrap();
        let mut output = tokio::io::BufReader::new(child.stdout.take().unwrap());
        for (i, request) in [init_request(), request(1)].into_iter().enumerate() {
            input.write_all(request.as_bytes()).await?;
            input.write_all(b"\n").await?;
            input.flush().await?;
            let mut raw = String::new();
            assert!(
                tokio::time::timeout(
                    std::time::Duration::from_secs(20),
                    output.read_line(&mut raw)
                )
                .await??
                    > 0
            );
            if i == 1 {
                verify_lossless(&raw, &h.payload)?;
            }
        }
        input.shutdown().await?;
        drop(input);
        assert!(
            tokio::time::timeout(std::time::Duration::from_secs(5), child.wait())
                .await??
                .success()
        );
    }
    Ok(())
}
#[test]
fn trusted_admission_extracts_only_raw_name_not_arbitrary_argument_numbers() -> Result<()> {
    let catalogs = wire_handler::WireHandler::catalogs();
    let mut overlay = Overlay::default();
    overlay.request(&catalogs,r#"{"jsonrpc":"2.0","id":1,"method":"tools/call","params":{"name":"native_wire_probe","arguments":{"unknown":1e400}}}"#)?;
    assert!(overlay.has_pending());
    let result = json!({"content":[{"type":"text","text":"{\"ok\":null}"}],"structuredContent":{},"_meta":{"ssx/native-structured-text":true}});
    assert!(
        overlay
            .response(&response(1, &result.to_string()))?
            .contains("\"structuredContent\":{\"ok\":null}")
    );
    assert!(!overlay.has_pending());
    for params in [
        "null",
        "[]",
        r#"{"name":1e400,"arguments":{}}"#,
        r#"{"name":"untrusted","arguments":{"x":1e400}}"#,
    ] {
        let request = format!(r#"{{"id":2,"method":"tools/call","params":{params}}}"#);
        let mut overlay = Overlay::default();
        overlay.request(&catalogs, &request)?;
        assert!(!overlay.has_pending());
    }
    Ok(())
}
