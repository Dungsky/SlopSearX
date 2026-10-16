//! Google HTML adapter. Provider HTML and source order are preserved, including
//! source's permissive `http` prefix and position gaps after skipped nodes.
use super::duckduckgo::{
    ScrapeConfig, ScrapeRuntime, elapsed, headers, http_error, parse_document, python_form, select,
    select_with_context, text,
};
use crate::http::{AdapterHttp, Cancellation, Error as HttpError, Options, Request};
use ssx_application::{AdmissionDecision, PortFuture, SearchAdmission, SearchEngine};
use ssx_domain::{AdapterResponse, EngineStatus, SearchRequest, SearchResult};
use std::{collections::HashSet, sync::Arc, time::Instant};

#[derive(Clone, Debug)]
pub struct GoogleConfig {
    pub base_url: String,
    pub timeout_ms: ssx_config::ConfigScalar,
    pub max_results: ssx_json::Value,
    pub allocation_budget_bytes: usize,
    pub scrape: ScrapeConfig,
}
impl Default for GoogleConfig {
    fn default() -> Self {
        Self {
            base_url: "https://www.google.com/search".into(),
            timeout_ms: 10_000.into(),
            max_results: ssx_json::json!(10),
            allocation_budget_bytes: crate::html_budget::MAX_SEARCH_BYTES,
            scrape: ScrapeConfig::default(),
        }
    }
}
pub(super) struct GoogleAdapter {
    http: Arc<AdapterHttp>,
    admission: Arc<dyn SearchAdmission>,
    config: GoogleConfig,
    runtime: ScrapeRuntime,
}
impl GoogleAdapter {
    pub(super) fn new(
        http: Arc<AdapterHttp>,
        admission: Arc<dyn SearchAdmission>,
        config: GoogleConfig,
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
        let response = |status, results, error_message| AdapterResponse {
            status,
            results,
            error_message,
            latency_ms: elapsed(started),
            ..AdapterResponse::default()
        };
        match self.admission.admit("google").await {
            AdmissionDecision::Allowed => {}
            AdmissionDecision::Denied => return response(EngineStatus::RateLimited, vec![], None),
            AdmissionDecision::Unavailable => {
                return response(
                    EngineStatus::Error,
                    vec![],
                    Some("rate policy unavailable".into()),
                );
            }
        }
        let deadline = match super::html_timeout::deadline(&self.config.timeout_ms, context) {
            Ok(deadline) => deadline,
            Err(error) => {
                return response(EngineStatus::Error, vec![], Some(error.message().into()));
            }
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
                return response(EngineStatus::Timeout, vec![], None);
            }
            Err(error) => {
                return response(EngineStatus::Error, vec![], Some(error.message().into()));
            }
        };
        let proxy = self.runtime.select();
        let mut outbound = match Request::get(&self.config.base_url) {
            Ok(r) => r,
            Err(_) => {
                return response(
                    EngineStatus::Error,
                    vec![],
                    Some("invalid Google base URL".into()),
                );
            }
        };
        let num = ssx_domain::storage::python_value_string(&self.config.max_results);
        let size = match super::duckduckgo::python_form_size(&[
            ("q", &input.query),
            ("hl", "en"),
            ("num", &num),
        ]) {
            Some(size) => size,
            None => {
                return response(
                    EngineStatus::Error,
                    vec![],
                    Some(crate::html_budget::RESOURCE_ERROR.into()),
                );
            }
        };
        let _request_charge = match operation.budget.charge(size) {
            Ok(charge) => charge,
            Err(error) => return response(EngineStatus::Error, vec![], Some(error.into())),
        };
        let encoded = match python_form(&[("q", &input.query), ("hl", "en"), ("num", &num)]) {
            Ok(encoded) => encoded,
            Err(error) => return response(EngineStatus::Error, vec![], Some(error)),
        };
        outbound.url.set_query(Some(&encoded));
        outbound.headers = headers();
        let options = Options {
            proxy: proxy.clone(),
            follow_redirects: true,
            ..Options::default()
        };
        let deadline = operation.deadline;
        let mut session = match self
            .http
            .session(options, deadline, Cancellation::default())
        {
            Ok(s) => s,
            Err(_) => {
                return response(
                    EngineStatus::Error,
                    vec![],
                    Some("HTTP session unavailable".into()),
                );
            }
        };
        let resp = match session.execute(outbound).await {
            Ok(r) => r,
            Err(HttpError::Deadline) => return response(EngineStatus::Timeout, vec![], None),
            Err(e) => return response(EngineStatus::Error, vec![], Some(format!("{e:?}"))),
        };
        if [429, 403, 503].contains(&resp.status) {
            self.runtime.feedback(proxy.as_deref(), false);
            return response(
                if resp.status == 429 {
                    EngineStatus::RateLimited
                } else {
                    EngineStatus::Blocked
                },
                vec![],
                None,
            );
        }
        if !(200..300).contains(&resp.status) {
            return response(EngineStatus::Error, vec![], Some(http_error(&resp)));
        }
        let body = match super::duckduckgo::decode(&resp, deadline, operation.clone()).await {
            Ok(body) => body,
            Err(error) => return response(EngineStatus::Error, vec![], Some(error)),
        };
        if body.flags.google_challenge {
            self.runtime.feedback(proxy.as_deref(), false);
            return response(EngineStatus::Blocked, vec![], None);
        }
        self.runtime.feedback(proxy.as_deref(), true);
        match parse(
            &body.text,
            &self.config.max_results,
            deadline,
            operation.clone(),
        )
        .await
        {
            Ok(rows) => response(EngineStatus::Ok, rows, None),
            Err(e) => response(EngineStatus::Error, vec![], Some(e)),
        }
    }
}
impl SearchEngine for GoogleAdapter {
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
pub(super) fn challenge(raw: &ssx_source_codecs::Text) -> bool {
    [
        "challenge",
        "recaptcha",
        "cf-browser-verification",
        "unusual traffic",
        "verify you're not a robot",
        "g-recaptcha",
    ]
    .iter()
    .any(|s| raw.contains_ascii_lower(s))
}
async fn parse(
    raw: &ssx_source_codecs::Text,
    max: &ssx_json::Value,
    deadline: tokio::time::Instant,
    operation: Arc<crate::html_budget::Operation>,
) -> Result<Vec<SearchResult>, String> {
    let doc = parse_document(raw, deadline, operation).await?;
    extract_tree(&doc, super::RowLimit::from_value(max))
}
pub(super) fn extract_tree(
    doc: &crate::html::Html,
    max: super::RowLimit,
) -> Result<Vec<SearchResult>, String> {
    let mut rows = vec![];
    let mut seen = HashSet::new();
    for (i, node) in doc.select(&select("div.g")).enumerate() {
        if max.reached(rows.len())? {
            break;
        }
        let Some(link) = select_with_context(node, "a[href^='http']").next() else {
            continue;
        };
        let url = link.value().attr("href").unwrap_or("");
        if url.is_empty() || !seen.insert(url.to_owned()) {
            continue;
        }
        rows.push(SearchResult {
            url: url.into(),
            title: select_with_context(node, "h3")
                .next()
                .map(text)
                .unwrap_or_default(),
            content: select_with_context(node, "span.aCOpRe, div.VwiC3b, span.st")
                .next()
                .map(text)
                .unwrap_or_default(),
            engine: "google".into(),
            position: (i + 1) as i64,
            ..SearchResult::default()
        });
    }
    Ok(rows)
}
