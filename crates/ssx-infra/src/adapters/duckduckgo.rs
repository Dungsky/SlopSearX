//! Search-local HTML scrape session, preserving the Python endpoint state machine.
use crate::html::{ElementRef, Html, Selector};
use crate::http::{
    AdapterHttp, Cancellation, Error as HttpError, Options, ProxyRotation, Request, Response,
    Session,
};
use ssx_application::{AdmissionDecision, PortFuture, SearchAdmission, SearchEngine};
use ssx_domain::{AdapterResponse, EngineStatus, FiniteF64, SearchRequest, SearchResult};
use std::{
    sync::{Arc, Mutex},
    time::{Duration, Instant},
};

const UA: &str = "Mozilla/5.0 (X11; Linux x86_64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/131.0.0.0 Safari/537.36";
#[derive(Clone, Debug)]
pub struct ScrapeConfig {
    pub proxy_pool: Vec<String>,
    pub scrape_proxy_url: Option<String>,
    pub proxy_cooloff_seconds: u64,
}
impl Default for ScrapeConfig {
    fn default() -> Self {
        Self {
            proxy_pool: vec![],
            scrape_proxy_url: None,
            proxy_cooloff_seconds: 120,
        }
    }
}
pub(super) struct ScrapeRuntime(Mutex<ProxyRotation>);
impl ScrapeRuntime {
    pub(super) fn new(c: &ScrapeConfig) -> Self {
        Self(Mutex::new(ProxyRotation::new(
            c.proxy_pool.clone(),
            c.scrape_proxy_url.clone(),
            Duration::from_secs(c.proxy_cooloff_seconds),
        )))
    }
    pub(super) fn select(&self) -> Option<String> {
        self.0.lock().unwrap().select(Instant::now())
    }
    pub(super) fn feedback(&self, p: Option<&str>, success: bool) {
        if let Some(p) = p {
            let mut pool = self.0.lock().unwrap();
            if success {
                pool.success(p)
            } else {
                pool.failure(p, Instant::now())
            }
        }
    }
}
#[derive(Clone, Debug)]
pub struct DuckDuckGoConfig {
    pub base_url: String,
    pub fallback_base_url: String,
    pub bootstrap_url: String,
    pub bootstrap_timeout_ms: f64,
    pub timeout_ms: ssx_config::ConfigScalar,
    pub max_results: ssx_json::Value,
    pub allocation_budget_bytes: usize,
    pub scrape: ScrapeConfig,
}
impl Default for DuckDuckGoConfig {
    fn default() -> Self {
        Self {
            base_url: "https://html.duckduckgo.com/html/".into(),
            fallback_base_url: "https://lite.duckduckgo.com/lite/".into(),
            bootstrap_url: "https://duckduckgo.com/".into(),
            bootstrap_timeout_ms: 2500.0,
            timeout_ms: 10_000.into(),
            max_results: ssx_json::json!(10),
            allocation_budget_bytes: crate::html_budget::MAX_SEARCH_BYTES,
            scrape: ScrapeConfig::default(),
        }
    }
}
pub(super) struct DuckDuckGoAdapter {
    http: Arc<AdapterHttp>,
    admission: Arc<dyn SearchAdmission>,
    config: DuckDuckGoConfig,
    runtime: ScrapeRuntime,
}
impl DuckDuckGoAdapter {
    pub(super) fn new(
        http: Arc<AdapterHttp>,
        admission: Arc<dyn SearchAdmission>,
        config: DuckDuckGoConfig,
    ) -> Self {
        let runtime = ScrapeRuntime::new(&config.scrape);
        Self {
            http,
            admission,
            config,
            runtime,
        }
    }
    async fn run(
        &self,
        input: &SearchRequest,
        context: Option<ssx_application::SearchContext>,
    ) -> AdapterResponse {
        let started = Instant::now();
        let done = |out: Outcome| AdapterResponse {
            status: out.status,
            results: out.results,
            error_message: out.error,
            latency_ms: elapsed(started),
            ..AdapterResponse::default()
        };
        match self.admission.admit("duckduckgo").await {
            AdmissionDecision::Allowed => {}
            AdmissionDecision::Denied => return done(Outcome::new(EngineStatus::RateLimited)),
            AdmissionDecision::Unavailable => {
                return done(Outcome::error("rate policy unavailable"));
            }
        }
        let deadline = match super::html_timeout::deadline(&self.config.timeout_ms, context) {
            Ok(deadline) => deadline,
            Err(error) => return done(Outcome::error(error.message())),
        };
        let operation = match crate::html_budget::Operation::start(
            self.config.allocation_budget_bytes,
            input.query.capacity(),
            deadline,
        )
        .await
        {
            Ok(operation) => operation,
            Err(crate::html_budget::OperationError::Deadline) => {
                return done(Outcome::new(EngineStatus::Timeout));
            }
            Err(error) => return done(Outcome::error(error.message())),
        };
        let proxy = self.runtime.select();
        let options = Options {
            proxy: proxy.clone(),
            follow_redirects: true,
            ..Options::default()
        };
        let mut session = match self
            .http
            .session(options, deadline, Cancellation::default())
        {
            Ok(s) => s,
            Err(_) => return done(Outcome::error("HTTP session unavailable")),
        };
        // Source literally clamps milliseconds at 5.0 before dividing by 1000.
        // Preserve this 5ms default; correcting it would require separate review.
        let bootstrap = self.config.bootstrap_timeout_ms.min(5.0) / 1000.0;
        if let Ok(mut request) = Request::get(&self.config.bootstrap_url) {
            request.headers = headers();
            for (key, value) in [
                ("sec-fetch-dest", "document"),
                ("sec-fetch-mode", "navigate"),
                ("sec-fetch-site", "none"),
                ("upgrade-insecure-requests", "1"),
            ] {
                request.headers.insert(key, value.parse().unwrap());
            }
            if let Ok(duration) = Duration::try_from_secs_f64(bootstrap) {
                session.set_deadline(
                    deadline.min(
                        tokio::time::Instant::now()
                            .checked_add(duration)
                            .unwrap_or(deadline),
                    ),
                );
                let _ = session.execute(request).await;
            }
        }
        let image = input
            .categories
            .as_ref()
            .is_some_and(|c| c.iter().any(|c| c == "images"));
        let primary = self
            .endpoint(
                &mut session,
                &operation,
                &self.config.base_url,
                &input.query,
                image,
                false,
                proxy.as_deref(),
            )
            .await;
        if !primary.retriable || image {
            return done(primary);
        }
        let fallback = self
            .endpoint(
                &mut session,
                &operation,
                &self.config.fallback_base_url,
                &input.query,
                false,
                true,
                proxy.as_deref(),
            )
            .await;
        if fallback.status == EngineStatus::Ok && !fallback.results.is_empty() {
            return done(fallback);
        }
        let base = &self.config.base_url;
        let lite = &self.config.fallback_base_url;
        let first = primary.error.as_deref().unwrap_or("unusable response");
        let mut outcome = Outcome::new(fallback.status.clone());
        outcome.error = Some(
            if matches!(
                fallback.status,
                EngineStatus::RateLimited | EngineStatus::Timeout
            ) {
                format!(
                    "{base} unusable ({first}); {lite}: {}",
                    fallback.error.as_deref().unwrap_or("None")
                )
            } else if fallback.status == EngineStatus::Blocked {
                format!(
                    "DuckDuckGo blocked or challenge-walled on all endpoints: {base} ({first}); {lite}: {}",
                    fallback.error.as_deref().unwrap_or("None")
                )
            } else if primary.status == EngineStatus::Blocked && fallback.status == EngineStatus::Ok
            {
                outcome.status = EngineStatus::Blocked;
                format!("{base} blocked/challenge-walled; {lite} served no results for this query")
            } else {
                if outcome.status == EngineStatus::Ok {
                    outcome.status = EngineStatus::Error
                };
                format!(
                    "DuckDuckGo served no parsable results on any endpoint: {base} ({}); {lite} ({})",
                    primary.error.as_deref().unwrap_or("empty"),
                    fallback.error.as_deref().unwrap_or("empty")
                )
            },
        );
        done(outcome)
    }
    async fn endpoint(
        &self,
        session: &mut Session,
        operation: &Arc<crate::html_budget::Operation>,
        url: &str,
        query: &str,
        image: bool,
        lite: bool,
        proxy: Option<&str>,
    ) -> Outcome {
        if tokio::time::Instant::now() >= operation.deadline {
            return Outcome::new(EngineStatus::Timeout);
        }
        let mut request = match Request::get(url) {
            Ok(r) => r,
            Err(_) => return Outcome::retry(format!("{url}: invalid request URL")),
        };
        request.method = reqwest::Method::POST;
        let mut fields = vec![("q", query)];
        if image {
            fields.push(("iar", "images"));
        }
        let size = match python_form_size(&fields) {
            Some(size) => size,
            None => return Outcome::error(crate::html_budget::RESOURCE_ERROR),
        };
        let _request_charge = match operation.budget.charge(size) {
            Ok(charge) => charge,
            Err(error) => return Outcome::error(error),
        };
        request.body = match python_form(&fields) {
            Ok(body) => body.into_bytes(),
            Err(error) => return Outcome::error(&error),
        };
        request.headers = match self.search_headers(url) {
            Ok(h) => h,
            Err(()) => return Outcome::retry(format!("{url}: invalid request headers")),
        };
        request.headers.insert(
            "content-type",
            "application/x-www-form-urlencoded".parse().unwrap(),
        );
        let deadline = operation.deadline;
        session.set_deadline(deadline);
        let resp = match session.execute(request).await {
            Ok(r) => r,
            Err(HttpError::Deadline) => {
                self.runtime.feedback(proxy, false);
                return Outcome::new(EngineStatus::Timeout);
            }
            Err(e) => {
                self.runtime.feedback(proxy, false);
                return Outcome::retry(format!("{url}: {e:?}"));
            }
        };
        if resp.status == 429 {
            self.runtime.feedback(proxy, false);
            return Outcome::new(EngineStatus::RateLimited);
        }
        if [403, 503].contains(&resp.status) {
            self.runtime.feedback(proxy, false);
            let mut out = Outcome::retry(format!("{url} answered {}", resp.status));
            out.status = EngineStatus::Blocked;
            return out;
        }
        if !(200..300).contains(&resp.status) {
            self.runtime.feedback(proxy, false);
            return Outcome::retry(format!("{url}: {}", http_error(&resp)));
        }
        let body = match decode(&resp, deadline, operation.clone()).await {
            Ok(body) => body,
            Err(error) => {
                self.runtime.feedback(proxy, false);
                return Outcome::retry(format!("{url}: {error}"));
            }
        };
        if body.flags.duckduckgo_challenge {
            self.runtime.feedback(proxy, false);
            let mut out = Outcome::retry(format!("{url} served a CAPTCHA/challenge wall"));
            out.status = EngineStatus::Blocked;
            return out;
        }
        let results = match parse(
            &body.text,
            query,
            &self.config.max_results,
            image,
            lite,
            deadline,
            operation.clone(),
        )
        .await
        {
            Ok(r) => r,
            Err(e) => {
                self.runtime.feedback(proxy, false);
                return Outcome::retry(format!("{url}: {e}"));
            }
        };
        if image || !results.is_empty() || body.flags.duckduckgo_legitimate_empty {
            self.runtime.feedback(proxy, true);
            return Outcome {
                status: EngineStatus::Ok,
                results,
                error: None,
                retriable: false,
            };
        }
        Outcome::retry(format!("{url} returned no parsable results"))
    }
    fn search_headers(&self, target: &str) -> Result<reqwest::header::HeaderMap, ()> {
        let target = url::Url::parse(target).map_err(|_| ())?;
        let home = url::Url::parse(&self.config.bootstrap_url).map_err(|_| ())?;
        let host = target.host_str().unwrap_or("");
        let home = home.host_str().unwrap_or("");
        let scheme = target.scheme();
        let site = if host == home {
            "same-origin"
        } else if host.ends_with(&format!(".{home}")) {
            "same-site"
        } else {
            "cross-site"
        };
        let mut h = headers();
        h.insert(
            "accept",
            "text/html,application/xhtml+xml,application/xml;q=0.9,image/avif,image/webp,*/*;q=0.8"
                .parse()
                .unwrap(),
        );
        for (key, value) in [
            ("origin", format!("{scheme}://{home}")),
            ("referer", format!("{scheme}://{home}/")),
            ("sec-fetch-site", site.into()),
            ("sec-fetch-dest", "document".into()),
            ("sec-fetch-mode", "navigate".into()),
            ("upgrade-insecure-requests", "1".into()),
        ] {
            h.insert(key, value.parse().map_err(|_| ())?);
        }
        Ok(h)
    }
}
impl SearchEngine for DuckDuckGoAdapter {
    fn search<'a>(&'a self, r: &'a SearchRequest) -> PortFuture<'a, AdapterResponse> {
        Box::pin(self.run(r, None))
    }
    fn search_with_context<'a>(
        &'a self,
        r: &'a SearchRequest,
        context: ssx_application::SearchContext,
    ) -> PortFuture<'a, AdapterResponse> {
        Box::pin(self.run(r, Some(context)))
    }
}
struct Outcome {
    status: EngineStatus,
    results: Vec<SearchResult>,
    error: Option<String>,
    retriable: bool,
}
impl Outcome {
    fn new(status: EngineStatus) -> Self {
        Self {
            status,
            results: vec![],
            error: None,
            retriable: false,
        }
    }
    fn error(e: &str) -> Self {
        Self {
            error: Some(e.into()),
            ..Self::new(EngineStatus::Error)
        }
    }
    fn retry(e: String) -> Self {
        Self {
            retriable: true,
            ..Self::error(&e)
        }
    }
}
pub(super) fn elapsed(start: Instant) -> FiniteF64 {
    FiniteF64::new(start.elapsed().as_secs_f64() * 1000.0).unwrap_or_default()
}
pub(super) fn headers() -> reqwest::header::HeaderMap {
    let mut h = reqwest::header::HeaderMap::new();
    for (k, v) in [
        ("user-agent", UA),
        (
            "accept",
            "text/html,application/xhtml+xml,application/xml;q=0.9,*/*;q=0.8",
        ),
        ("accept-language", "en-US,en;q=0.9"),
    ] {
        h.insert(k, v.parse().unwrap());
    }
    h
}
pub(super) fn select(s: &str) -> Selector {
    Selector::parse(s).expect("static CSS")
}
pub(super) fn text(node: ElementRef<'_>) -> String {
    let mut text = node.text().collect::<String>();
    let leading = text.len() - text.trim_start_matches(python_whitespace).len();
    let end = text.trim_end_matches(python_whitespace).len();
    if leading >= end {
        text.clear();
    } else {
        text.truncate(end);
        text.drain(..leading);
    }
    text
}
/// The native source tree preserves context-first selector order.
pub(super) fn select_with_context<'a>(
    node: ElementRef<'a>,
    css: &str,
) -> impl Iterator<Item = ElementRef<'a>> {
    node.select_context(css)
}

/// HTTPX/Python urlencode uses quote_plus: unlike WHATWG form encoding, `~`
/// remains literal and `*` is escaped. Preserve exact UTF-8 request bytes.
pub(super) fn python_form_size(fields: &[(&str, &str)]) -> Option<usize> {
    fields
        .iter()
        .enumerate()
        .try_fold(0usize, |total, (index, (key, value))| {
            let total = total.checked_add(usize::from(index != 0))?.checked_add(1)?;
            key.bytes()
                .chain(value.bytes())
                .try_fold(total, |total, byte| {
                    total.checked_add(
                        if byte.is_ascii_alphanumeric()
                            || matches!(byte, b'-' | b'_' | b'.' | b'~' | b' ')
                        {
                            1
                        } else {
                            3
                        },
                    )
                })
        })
}
pub(super) fn python_form(fields: &[(&str, &str)]) -> Result<String, String> {
    const HEX: &[u8; 16] = b"0123456789ABCDEF";
    let size = python_form_size(fields).ok_or("HTML resource budget exceeded")?;
    let mut out = String::new();
    out.try_reserve_exact(size)
        .map_err(|_| "HTML resource budget exceeded")?;
    // The resource-accounted caller checks the exact size before reserving.
    // Emit directly into one buffer rather than component/vector/join copies.
    for (index, (key, value)) in fields.iter().enumerate() {
        if index != 0 {
            out.push('&');
        }
        for (component, value) in [key, value].into_iter().enumerate() {
            if component != 0 {
                out.push('=');
            }
            for byte in value.bytes() {
                match byte {
                    b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                        out.push(char::from(byte))
                    }
                    b' ' => out.push('+'),
                    _ => {
                        out.push('%');
                        out.push(char::from(HEX[(byte >> 4) as usize]));
                        out.push(char::from(HEX[(byte & 15) as usize]));
                    }
                }
            }
        }
    }
    Ok(out)
}

// CPython3.14 str.isspace/strip predicate, including its C0 separators.
// Apply only at the source's text_content().strip() extraction boundaries;
// attributes and internal text remain byte-for-byte untrimmed.
fn python_whitespace(c: char) -> bool {
    matches!(c,'\u{0009}'..='\u{000d}'|'\u{001c}'..='\u{0020}'|'\u{0085}'|'\u{00a0}'|'\u{1680}'|'\u{2000}'..='\u{200a}'|'\u{2028}'|'\u{2029}'|'\u{202f}'|'\u{205f}'|'\u{3000}')
}

pub(super) async fn parse_document(
    raw: &ssx_source_codecs::Text,
    deadline: tokio::time::Instant,
    operation: Arc<crate::html_budget::Operation>,
) -> Result<Html, String> {
    crate::html::parse_with_operation(raw, deadline, operation).await
}

pub(super) async fn decode(
    r: &Response,
    deadline: tokio::time::Instant,
    operation: Arc<crate::html_budget::Operation>,
) -> Result<crate::html::DecodedText, String> {
    let charset = r
        .headers
        .get("content-type")
        .and_then(|v| v.to_str().ok())
        .and_then(|v| {
            v.split(';').find_map(|part| {
                let (k, v) = part.split_once('=')?;
                k.trim()
                    .eq_ignore_ascii_case("charset")
                    .then(|| v.trim().trim_matches(['\'', '"']))
            })
        });
    crate::html::decode_with_operation(&r.body, charset.unwrap_or("utf-8"), deadline, operation)
        .await
}

pub(super) fn http_error(r: &Response) -> String {
    let code = reqwest::StatusCode::from_u16(r.status).unwrap();
    let class = if r.status < 400 {
        "Redirect response"
    } else if r.status < 500 {
        "Client error"
    } else {
        "Server error"
    };
    format!(
        "{class} '{} {}' for url '{}'\nFor more information check: https://developer.mozilla.org/en-US/docs/Web/HTTP/Status/{}",
        r.status,
        code.canonical_reason().unwrap_or(""),
        r.url,
        r.status
    )
}
pub(super) fn challenge(raw: &ssx_source_codecs::Text) -> bool {
    [
        "challenge",
        "verify you're human",
        "hcaptcha",
        "cf-browser-verification",
        "ddg_sl_",
        "data-challenge",
        "bots use duckduckgo",
        "anomaly detected",
    ]
    .iter()
    .any(|s| raw.contains_ascii_lower(s))
}
pub(super) fn legitimate_empty(raw: &ssx_source_codecs::Text) -> bool {
    [
        "class=\"no-results\"",
        "no results</div>",
        "no results for",
        "did not match any documents",
    ]
    .iter()
    .any(|s| raw.contains_ascii_lower(s))
}
fn unwrap_redirect(raw: &str) -> String {
    if !raw.contains("//duckduckgo.com/l/") {
        return raw.into();
    }
    let query = raw
        .split_once('?')
        .map(|(_, q)| q.split('#').next().unwrap_or(q))
        .unwrap_or("");
    url::form_urlencoded::parse(query.as_bytes())
        .find(|(k, v)| k == "uddg" && !v.is_empty())
        .map(|(_, v)| v.into_owned())
        .unwrap_or_else(|| raw.into())
}
async fn parse(
    raw: &ssx_source_codecs::Text,
    query: &str,
    max: &ssx_json::Value,
    image: bool,
    lite: bool,
    deadline: tokio::time::Instant,
    operation: Arc<crate::html_budget::Operation>,
) -> Result<Vec<SearchResult>, String> {
    let doc = parse_document(raw, deadline, operation).await?;
    extract_tree(&doc, query, super::RowLimit::from_value(max), image, lite)
}
pub(super) fn extract_tree(
    doc: &Html,
    query: &str,
    max: super::RowLimit,
    image: bool,
    lite: bool,
) -> Result<Vec<SearchResult>, String> {
    let mut rows = vec![];
    let nodes = if image {
        ".tile--img, .result--image, .tile, .image-result"
    } else if lite {
        "a.result-link"
    } else {
        ".result"
    };
    for (i, node) in doc.select(&select(nodes)).enumerate() {
        if max.reached(rows.len())? {
            break;
        }
        let (url, title, content, img_src) = if image {
            let image = select_with_context(node, "img").next();
            let link = select_with_context(node, "a").next();
            let src = image
                .and_then(|n| {
                    n.value()
                        .attr("src")
                        .filter(|v| !v.is_empty())
                        .or_else(|| n.value().attr("data-src"))
                })
                .unwrap_or("");
            let src = if src.starts_with("//") {
                format!("https:{src}")
            } else {
                src.into()
            };
            let title = image
                .and_then(|n| {
                    n.value()
                        .attr("alt")
                        .filter(|v| !v.is_empty())
                        .or_else(|| n.value().attr("title"))
                })
                .filter(|v| !v.is_empty())
                .map(str::to_owned)
                .or_else(|| link.map(text))
                .filter(|v| !v.is_empty())
                .unwrap_or_else(|| query.into());
            let url = unwrap_redirect(link.and_then(|n| n.value().attr("href")).unwrap_or(""));
            if url.is_empty() {
                continue;
            }
            (
                url,
                title,
                select_with_context(node, ".tile__caption, .result__content, .caption")
                    .next()
                    .map(text)
                    .unwrap_or_default(),
                Some(src),
            )
        } else if lite {
            let url = unwrap_redirect(node.value().attr("href").unwrap_or(""));
            let title = text(node);
            if url.is_empty() || title.is_empty() {
                continue;
            }
            let row = node
                .ancestors()
                .filter_map(ElementRef::wrap)
                .find(|n| n.value().name() == "tr");
            let next = row.and_then(|r| {
                r.next_siblings()
                    .filter_map(ElementRef::wrap)
                    .find(|n| n.value().name() == "tr")
            });
            let content = next
                .and_then(|r| select_with_context(r, "td[class*='result-snippet']").next())
                .map(text)
                .unwrap_or_default();
            (url, title, content, None)
        } else {
            let Some(link) = select_with_context(node, ".result__a").next() else {
                continue;
            };
            let url = select_with_context(link, ".result__url")
                .next()
                .map(text)
                .unwrap_or_else(|| link.value().attr("href").unwrap_or("").into());
            let url = if url.contains("//duckduckgo.com/l/") && url.contains("?uddg=") {
                url.rsplit("?uddg=").next().unwrap().into()
            } else {
                url
            };
            (
                url,
                text(link),
                select_with_context(node, ".result__snippet")
                    .next()
                    .map(text)
                    .unwrap_or_default(),
                None,
            )
        };
        let media = img_src
            .as_ref()
            .filter(|src| !src.is_empty())
            .and_then(|src| {
                ssx_domain::storage::build_media(
                    "image",
                    Some(src),
                    Some(src),
                    Some(&url),
                    None,
                    None,
                    None,
                )
            });
        rows.push(SearchResult {
            url,
            title,
            content,
            engine: "duckduckgo".into(),
            position: if lite {
                (rows.len() + 1) as i64
            } else {
                (i + 1) as i64
            },
            category: if image {
                "images".into()
            } else {
                "general".into()
            },
            img_src: img_src.filter(|s| !s.is_empty()),
            media,
            ..SearchResult::default()
        });
    }
    Ok(rows)
}
