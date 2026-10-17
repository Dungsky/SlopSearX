//! Explicit native T06 qualification process. Fake providers are never public startup defaults.
#[path = "../../tests/t06_support/fixture.rs"]
mod fixture;
use ssx_mcp::{Result, search_surface::SearchSurface};
#[tokio::main]
async fn main() -> Result<()> {
    let args: Vec<_> = std::env::args().skip(1).collect();
    let (mode, url, endpoint) = match args.as_slice() {
        [mode, url] => (mode, url, None),
        [mode, url, endpoint] => (mode, url, Some(endpoint)),
        _ => return Err(
            "usage: search-qualification stdio PRIVATE_VALKEY_URL [LOOPBACK_FIXTURE_ROUTING_URL]"
                .into(),
        ),
    };
    if mode != "stdio" {
        return Err("qualification requires stdio".into());
    }
    // Accept only this test suite's exclusive private directory, including a
    // platform-specific temp root. Never derive authority from a URL prefix.
    use std::os::unix::fs::{MetadataExt, PermissionsExt};
    let socket = std::path::Path::new(
        url.strip_prefix("unix://")
            .ok_or("private Unix socket required")?,
    );
    let parent = socket.parent().ok_or("private socket parent required")?;
    let metadata = std::fs::symlink_metadata(parent)?;
    let name = parent
        .file_name()
        .and_then(|s| s.to_str())
        .ok_or("private fixture directory name")?;
    let id = name
        .strip_prefix("ssx-t06-")
        .ok_or("private fixture directory prefix")?;
    if !metadata.is_dir()
        || metadata.file_type().is_symlink()
        || metadata.permissions().mode() & 0o777 != 0o700
        || metadata.uid() != std::fs::metadata(std::env::current_exe()?)?.uid()
        || id.len() != 32
        || !id.bytes().all(|b| b.is_ascii_hexdigit())
        || socket.file_name().and_then(|s| s.to_str()) != Some("s.sock")
    {
        return Err("qualification requires an owned exclusive private fixture socket".into());
    }
    let dispatch_receipt = std::sync::Arc::new(
        ssx_infra::valkey::Valkey::new(url, std::time::Duration::from_millis(300))
            .map_err(|e| format!("private dispatch receipt store: {e:?}"))?,
    );
    let fixture = if let Some(endpoint) = endpoint {
        let endpoint = url::Url::parse(endpoint)?;
        if endpoint.scheme() != "http"
            || endpoint.host_str() != Some("127.0.0.1")
            || endpoint.port().is_none()
            || !endpoint.username().is_empty()
            || endpoint.password().is_some()
        {
            return Err("fixture routing endpoint must be explicit plain loopback".into());
        }
        let router = std::sync::Arc::new(ssx_infra::jev::JevRouter::with_endpoint(
            ssx_infra::jev::JevConfig::from_environment(&std::collections::BTreeMap::from([
                ("TYPESAFE_API_KEY".into(), "fixture-only".into()),
                ("JEV_TIMEOUT_MS".into(), "10000".into()),
            ]))
            .unwrap(),
            ssx_registry::Registry::builtins()?,
            endpoint.into(),
        ));
        let engines = ["wikipedia", "pubmed"]
            .into_iter()
            .map(|name| fixture::FakeEngine {
                name: name.into(),
                calls: Default::default(),
                dispatch_receipt: Some(dispatch_receipt.clone()),
                count: 3,
                status: ssx_domain::EngineStatus::Ok,
                delay: std::time::Duration::ZERO,
                results: None,
            })
            .collect();
        fixture::Fixture::with_options(
            url,
            Default::default(),
            engines,
            &[],
            ssx_infra::search::SearchConfig {
                max_callers: 1,
                optional_port_timeout: std::time::Duration::from_secs(10),
                ..Default::default()
            },
            Some(router),
        )
        .await
    } else {
        let engines = ["wikipedia", "brave", "duckduckgo"]
            .into_iter()
            .map(|name| {
                Ok(fixture::FakeEngine {
                    name: name.into(),
                    calls: Default::default(),
                    dispatch_receipt: Some(dispatch_receipt.clone()),
                    count: 3,
                    status: ssx_domain::EngineStatus::Ok,
                    delay: std::time::Duration::ZERO,
                    results: None,
                })
            })
            .collect::<Result<Vec<_>>>()?;
        fixture::Fixture::with_engines(url, false, engines).await
    };
    let surface = fixture.surface.clone();
    ssx_mcp::opaque::stdio_with_factory(
        move |wire| surface.bind_connection(wire, None),
        SearchSurface::catalogs(),
    )
    .await?;
    fixture.service.shutdown().await;
    Ok(())
}
