//! Loopback-only, injected-state host with explicit cancellation ownership.
use crate::{Registry, Result};
use axum::{
    Router,
    body::Body,
    http::{Request, StatusCode},
    middleware::{self, Next},
    response::{IntoResponse, Response},
};
use rmcp::transport::streamable_http_server::{
    StreamableHttpServerConfig, StreamableHttpService, session::local::LocalSessionManager,
};
use std::{net::SocketAddr, time::Duration};
use subtle::ConstantTimeEq;
use tokio_util::sync::CancellationToken;
async fn bearer(
    axum::extract::State(token): axum::extract::State<String>,
    request: Request<Body>,
    next: Next,
) -> Response {
    let authorized = request
        .headers()
        .get("authorization")
        .and_then(|v| v.to_str().ok())
        .and_then(|s| s.split_once(' '))
        .is_some_and(|(scheme, value)| {
            scheme.eq_ignore_ascii_case("bearer")
                && bool::from(value.as_bytes().ct_eq(token.as_bytes()))
        });
    if authorized {
        next.run(request).await
    } else {
        (
            StatusCode::UNAUTHORIZED,
            [("content-type", "text/plain; charset=utf-8")],
            "unauthorized",
        )
            .into_response()
    }
}
pub fn http_router(registry: Registry, token: Option<String>, cancel: CancellationToken) -> Router {
    http_router_mode(registry, token, cancel, false)
}
/// Additional SDK session qualification seam; production baseline remains stateless.
pub fn http_router_mode(
    registry: Registry,
    token: Option<String>,
    cancel: CancellationToken,
    stateful: bool,
) -> Router {
    let opaque = registry.opaque.clone();
    let service: StreamableHttpService<Registry, LocalSessionManager> = StreamableHttpService::new(
        move || Ok(registry.clone()),
        Default::default(),
        StreamableHttpServerConfig::default()
            .with_legacy_session_mode(stateful)
            .with_json_response(true)
            .with_cancellation_token(cancel),
    );
    let router = Router::new()
        .nest_service("/mcp", service)
        .layer(middleware::from_fn_with_state(
            OpaqueState {
                catalogs: opaque,
                sessions: Default::default(),
                admission: stateful.then(|| std::sync::Arc::new(tokio::sync::Semaphore::new(128))),
            },
            opaque_catalog,
        ));
    if let Some(token) = token.filter(|s| !s.is_empty()) {
        router.layer(middleware::from_fn_with_state(token, bearer))
    } else {
        router
    }
}
type SharedWire = std::sync::Arc<std::sync::Mutex<crate::opaque::WireState>>;
struct WireLease {
    wire: SharedWire,
    request: String,
}
impl Drop for WireLease {
    fn drop(&mut self) {
        self.wire.lock().unwrap().abandon(&self.request);
    }
}
#[derive(Clone)]
struct OpaqueState {
    admission: Option<std::sync::Arc<tokio::sync::Semaphore>>,
    catalogs: std::sync::Arc<crate::opaque::Catalogs>,
    sessions: std::sync::Arc<
        std::sync::Mutex<
            std::collections::HashMap<String, (SharedWire, tokio::sync::OwnedSemaphorePermit)>,
        >,
    >,
}
async fn opaque_catalog(
    axum::extract::State(state): axum::extract::State<OpaqueState>,
    request: Request<Body>,
    next: Next,
) -> Response {
    let session = request
        .headers()
        .get("mcp-session-id")
        .and_then(|s| s.to_str().ok())
        .map(str::to_owned);
    // Only server-issued sessions are retained; arbitrary client headers never
    // allocate persistent alias state or consume session admission capacity.
    let wire = session
        .as_ref()
        .and_then(|id| {
            state
                .sessions
                .lock()
                .unwrap()
                .get(id)
                .map(|entry| entry.0.clone())
        })
        .unwrap_or_default();
    if request.method() == axum::http::Method::DELETE {
        let response = next.run(request).await;
        if let Some(session) = session {
            state.sessions.lock().unwrap().remove(&session);
        }
        return response;
    }
    if request.method() != axum::http::Method::POST {
        return next.run(request).await;
    }
    let (parts, body) = request.into_parts();
    let bytes = match axum::body::to_bytes(body, 4 * 1024 * 1024).await {
        Ok(bytes) => bytes,
        Err(_) => return StatusCode::PAYLOAD_TOO_LARGE.into_response(),
    };
    let mut overlay = crate::opaque::Overlay::default();
    let bytes = if let Ok(text) = std::str::from_utf8(&bytes) {
        match wire.lock().unwrap().request(text) {
            Ok(rewritten) => {
                let _ = overlay.request(&state.catalogs, &rewritten);
                axum::body::Bytes::from(rewritten)
            }
            Err(_) => return StatusCode::BAD_REQUEST.into_response(),
        }
    } else {
        bytes
    };
    let initializing = session.is_none()
        && serde_json::from_slice::<
            std::collections::BTreeMap<String, Box<serde_json::value::RawValue>>,
        >(&bytes)
        .ok()
        .is_some_and(|fields| {
            fields
                .get("method")
                .is_some_and(|m| m.get() == "\"initialize\"")
        });
    let admission = if initializing {
        match &state.admission {
            Some(semaphore) => match semaphore.clone().try_acquire_owned() {
                Ok(permit) => Some(permit),
                Err(_) => return StatusCode::SERVICE_UNAVAILABLE.into_response(),
            },
            None => None,
        }
    } else {
        None
    };
    let lease = WireLease {
        wire: wire.clone(),
        request: String::from_utf8_lossy(&bytes).into_owned(),
    };
    let response = next
        .run(Request::from_parts(parts, Body::from(bytes)))
        .await;
    if response.status() != StatusCode::OK {
        return response;
    }
    if let Some(admission) = admission
        && let Some(issued) = response
            .headers()
            .get("mcp-session-id")
            .and_then(|s| s.to_str().ok())
    {
        state
            .sessions
            .lock()
            .unwrap()
            .insert(issued.to_owned(), (wire.clone(), admission));
    }
    let (mut parts, body) = response.into_parts();
    if parts
        .headers
        .get("content-type")
        .and_then(|h| h.to_str().ok())
        .is_some_and(|h| h.starts_with("text/event-stream"))
    {
        use futures_util::StreamExt;
        let stream = futures_util::stream::try_unfold(
            (
                body.into_data_stream(),
                Vec::<u8>::new(),
                overlay,
                wire,
                lease,
            ),
            |(mut stream, mut buffer, mut overlay, wire, lease)| async move {
                loop {
                    let delimiter = buffer
                        .windows(4)
                        .position(|p| p == b"\r\n\r\n")
                        .map(|p| (p, 4))
                        .or_else(|| buffer.windows(2).position(|p| p == b"\n\n").map(|p| (p, 2)));
                    if let Some((end, n)) = delimiter {
                        let frame: Vec<_> = buffer.drain(..end + n).collect();
                        let text = String::from_utf8(frame).map_err(std::io::Error::other)?;
                        let rewritten = crate::opaque::overlay_event(&mut overlay, &text)
                            .map_err(|e| std::io::Error::other(e.to_string()))?;
                        let rewritten =
                            crate::opaque::wire_event(&mut wire.lock().unwrap(), &rewritten)
                                .map_err(|e| std::io::Error::other(e.to_string()))?;
                        return Ok::<_, std::io::Error>(Some((
                            axum::body::Bytes::from(rewritten),
                            (stream, buffer, overlay, wire, lease),
                        )));
                    }
                    let Some(chunk) = stream.next().await else {
                        if buffer.is_empty() {
                            return Ok(None);
                        }
                        return Err(std::io::Error::other("incomplete MCP SSE frame"));
                    };
                    let chunk = chunk.map_err(std::io::Error::other)?;
                    if buffer.len() + chunk.len() > 4 * 1024 * 1024 {
                        return Err(std::io::Error::other("MCP SSE frame exceeds 4 MiB"));
                    }
                    buffer.extend_from_slice(&chunk);
                }
            },
        );
        parts.headers.remove("content-length");
        return Response::from_parts(parts, Body::from_stream(stream));
    }
    let bytes = match axum::body::to_bytes(body, 4 * 1024 * 1024).await {
        Ok(bytes) => bytes,
        Err(_) => return StatusCode::BAD_GATEWAY.into_response(),
    };
    let replacement = std::str::from_utf8(&bytes)
        .ok()
        .and_then(|s| overlay.response(s).ok())
        .and_then(|s| wire.lock().unwrap().response(&s).ok())
        .unwrap_or_else(|| String::from_utf8_lossy(&bytes).into_owned());
    parts.headers.remove("content-length");
    Response::from_parts(parts, Body::from(replacement))
}
pub struct Host {
    pub address: SocketAddr,
    pub cancel: CancellationToken,
    task: Option<tokio::task::JoinHandle<std::io::Result<()>>>,
}
impl Host {
    pub async fn start(router: Router, cancel: CancellationToken) -> Result<Self> {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await?;
        let address = listener.local_addr()?;
        let ct = cancel.clone();
        let task = tokio::spawn(async move {
            axum::serve(listener, router)
                .with_graceful_shutdown(ct.cancelled_owned())
                .await
        });
        Ok(Self {
            address,
            cancel,
            task: Some(task),
        })
    }
    pub async fn shutdown(mut self) -> Result<()> {
        self.cancel.cancel();
        if let Some(mut task) = self.task.take()
            && tokio::time::timeout(Duration::from_secs(3), &mut task)
                .await
                .is_err()
        {
            task.abort();
            let _ = task.await;
            return Err("host shutdown deadline".into());
        }
        Ok(())
    }
}
impl Drop for Host {
    fn drop(&mut self) {
        self.cancel.cancel();
        if let Some(task) = self.task.take() {
            task.abort();
        }
    }
}
