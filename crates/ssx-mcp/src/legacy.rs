//! Legacy HTTP+SSE shim, separate from rmcp's streamable-HTTP SSE responses.
use crate::Registry;
use axum::{
    Router,
    extract::{DefaultBodyLimit, RawQuery, State},
    http::{HeaderMap, HeaderValue, StatusCode},
    response::{
        IntoResponse, Response,
        sse::{Event, Sse},
    },
    routing::{get, post},
};
use futures_util::stream;
use rmcp::ServiceExt;
use std::{
    collections::HashMap,
    convert::Infallible,
    sync::{Arc, Mutex},
    time::Duration,
};
use tokio::{
    io::{AsyncBufReadExt, AsyncWriteExt, BufReader},
    sync::mpsc,
};
use tokio_util::sync::CancellationToken;
#[derive(Clone)]
struct Session {
    input: mpsc::Sender<String>,
    cancel: CancellationToken,
}
#[derive(Clone)]
pub struct Legacy {
    registry: Registry,
    sessions: Arc<Mutex<HashMap<String, Session>>>,
    pub message_path: String,
    pub ttl: Duration,
    cancel: CancellationToken,
}
impl Legacy {
    pub fn new(
        registry: Registry,
        message_path: &str,
        ttl: Duration,
        cancel: CancellationToken,
    ) -> crate::Result<Self> {
        if !message_path.starts_with('/')
            || !message_path.ends_with('/')
            || message_path.contains('?')
            || message_path.contains('#')
        {
            return Err("legacy message prefix must be an absolute path ending /".into());
        }
        Ok(Self {
            registry,
            sessions: Arc::new(Mutex::new(HashMap::new())),
            message_path: message_path.into(),
            ttl,
            cancel,
        })
    }
    pub fn active_sessions(&self) -> usize {
        self.sessions.lock().unwrap().len()
    }
    pub fn router(self, sse_path: &str) -> crate::Result<Router> {
        if !sse_path.starts_with('/') || sse_path.ends_with('/') {
            return Err("invalid SSE path".into());
        }
        let prefix = self.message_path.trim_end_matches('/');
        Ok(Router::new()
            .route(sse_path, get(connect))
            .route(prefix, post(message))
            .route(&format!("{prefix}/"), post(message))
            .route(&format!("{prefix}/{{*suffix}}"), post(message))
            .layer(DefaultBodyLimit::max(4 * 1024 * 1024))
            .with_state(self))
    }
}
struct StreamState {
    rx: mpsc::Receiver<std::result::Result<Event, Infallible>>,
    session: String,
    state: Legacy,
}
impl Drop for StreamState {
    fn drop(&mut self) {
        if let Some(session) = self.state.sessions.lock().unwrap().remove(&self.session) {
            session.cancel.cancel();
        }
    }
}
async fn connect(State(state): State<Legacy>) -> Response {
    let id = uuid::Uuid::new_v4().simple().to_string();
    let cancel = state.cancel.child_token();
    let (input, mut incoming) = mpsc::channel::<String>(32);
    let (events, rx) = mpsc::channel(32);
    state.sessions.lock().unwrap().insert(
        id.clone(),
        Session {
            input,
            cancel: cancel.clone(),
        },
    );
    let endpoint = format!("{}?session_id={id}", state.message_path);
    let _ = events
        .send(Ok(Event::default().event("endpoint").data(endpoint)))
        .await;
    let (server_io, client_io) = tokio::io::duplex(65536);
    let (reader, mut writer) = tokio::io::split(client_io);
    let registry = state.registry.clone();
    let catalogs = registry.opaque.clone();
    let state_cleanup = state.clone();
    let id_cleanup = id.clone();
    let ct = cancel.clone();
    tokio::spawn(async move {
        let serving = async {
            let service = registry.serve(server_io).await?;
            service.waiting().await?;
            Ok::<(), crate::Error>(())
        };
        let pump = async {
            let mut lines = BufReader::new(reader).lines();
            let mut overlay = crate::opaque::Overlay::default();
            let mut lifecycle = crate::opaque::Lifecycle::default();
            let mut wire = crate::opaque::WireState::default();
            loop {
                tokio::select! {
                    input=incoming.recv()=>match input {Some(s)=>{if let Some(reply)=lifecycle.request(&s)?{if let Some(reply)=reply&& events.send(Ok(Event::default().event("message").data(reply))).await.is_err(){break;}continue;}let rewritten=wire.request(&s)?;lifecycle.remap(&s,&rewritten)?;let s=rewritten;overlay.request(&catalogs,&s)?;writer.write_all(s.as_bytes()).await?;writer.write_all(b"\n").await?;}None=>break},
                    line=lines.next_line()=>match line? {Some(s)=>{if lifecycle.response(&s)?{writer.write_all(crate::opaque::INITIALIZED).await?;}let s=overlay.response(&s)?;let s=wire.response(&s)?;if events.send(Ok(Event::default().event("message").data(s))).await.is_err(){break;}}None=>break}
                }
            }
            Ok::<(), crate::Error>(())
        };
        tokio::select! {_=ct.cancelled()=>{},_=tokio::time::sleep(state_cleanup.ttl)=>{},_=serving=>{},_=pump=>{}}
        state_cleanup.sessions.lock().unwrap().remove(&id_cleanup);
        ct.cancel();
    });
    let state = StreamState {
        rx,
        session: id,
        state,
    };
    let mut response = Sse::new(stream::unfold(state, |mut s| async move {
        s.rx.recv().await.map(|e| (e, s))
    }))
    .into_response();
    response.headers_mut().insert(
        "content-type",
        HeaderValue::from_static("text/event-stream; charset=utf-8"),
    );
    response
}
async fn message(
    State(state): State<Legacy>,
    RawQuery(query): RawQuery,
    headers: HeaderMap,
    body: std::result::Result<String, axum::extract::rejection::StringRejection>,
) -> Response {
    let body = match body {
        Ok(body) => body,
        Err(error) => {
            return if error.status() == StatusCode::PAYLOAD_TOO_LARGE {
                (StatusCode::PAYLOAD_TOO_LARGE, "Request body too large").into_response()
            } else {
                (StatusCode::BAD_REQUEST, "Could not parse message").into_response()
            };
        }
    };
    if headers
        .get("content-type")
        .and_then(|v| v.to_str().ok())
        .is_none_or(|v| !v.starts_with("application/json"))
    {
        return (StatusCode::BAD_REQUEST, "Invalid Content-Type header").into_response();
    }
    let id = query
        .and_then(|q| reqwest::Url::parse(&format!("http://localhost/?{q}")).ok())
        .and_then(|u| {
            u.query_pairs()
                .filter(|(k, _)| k == "session_id")
                .map(|(_, v)| v.into_owned())
                .last()
        });
    let Some(id) = id else {
        return (StatusCode::BAD_REQUEST, "session_id is required").into_response();
    };
    let id = match uuid::Uuid::parse_str(&id) {
        Ok(id) => id.simple().to_string(),
        Err(_) => return (StatusCode::BAD_REQUEST, "Invalid session ID").into_response(),
    };
    let session = state.sessions.lock().unwrap().get(&id).cloned();
    let Some(session) = session else {
        return (StatusCode::NOT_FOUND, "Could not find session").into_response();
    };
    // Validate the same envelope after a disposable lossless ID alias; the session
    // pump retains the real alias mapping through response/cancellation.
    let validation = crate::opaque::sdk_validation(&body);
    if validation
        .as_ref()
        .ok()
        .is_none_or(|text| serde_json::from_str::<rmcp::model::ClientJsonRpcMessage>(text).is_err())
    {
        return (StatusCode::BAD_REQUEST, "Could not parse message").into_response();
    }
    if session.input.send(body).await.is_err() {
        return (StatusCode::NOT_FOUND, "Could not find session").into_response();
    }
    (StatusCode::ACCEPTED, "Accepted").into_response()
}
