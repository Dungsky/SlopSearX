//! Adapter-owned sockets and search-owned identity. No provider retries or parsers.
use reqwest::{
    Client, Method, Url,
    cookie::{CookieStore, Jar},
    header::{self, HeaderMap, HeaderValue},
};
use std::{
    collections::HashMap,
    future::Future,
    io::Read,
    pin::Pin,
    sync::{Arc, Mutex, Weak},
    time::{Duration, Instant},
};
use tokio::sync::{Notify, Semaphore, watch};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Error {
    Closed,
    Cancelled,
    Deadline,
    RequestLimit,
    RequestBodyLimit,
    WireBodyLimit,
    DecodedBodyLimit,
    RedirectLimit,
    InvalidRequest,
    InvalidResponse,
    UnsupportedEncoding,
    Network,
}
pub type Result<T> = std::result::Result<T, Error>;
pub type ExchangeFuture<'a> = Pin<Box<dyn Future<Output = Result<Response>> + Send + 'a>>;
/// Caller-owned transports are borrowed and never closed by this module.
pub trait Transport: Send + Sync {
    fn exchange(&self, request: Request) -> ExchangeFuture<'_>;
}
#[derive(Clone, Debug)]
pub struct Request {
    pub method: Method,
    pub url: Url,
    pub headers: HeaderMap,
    pub body: Vec<u8>,
}
impl Request {
    pub fn get(url: &str) -> Result<Self> {
        Ok(Self {
            method: Method::GET,
            url: Url::parse(url).map_err(|_| Error::InvalidRequest)?,
            headers: HeaderMap::new(),
            body: Vec::new(),
        })
    }
}
#[derive(Clone, Debug)]
pub struct Response {
    pub status: u16,
    pub url: Url,
    pub headers: HeaderMap,
    pub body: Vec<u8>,
}
/// New migration safety bounds; adapters can select explicit, finite values.
#[derive(Clone, Debug)]
pub struct Bounds {
    pub wire_bytes: usize,
    pub decoded_bytes: usize,
    pub request_bytes: usize,
    pub requests: usize,
    pub redirects: usize,
}
impl Default for Bounds {
    fn default() -> Self {
        Self {
            wire_bytes: 8 * 1024 * 1024,
            decoded_bytes: 32 * 1024 * 1024,
            request_bytes: 2 * 1024 * 1024,
            requests: 16,
            redirects: 10,
        }
    }
}
#[derive(Clone, Debug)]
pub struct Options {
    pub proxy: Option<String>,
    pub trust_env: bool,
    pub verify_tls: bool,
    pub http2: bool,
    pub root_certificates: Vec<reqwest::Certificate>,
    pub trust_roots_only: bool,
    pub identity: Option<reqwest::Identity>,
    pub follow_redirects: bool,
    pub bounds: Bounds,
}
impl Default for Options {
    fn default() -> Self {
        Self {
            proxy: None,
            trust_env: true,
            verify_tls: true,
            http2: false,
            root_certificates: Vec::new(),
            trust_roots_only: false,
            identity: None,
            follow_redirects: false,
            bounds: Bounds::default(),
        }
    }
}
#[derive(Clone)]
pub struct Cancellation(watch::Sender<bool>);
impl Default for Cancellation {
    fn default() -> Self {
        Self(watch::channel(false).0)
    }
}
impl Cancellation {
    pub fn cancel(&self) {
        self.0.send_replace(true);
    }
    async fn cancelled(&self) {
        let mut rx = self.0.subscribe();
        loop {
            if *rx.borrow_and_update() {
                return;
            }
            if rx.changed().await.is_err() {
                return;
            }
        }
    }
}
struct Idle {
    origin: String,
    client: Client,
    used: Instant,
}
struct Pool {
    slots: Mutex<Vec<(bool, Option<Idle>)>>,
    permits: Arc<Semaphore>,
    options: Options,
}
impl Pool {
    fn new(options: Options) -> Self {
        Self {
            slots: Mutex::new((0..10).map(|_| (false, None)).collect()),
            permits: Arc::new(Semaphore::new(20)),
            options,
        }
    }
    fn checkout(self: &Arc<Self>, url: &Url) -> Result<Checkout> {
        let origin = url.origin().ascii_serialization();
        let mut slots = self.slots.lock().unwrap();
        if self.permits.is_closed() {
            return Err(Error::Closed);
        }
        // At most ten idle clients, each permanently tied to one origin. The other
        // ten concurrent requests use non-retained clients. This enforces a global
        // cap instead of mistaking reqwest's per-host setting for a global cap.
        let slot = slots
            .iter()
            .position(|(busy, s)| {
                !busy
                    && s.as_ref().is_some_and(|i| {
                        i.origin == origin && i.used.elapsed() < Duration::from_secs(5)
                    })
            })
            .or_else(|| slots.iter().position(|(busy, s)| !busy && s.is_none()))
            .or_else(|| {
                slots
                    .iter()
                    .enumerate()
                    .filter(|(_, (busy, _))| !*busy)
                    .filter_map(|(n, (_, s))| s.as_ref().map(|i| (n, i.used)))
                    .min_by_key(|(_, t)| *t)
                    .map(|(n, _)| n)
            });
        let client = if let Some(n) = slot {
            let client = match slots[n].1.take() {
                Some(i) if i.origin == origin && i.used.elapsed() < Duration::from_secs(5) => {
                    i.client
                }
                _ => build_client(&self.options)?,
            };
            slots[n].0 = true;
            client
        } else {
            build_client(&self.options)?
        };
        Ok(Checkout {
            pool: self.clone(),
            slot,
            origin,
            client: Some(client),
        })
    }
    fn close(&self) {
        self.permits.close();
        self.slots
            .lock()
            .unwrap()
            .iter_mut()
            .for_each(|s| s.1 = None);
    }
}
struct Checkout {
    pool: Arc<Pool>,
    slot: Option<usize>,
    origin: String,
    client: Option<Client>,
}
impl Drop for Checkout {
    fn drop(&mut self) {
        if let Some(n) = self.slot {
            let mut slots = self.pool.slots.lock().unwrap();
            if self.pool.permits.is_closed() {
                return;
            }
            slots[n] = (
                false,
                Some(Idle {
                    origin: self.origin.clone(),
                    client: self.client.take().unwrap(),
                    used: Instant::now(),
                }),
            );
        }
    }
}
fn build_client(o: &Options) -> Result<Client> {
    let mut b = Client::builder()
        .redirect(reqwest::redirect::Policy::none())
        .retry(reqwest::retry::never())
        .pool_max_idle_per_host(1)
        .pool_idle_timeout(Duration::from_secs(5))
        .danger_accept_invalid_certs(!o.verify_tls)
        .no_gzip()
        .no_brotli()
        .no_deflate()
        .no_zstd();
    if !o.root_certificates.is_empty() {
        b = if o.trust_roots_only {
            b.tls_certs_only(o.root_certificates.clone())
        } else {
            b.tls_certs_merge(o.root_certificates.clone())
        };
    }
    if let Some(identity) = &o.identity {
        b = b.identity(identity.clone());
    }
    if !o.http2 {
        b = b.http1_only();
    }
    if !o.trust_env {
        b = b.no_proxy();
    }
    if let Some(proxy) = &o.proxy {
        b = b.proxy(reqwest::Proxy::all(proxy).map_err(|_| Error::InvalidRequest)?);
    }
    b.build().map_err(|_| Error::InvalidRequest)
}
struct State {
    closed: bool,
    used: bool,
    pools: HashMap<Option<String>, Arc<Pool>>,
    injected: Option<Arc<dyn Transport>>,
    all_pools: Vec<Weak<Pool>>,
}
pub struct AdapterHttp {
    decoder: Arc<Decoder>,
    state: Mutex<State>,
    stop: Cancellation,
}
impl Default for AdapterHttp {
    fn default() -> Self {
        Self {
            decoder: Arc::new(Decoder::default()),
            state: Mutex::new(State {
                closed: false,
                used: false,
                pools: HashMap::new(),
                injected: None,
                all_pools: Vec::new(),
            }),
            stop: Cancellation::default(),
        }
    }
}
impl AdapterHttp {
    pub fn set_transport(&self, t: Arc<dyn Transport>) -> Result<()> {
        let mut s = self.state.lock().unwrap();
        if s.closed || s.used {
            return Err(Error::Closed);
        }
        s.injected = Some(t);
        Ok(())
    }
    pub fn session(
        &self,
        mut options: Options,
        deadline: tokio::time::Instant,
        cancel: Cancellation,
    ) -> Result<Session> {
        let mut s = self.state.lock().unwrap();
        if s.closed {
            return Err(Error::Closed);
        }
        if options.trust_roots_only && options.root_certificates.is_empty() {
            return Err(Error::InvalidRequest);
        }
        s.used = true;
        if s.injected.is_none()
            && options.verify_tls
            && options.trust_env
            && options.root_certificates.is_empty()
        {
            if let Some(path) = std::env::var_os("SSL_CERT_FILE") {
                let pem = std::fs::read(path).map_err(|_| Error::InvalidRequest)?;
                let certs = reqwest::Certificate::from_pem_bundle(&pem)
                    .map_err(|_| Error::InvalidRequest)?;
                if certs.is_empty() {
                    return Err(Error::InvalidRequest);
                }
                options.root_certificates.extend(certs);
                options.trust_roots_only = true;
            } else if let Some(path) = std::env::var_os("SSL_CERT_DIR") {
                for entry in std::fs::read_dir(path).map_err(|_| Error::InvalidRequest)? {
                    let entry = entry.map_err(|_| Error::InvalidRequest)?;
                    if entry.path().is_file() {
                        let pem = std::fs::read(entry.path()).map_err(|_| Error::InvalidRequest)?;
                        if let Ok(certs) = reqwest::Certificate::from_pem_bundle(&pem) {
                            options.root_certificates.extend(certs);
                        }
                    }
                }
                if options.root_certificates.is_empty() {
                    return Err(Error::InvalidRequest);
                }
                options.trust_roots_only = true;
            }
        }
        let ephemeral = !options.root_certificates.is_empty()
            || options.identity.is_some()
            || options.http2
            || !options.verify_tls
            || !options.trust_env
            || (options.proxy.is_none()
                && [
                    "HTTP_PROXY",
                    "HTTPS_PROXY",
                    "ALL_PROXY",
                    "http_proxy",
                    "https_proxy",
                    "all_proxy",
                ]
                .iter()
                .any(|k| std::env::var(k).is_ok_and(|v| !v.is_empty())));
        let pool = if ephemeral || (!s.pools.contains_key(&options.proxy) && s.pools.len() >= 8) {
            Arc::new(Pool::new(options.clone()))
        } else {
            s.pools
                .entry(options.proxy.clone())
                .or_insert_with(|| Arc::new(Pool::new(options.clone())))
                .clone()
        };
        s.all_pools.retain(|p| p.strong_count() > 0);
        if !s.all_pools.iter().any(|p| p.ptr_eq(&Arc::downgrade(&pool))) {
            s.all_pools.push(Arc::downgrade(&pool));
        }
        Ok(Session {
            decoder: self.decoder.clone(),
            pool,
            injected: s.injected.clone(),
            jar: Jar::default(),
            headers: HeaderMap::new(),
            options,
            deadline,
            cancel,
            stop: self.stop.clone(),
            requests: 0,
        })
    }
    /// Cancels in-flight operations; existing session handles reject further work.
    /// Synchronous drop closes owned idle clients; it never closes injected transports.
    pub fn shutdown(&self) {
        let mut s = self.state.lock().unwrap();
        s.closed = true;
        self.stop.cancel();
        self.decoder.close();
        for p in s.all_pools.drain(..).filter_map(|p| p.upgrade()) {
            p.close();
        }
        s.pools.clear();
    }
    /// Stop transport work and await completion of every owned blocking decoder.
    /// Caller-owned search tasks still belong to the caller's supervisor.
    pub async fn shutdown_and_wait(&self) {
        self.shutdown();
        self.decoder.drain().await;
    }
    pub fn retained_pools(&self) -> usize {
        self.state.lock().unwrap().pools.len()
    }
}
impl Drop for AdapterHttp {
    fn drop(&mut self) {
        self.shutdown();
    }
}
pub struct Session {
    decoder: Arc<Decoder>,
    pool: Arc<Pool>,
    injected: Option<Arc<dyn Transport>>,
    jar: Jar,
    pub headers: HeaderMap,
    options: Options,
    deadline: tokio::time::Instant,
    cancel: Cancellation,
    stop: Cancellation,
    requests: usize,
}
impl Session {
    pub async fn execute(&mut self, mut request: Request) -> Result<Response> {
        if *self.stop.0.borrow() {
            return Err(Error::Closed);
        }
        if *self.cancel.0.borrow() {
            return Err(Error::Cancelled);
        }
        let cancel = self.cancel.clone();
        let stop = self.stop.clone();
        let deadline = self.deadline;
        for (name, value) in &self.headers {
            if !request.headers.contains_key(name) {
                request.headers.insert(name, value.clone());
            }
        }
        tokio::select! { biased;
            _=stop.cancelled()=>Err(Error::Closed),
            _=cancel.cancelled()=>Err(Error::Cancelled),
            _=tokio::time::sleep_until(deadline)=>Err(Error::Deadline),
            result=self.execute_inner(request)=>result,
        }
    }
    async fn execute_inner(&mut self, mut request: Request) -> Result<Response> {
        let mut redirects = 0;
        loop {
            if self.requests >= self.options.bounds.requests {
                return Err(Error::RequestLimit);
            }
            if request.body.len() > self.options.bounds.request_bytes {
                return Err(Error::RequestBodyLimit);
            }
            if !matches!(request.url.scheme(), "http" | "https") {
                return Err(Error::InvalidRequest);
            }
            self.requests += 1;
            let mut outbound = request.clone();
            if !outbound.headers.contains_key(header::COOKIE)
                && let Some(c) = self.jar.cookies(&request.url)
            {
                outbound.headers.insert(header::COOKIE, c);
            }
            let mut response = if let Some(t) = &self.injected {
                t.exchange(outbound).await?
            } else {
                self.network(outbound).await?
            };
            if response.body.len() > self.options.bounds.wire_bytes {
                return Err(Error::WireBodyLimit);
            }
            let mut cookies = response.headers.get_all(header::SET_COOKIE).iter();
            self.jar.set_cookies(&mut cookies, &request.url);
            let headers = response.headers.clone();
            let body = std::mem::take(&mut response.body);
            let bound = self.options.bounds.decoded_bytes;
            response.body = self
                .decoder
                .decode(
                    headers,
                    body,
                    bound,
                    self.cancel.clone(),
                    self.stop.clone(),
                    self.deadline,
                )
                .await?;
            if self.options.follow_redirects
                && matches!(response.status, 301 | 302 | 303 | 307 | 308)
                && let Some(location) = response.headers.get(header::LOCATION)
            {
                if redirects >= self.options.bounds.redirects {
                    return Err(Error::RedirectLimit);
                }
                redirects += 1;
                let next = request
                    .url
                    .join(location.to_str().map_err(|_| Error::InvalidResponse)?)
                    .map_err(|_| Error::InvalidResponse)?;
                if request.url.origin() != next.origin() {
                    request.headers.remove(header::AUTHORIZATION);
                    request.headers.remove(header::PROXY_AUTHORIZATION);
                    request.headers.remove(header::COOKIE);
                    request.headers.remove(header::HOST);
                }
                // HTTPX chooses GET for every non-HEAD 302/303, but only
                // changes the stream/framing if the original method differs.
                request.headers.remove(header::COOKIE);
                if request.method != Method::GET
                    && ((matches!(response.status, 302 | 303) && request.method != Method::HEAD)
                        || (response.status == 301 && request.method == Method::POST))
                {
                    request.method = Method::GET;
                    request.body.clear();
                    for h in [header::CONTENT_LENGTH, header::TRANSFER_ENCODING] {
                        request.headers.remove(h);
                    }
                }
                request.url = next;
                continue;
            }
            return Ok(response);
        }
    }
    async fn network(&self, mut r: Request) -> Result<Response> {
        let _permit = self
            .pool
            .permits
            .acquire()
            .await
            .map_err(|_| Error::Closed)?;
        let checkout = self.pool.checkout(&r.url)?;
        r.headers
            .entry(header::ACCEPT_ENCODING)
            .or_insert(HeaderValue::from_static("gzip, deflate"));
        let mut response = checkout
            .client
            .as_ref()
            .unwrap()
            .request(r.method, r.url)
            .headers(r.headers)
            .body(r.body)
            .send()
            .await
            .map_err(classify)?;
        let status = response.status().as_u16();
        let url = response.url().clone();
        let headers = response.headers().clone();
        let mut body = Vec::new();
        while let Some(chunk) = response.chunk().await.map_err(classify)? {
            if chunk.len() > self.options.bounds.wire_bytes.saturating_sub(body.len()) {
                return Err(Error::WireBodyLimit);
            }
            body.extend_from_slice(&chunk);
        }
        Ok(Response {
            status,
            url,
            headers,
            body,
        })
    }
}
fn classify(e: reqwest::Error) -> Error {
    if e.is_timeout() {
        Error::Deadline
    } else {
        Error::Network
    }
}
const DECODE_CONCURRENCY: usize = 4;
#[derive(Default)]
struct DecodeState {
    closed: bool,
    active: usize,
}
struct Decoder {
    slots: Arc<Semaphore>,
    state: Mutex<DecodeState>,
    done: Notify,
}
impl Default for Decoder {
    fn default() -> Self {
        Self {
            slots: Arc::new(Semaphore::new(DECODE_CONCURRENCY)),
            state: Mutex::new(DecodeState::default()),
            done: Notify::new(),
        }
    }
}
struct DecodeWorker(Arc<Decoder>);
impl Drop for DecodeWorker {
    fn drop(&mut self) {
        let mut state = self.0.state.lock().unwrap();
        state.active -= 1;
        if state.active == 0 {
            self.0.done.notify_waiters();
        }
    }
}
impl Decoder {
    async fn run<T: Send + 'static>(
        self: &Arc<Self>,
        work: impl FnOnce() -> T + Send + 'static,
    ) -> Result<T> {
        let permit = self
            .slots
            .clone()
            .acquire_owned()
            .await
            .map_err(|_| Error::Closed)?;
        {
            let mut state = self.state.lock().unwrap();
            if state.closed {
                return Err(Error::Closed);
            }
            state.active += 1;
        }
        let worker = DecodeWorker(self.clone());
        tokio::task::spawn_blocking(move || {
            let _permit = permit;
            let _worker = worker;
            work()
        })
        .await
        .map_err(|_| Error::InvalidResponse)
    }
    async fn decode(
        self: &Arc<Self>,
        headers: HeaderMap,
        body: Vec<u8>,
        bound: usize,
        cancel: Cancellation,
        stop: Cancellation,
        deadline: tokio::time::Instant,
    ) -> Result<Vec<u8>> {
        self.run(move || decode(&headers, &body, bound, &cancel, &stop, deadline.into_std()))
            .await?
    }
    fn close(&self) {
        self.state.lock().unwrap().closed = true;
        self.slots.close();
    }
    async fn drain(&self) {
        loop {
            let notified = self.done.notified();
            tokio::pin!(notified);
            notified.as_mut().enable();
            if self.state.lock().unwrap().active == 0 {
                return;
            }
            notified.await;
        }
    }
}
fn bounded_read(
    mut reader: impl Read,
    bound: usize,
    cancel: &Cancellation,
    stop: &Cancellation,
    deadline: Instant,
) -> Result<Vec<u8>> {
    let mut result = Vec::new();
    let mut chunk = [0; 64 * 1024];
    loop {
        if *stop.0.borrow() {
            return Err(Error::Closed);
        }
        if *cancel.0.borrow() {
            return Err(Error::Cancelled);
        }
        if Instant::now() >= deadline {
            return Err(Error::Deadline);
        }
        let size = reader
            .read(&mut chunk)
            .map_err(|_| Error::InvalidResponse)?;
        if size == 0 {
            break;
        }
        if size > bound.saturating_sub(result.len()) {
            return Err(Error::DecodedBodyLimit);
        }
        result.extend_from_slice(&chunk[..size]);
    }
    Ok(result)
}
fn decode(
    headers: &HeaderMap,
    body: &[u8],
    bound: usize,
    cancel: &Cancellation,
    stop: &Cancellation,
    deadline: Instant,
) -> Result<Vec<u8>> {
    let mut encodings = Vec::new();
    for value in headers.get_all(header::CONTENT_ENCODING) {
        let value = value.to_str().map_err(|_| Error::InvalidResponse)?;
        encodings.extend(
            value
                .split(',')
                .map(|value| value.trim().to_ascii_lowercase()),
        );
    }
    if encodings.is_empty() {
        encodings.push("identity".into());
    }
    let mut result = body.to_vec();
    for encoding in encodings.into_iter().rev() {
        result = match encoding.as_str() {
            "identity" | "" => bounded_read(result.as_slice(), bound, cancel, stop, deadline)?,
            "gzip" => bounded_read(
                flate2::read::MultiGzDecoder::new(result.as_slice()),
                bound,
                cancel,
                stop,
                deadline,
            )?,
            "deflate" => match bounded_read(
                flate2::read::ZlibDecoder::new(result.as_slice()),
                bound,
                cancel,
                stop,
                deadline,
            ) {
                Err(Error::InvalidResponse) => bounded_read(
                    flate2::read::DeflateDecoder::new(result.as_slice()),
                    bound,
                    cancel,
                    stop,
                    deadline,
                )?,
                other => other?,
            },
            _ => return Err(Error::UnsupportedEncoding),
        };
    }
    Ok(result)
}
/// Offline exchange replay exercises the exact session policy without parsing bodies.
/// Adapters own HTML/XML/JSON parsing and provider-specific status classification.
pub struct Replay {
    exchanges: Mutex<std::collections::VecDeque<(Request, Response)>>,
}
impl Replay {
    pub fn new(exchanges: Vec<(Request, Response)>) -> Self {
        Self {
            exchanges: Mutex::new(exchanges.into()),
        }
    }
    pub fn remaining(&self) -> usize {
        self.exchanges.lock().unwrap().len()
    }
}
impl Transport for Replay {
    fn exchange(&self, r: Request) -> ExchangeFuture<'_> {
        Box::pin(async move {
            let (expected, response) = self
                .exchanges
                .lock()
                .unwrap()
                .pop_front()
                .ok_or(Error::RequestLimit)?;
            if r.method != expected.method
                || r.url != expected.url
                || r.headers != expected.headers
                || r.body != expected.body
            {
                return Err(Error::InvalidRequest);
            }
            Ok(response)
        })
    }
}

/// Provider-directed round-robin routing. Reporting failure never resends a
/// request. Caller supplies monotonic time so cooldowns are deterministic.
pub struct ProxyRotation {
    proxies: Vec<String>,
    dynamic: Option<String>,
    next: usize,
    cooldown: Duration,
    failures: HashMap<String, u32>,
    until: HashMap<String, Instant>,
}
impl ProxyRotation {
    pub fn new(proxies: Vec<String>, dynamic: Option<String>, cooldown: Duration) -> Self {
        Self {
            proxies,
            dynamic,
            next: 0,
            cooldown,
            failures: HashMap::new(),
            until: HashMap::new(),
        }
    }
    pub fn select(&mut self, now: Instant) -> Option<String> {
        if let Some(dynamic) = &self.dynamic
            && !dynamic.is_empty()
        {
            return Some(dynamic.clone());
        }
        if self.proxies.is_empty() {
            return None;
        }
        for _ in 0..self.proxies.len() {
            let proxy = self.proxies[self.next].clone();
            self.next = (self.next + 1) % self.proxies.len();
            if !self.until.get(&proxy).is_some_and(|until| now < *until) {
                return Some(proxy);
            }
        }
        let proxy = self.proxies[self.next].clone();
        self.next = (self.next + 1) % self.proxies.len();
        self.until.remove(&proxy);
        Some(proxy)
    }
    pub fn failure(&mut self, proxy: &str, now: Instant) {
        if !self.proxies.iter().any(|p| p == proxy) {
            return;
        }
        let count = self.failures.entry(proxy.to_owned()).or_default();
        *count = count.saturating_add(1);
        self.until.insert(
            proxy.to_owned(),
            now + self.cooldown.saturating_mul(if *count < 3 { 1 } else { 3 }),
        );
    }
    pub fn success(&mut self, proxy: &str) {
        self.failures.remove(proxy);
        self.until.remove(proxy);
    }
    pub fn available(&self, now: Instant) -> usize {
        self.proxies
            .iter()
            .filter(|p| !self.until.get(*p).is_some_and(|until| now < *until))
            .count()
    }
}

#[cfg(test)]
mod decode_tests {
    use super::*;
    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn decoder_budget_remains_charged_after_waiter_abort_and_drain_joins_workers() {
        let http = Arc::new(AdapterHttp::default());
        let decoder = http.decoder.clone();
        let release = Arc::new((Mutex::new(false), std::sync::Condvar::new()));
        let started = Arc::new(std::sync::atomic::AtomicUsize::new(0));
        let mut tasks = Vec::new();
        for _ in 0..DECODE_CONCURRENCY {
            let decoder = decoder.clone();
            let release = release.clone();
            let started = started.clone();
            tasks.push(tokio::spawn(async move {
                decoder
                    .run(move || {
                        started.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
                        let (lock, cv) = &*release;
                        let mut released = lock.lock().unwrap();
                        while !*released {
                            released = cv.wait(released).unwrap();
                        }
                    })
                    .await
            }));
        }
        tokio::time::timeout(Duration::from_secs(2), async {
            while started.load(std::sync::atomic::Ordering::SeqCst) != DECODE_CONCURRENCY {
                tokio::task::yield_now().await;
            }
        })
        .await
        .unwrap();
        for task in tasks {
            task.abort();
            let _ = task.await;
        }
        assert_eq!(decoder.state.lock().unwrap().active, DECODE_CONCURRENCY);
        assert_eq!(decoder.slots.available_permits(), 0);
        assert!(
            tokio::time::timeout(
                Duration::from_millis(20),
                decoder.run(|| panic!("must not exceed worker bound"))
            )
            .await
            .is_err()
        );
        let owner = http.clone();
        let mut shutdown = tokio::spawn(async move {
            owner.shutdown_and_wait().await;
        });
        assert!(
            tokio::time::timeout(Duration::from_millis(20), &mut shutdown)
                .await
                .is_err()
        );
        {
            let (lock, cv) = &*release;
            *lock.lock().unwrap() = true;
            cv.notify_all();
        }
        tokio::time::timeout(Duration::from_secs(2), shutdown)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(decoder.state.lock().unwrap().active, 0);
        assert_eq!(decoder.run(|| ()).await.unwrap_err(), Error::Closed);
    }
    #[test]
    fn drop_closes_decode_admission_without_claiming_thread_join() {
        let http = AdapterHttp::default();
        let decoder = http.decoder.clone();
        drop(http);
        assert!(decoder.slots.is_closed());
        assert!(decoder.state.lock().unwrap().closed);
    }
    #[test]
    fn decoding_observes_cancel_shutdown_and_expired_budget() {
        let cancel = Cancellation::default();
        let stop = Cancellation::default();
        let deadline = Instant::now() + Duration::from_secs(2);
        cancel.cancel();
        assert_eq!(
            decode(&HeaderMap::new(), b"body", 32, &cancel, &stop, deadline).unwrap_err(),
            Error::Cancelled
        );
        stop.cancel();
        assert_eq!(
            decode(&HeaderMap::new(), b"body", 32, &cancel, &stop, deadline).unwrap_err(),
            Error::Closed
        );
        assert_eq!(
            decode(
                &HeaderMap::new(),
                b"body",
                32,
                &Cancellation::default(),
                &Cancellation::default(),
                Instant::now()
            )
            .unwrap_err(),
            Error::Deadline
        );
    }
    #[test]
    fn decoding_raw_zlib_stacked_and_unknown_encodings() {
        use std::io::Write;
        let body = b"bounded publication body";
        let mut raw =
            flate2::write::DeflateEncoder::new(Vec::new(), flate2::Compression::default());
        raw.write_all(body).unwrap();
        let mut zlib = flate2::write::ZlibEncoder::new(Vec::new(), flate2::Compression::default());
        zlib.write_all(body).unwrap();
        let zlib = zlib.finish().unwrap();
        let mut stacked = flate2::write::GzEncoder::new(Vec::new(), flate2::Compression::default());
        stacked.write_all(&zlib).unwrap();
        for (encoding, encoded) in [
            ("deflate", raw.finish().unwrap()),
            ("deflate", zlib),
            (" DEFLATE, GZIP ", stacked.finish().unwrap()),
        ] {
            let mut headers = HeaderMap::new();
            headers.insert(
                header::CONTENT_ENCODING,
                HeaderValue::from_str(encoding).unwrap(),
            );
            assert_eq!(
                decode(
                    &headers,
                    &encoded,
                    1024,
                    &Cancellation::default(),
                    &Cancellation::default(),
                    Instant::now() + Duration::from_secs(1)
                )
                .unwrap(),
                body
            );
        }
        let mut headers = HeaderMap::new();
        headers.insert(
            header::CONTENT_ENCODING,
            HeaderValue::from_static("unknown"),
        );
        assert_eq!(
            decode(
                &headers,
                body,
                1024,
                &Cancellation::default(),
                &Cancellation::default(),
                Instant::now() + Duration::from_secs(1)
            )
            .unwrap_err(),
            Error::UnsupportedEncoding
        );
    }
}
