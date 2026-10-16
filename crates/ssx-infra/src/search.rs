//! Native shared search execution and task ownership.
use ssx_application::{
    Audit, Metrics, PortFuture, SearchEngine, Supervisor, TaskEvent, TaskState,
    cache::{
        CacheAdmission, CacheEntry, CacheKeyInput, CanonicalCache, CanonicalResponse,
        RoutingIdentity, composed_key,
    },
    health::{HealthState, OutcomeOrigin},
    policy::{PolicyError, PolicyService, ScopePreview, ScopeProposal, ValidatedScope},
    ranking::{Feeds, rank, ranking_explanation},
    scholarly::group_publications,
};
use ssx_domain::{
    AdapterResponse, EngineOutcome, EngineStatus, ScopeDecision, SearchRequest, SearchResponse,
    WallClockTimestamp,
};
use std::{
    collections::{BTreeMap, HashMap},
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, Ordering},
    },
    time::{Duration, SystemTime, UNIX_EPOCH},
};
use tokio::{
    sync::{Notify, OwnedSemaphorePermit, Semaphore, watch},
    task::{JoinHandle, JoinSet},
    time::Instant,
};

/// Logical generation must change whenever an adapter is replaced or reconfigured.
/// It is supplied by the composition root, never a process address.
#[derive(Clone)]
pub struct EngineBinding {
    pub engine: Arc<dyn SearchEngine>,
    pub generation: String,
    pub timeout: Duration,
    pub scrape: bool,
}
pub trait ClientAdmission: Send + Sync {
    fn acquire<'a>(&'a self, client: &'a str) -> PortFuture<'a, bool>;
}
pub trait Suggestions: Send + Sync {
    fn fetch<'a>(&'a self, query: &'a str) -> PortFuture<'a, Vec<String>>;
    fn shutdown(&self) -> PortFuture<'_, ()>;
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RerankState {
    Skipped,
    Applied,
    Fallback,
}
pub struct RerankDecision {
    pub results: Vec<ssx_domain::SearchResult>,
    pub state: RerankState,
}
/// F15 implements this optional stage. Absence executes the complete baseline
/// ranking path; a classified fallback must never populate canonical cache.
pub trait RerankStage: Send + Sync {
    fn rerank<'a>(
        &'a self,
        request: &'a SearchRequest,
        results: &'a [ssx_domain::SearchResult],
        deadline: Instant,
    ) -> PortFuture<'a, Result<RerankDecision, ()>>;
}
pub trait SearchClock: Send + Sync {
    /// One captured instant supplies both epoch and process-local calendar date.
    fn capture(&self) -> (WallClockTimestamp, String);
}
pub struct SystemSearchClock;
impl SearchClock for SystemSearchClock {
    fn capture(&self) -> (WallClockTimestamp, String) {
        Self::at(SystemTime::now())
    }
}
impl SystemSearchClock {
    pub fn at(now: SystemTime) -> (WallClockTimestamp, String) {
        let local: chrono::DateTime<chrono::Local> = now.into();
        let seconds = now
            .duration_since(UNIX_EPOCH)
            .map_or_else(|e| -e.duration().as_secs_f64(), |v| v.as_secs_f64());
        (
            WallClockTimestamp::new(seconds).expect("finite system clock"),
            local.format("%Y-%m-%d").to_string(),
        )
    }
}
#[derive(Debug, Clone, PartialEq)]
pub enum SearchError {
    Policy(PolicyError),
    ClientLimited,
    Busy,
    Closed,
    Cancelled,
    InvalidConfiguration,
    Unavailable(SearchUnavailable),
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum UnavailableReason {
    NoEligibleEngines,
    NativeImplementationUnavailable,
}
#[derive(Debug, Clone, PartialEq)]
pub struct SearchUnavailable {
    /// Policy-checked explanation only. This type grants no dispatch capability.
    pub preview: Box<ScopePreview>,
    pub reason: UnavailableReason,
}
#[derive(Clone)]
pub struct SearchConfig {
    pub max_callers: usize,
    pub max_flights: usize,
    pub max_parallel_engines: usize,
    pub max_selected_engines: usize,
    pub ranking_strategy: String,
    pub empty_scrape_diagnostics: bool,
    /// Backstop for injected optional ports; native implementations also carry
    /// their accepted per-operation bounds. This is separate from dispatch time.
    pub optional_port_timeout: Duration,
}
impl Default for SearchConfig {
    fn default() -> Self {
        Self {
            max_callers: 256,
            max_flights: 64,
            max_parallel_engines: 10,
            max_selected_engines: 128,
            ranking_strategy: "presence".into(),
            empty_scrape_diagnostics: false,
            optional_port_timeout: Duration::from_secs(30),
        }
    }
}
pub struct SearchSettings {
    pub execution: SearchConfig,
    pub client_quota: ssx_domain::rate::QuotaConfig,
    pub client_fail_closed: bool,
    pub brave_category_routing: bool,
}
impl SearchSettings {
    /// Parse a captured environment, never ambient process state. Invalid source
    /// text falls back; native F10 rejects nonfinite/unsafe quota arithmetic.
    pub fn from_environment(
        env: &BTreeMap<String, String>,
        ranking_strategy: &str,
    ) -> Result<Self, SearchError> {
        let concurrency = env
            .get("MAX_CONCURRENT_ENGINES")
            .map_or_else(|| "10".into(), |s| s.clone());
        let normalized =
            ssx_domain::storage::normalize_python_number_text(&concurrency).unwrap_or_default();
        let digits = normalized.strip_prefix(['+', '-']).unwrap_or(&normalized);
        let max_parallel_engines =
            if !digits.is_empty() && digits.bytes().all(|b| b.is_ascii_digit()) {
                if normalized.starts_with('-') {
                    1
                } else {
                    normalized
                        .trim_start_matches('+')
                        .parse::<usize>()
                        .map_err(|_| SearchError::InvalidConfiguration)?
                        .max(1)
                }
            } else {
                10
            };
        let float = |name: &str, default: f64| {
            env.get(name)
                .and_then(|v| ssx_domain::storage::normalize_python_number_text(v).ok())
                .and_then(|v| v.parse::<f64>().ok())
                .unwrap_or(default)
        };
        let client_quota = ssx_domain::rate::QuotaConfig::new(
            float("PER_CLIENT_REQUESTS", 30.0),
            float("PER_CLIENT_WINDOW_SECONDS", 60.0),
        )
        .map_err(|_| SearchError::InvalidConfiguration)?;
        if max_parallel_engines > Semaphore::MAX_PERMITS {
            return Err(SearchError::InvalidConfiguration);
        }
        let raw_flag = |name: &str| {
            env.get(name)
                .map(|v| v.to_lowercase())
                .is_some_and(|v| matches!(v.as_str(), "true" | "1"))
        };
        Ok(Self {
            execution: SearchConfig {
                max_parallel_engines,
                ranking_strategy: ranking_strategy.into(),
                empty_scrape_diagnostics: raw_flag("FEATURE_EMPTY_SCRAPE_DIAGNOSTICS"),
                ..Default::default()
            },
            client_quota,
            client_fail_closed: env.get("FAIL_CLOSED").is_some_and(|v| {
                matches!(
                    v.trim_matches(ssx_json::python_whitespace)
                        .to_lowercase()
                        .as_str(),
                    "true" | "1" | "yes"
                )
            }),
            brave_category_routing: raw_flag("FEATURE_BRAVE_CATEGORY_ROUTING"),
        })
    }
}
pub struct SearchDependencies {
    pub policy: PolicyService,
    pub engines: BTreeMap<String, EngineBinding>,
    pub routing_identity: RoutingIdentity,
    pub cache: Option<Arc<CanonicalCache>>,
    pub client_admission: Option<Arc<dyn ClientAdmission>>,
    pub suggestions: Option<Arc<dyn Suggestions>>,
    pub jev: Option<Arc<crate::jev::JevRouter>>,
    pub routing_cache: Option<crate::valkey::Valkey>,
    pub metrics: Arc<dyn Metrics>,
    pub supervisor: Arc<dyn Supervisor>,
    pub audit: Arc<dyn Audit>,
    /// Cooldown-only lookup; adapter-owned F10 admission consumes engine quota.
    pub cooldowns: Option<Arc<dyn ssx_application::rate::RateStore>>,
    /// Explicitly owned transports/stores, closed only after all flights join.
    pub owned_http: Vec<Arc<crate::http::AdapterHttp>>,
    pub owned_stores: Vec<Arc<crate::valkey::Valkey>>,
    pub clock: Arc<dyn SearchClock>,
    pub reranker: Option<Arc<dyn RerankStage>>,
}
struct Flight {
    waiters: Mutex<usize>,
    cancel: watch::Sender<bool>,
    result: watch::Sender<Option<Result<CanonicalResponse, SearchError>>>,
    task: Mutex<Option<JoinHandle<()>>>,
    done: watch::Sender<bool>,
}
struct Completion(watch::Sender<bool>);
impl Drop for Completion {
    fn drop(&mut self) {
        self.0.send_replace(true);
    }
}
#[derive(Clone)]
pub struct SearchCancellation(watch::Sender<bool>);
impl Default for SearchCancellation {
    fn default() -> Self {
        Self(watch::channel(false).0)
    }
}
impl SearchCancellation {
    pub fn cancel(&self) {
        self.0.send_replace(true);
    }
}
struct Waiter(Arc<Flight>);
impl Drop for Waiter {
    fn drop(&mut self) {
        let mut waiters = self.0.waiters.lock().unwrap();
        *waiters -= 1;
        if *waiters == 0 {
            self.0.cancel.send_replace(true);
        }
    }
}
struct Inner {
    config: SearchConfig,
    deps: SearchDependencies,
    callers: Arc<Semaphore>,
    callers_done: Arc<Notify>,
    stop: watch::Sender<bool>,
    engines: Arc<Semaphore>,
    flights: Mutex<HashMap<String, Arc<Flight>>>,
    health: Mutex<BTreeMap<String, HealthState>>,
    closed: AtomicBool,
}
#[derive(Clone)]
pub struct SearchService(Arc<Inner>);
struct CallerGuard {
    permit: Option<OwnedSemaphorePermit>,
    done: Arc<Notify>,
}
impl Drop for CallerGuard {
    fn drop(&mut self) {
        drop(self.permit.take());
        self.done.notify_waiters();
    }
}
pub struct CapturedSearch {
    pub canonical: CanonicalResponse,
    pub response: SearchResponse,
    /// Optional persistence failure never discards the fresh complete evidence.
    pub snapshot: Result<ssx_domain::SnapshotId, ssx_application::snapshot::SnapshotFailure>,
}
impl SearchService {
    pub fn new(config: SearchConfig, deps: SearchDependencies) -> Result<Self, SearchError> {
        if config.max_callers == 0
            || config.max_flights == 0
            || config.max_parallel_engines == 0
            || config.max_selected_engines == 0
            || config.max_callers > Semaphore::MAX_PERMITS
            || config.max_parallel_engines > Semaphore::MAX_PERMITS
            || config.optional_port_timeout.is_zero()
            || Instant::now()
                .checked_add(config.optional_port_timeout)
                .is_none()
            || deps.engines.values().any(|e| {
                e.generation.is_empty()
                    || e.timeout.is_zero()
                    || Instant::now().checked_add(e.timeout).is_none()
            })
        {
            return Err(SearchError::InvalidConfiguration);
        }
        Ok(Self(Arc::new(Inner {
            callers: Arc::new(Semaphore::new(config.max_callers)),
            callers_done: Arc::new(Notify::new()),
            stop: watch::channel(false).0,
            engines: Arc::new(Semaphore::new(config.max_parallel_engines)),
            config,
            deps,
            flights: Mutex::new(HashMap::new()),
            health: Mutex::new(BTreeMap::new()),
            closed: AtomicBool::new(false),
        })))
    }
    pub fn health(&self) -> BTreeMap<String, HealthState> {
        self.0.health.lock().unwrap().clone()
    }
    fn current_policy(&self, now: WallClockTimestamp) -> Result<PolicyService, SearchError> {
        let mut policy = self.0.deps.policy.clone();
        let mut capabilities = policy.catalog.engines().to_vec();
        for capability in &mut capabilities {
            if let Some(health) = self.0.health.lock().unwrap().get(&capability.name) {
                capability.circuit_open |= health.circuit_open(now);
                if let Ok(value) =
                    ssx_application::health::build_engine_health(Some(health), None, now, 300.0)
                {
                    capability.health_status = value["status"].as_str().unwrap_or("unknown").into();
                    capability.health_stale = value["stale"].as_bool().unwrap_or(true);
                }
            }
        }
        policy.catalog = ssx_application::scope::CatalogSnapshot::new(capabilities)
            .map_err(|_| SearchError::InvalidConfiguration)?;
        Ok(policy)
    }
    /// Reuse this opaque authority after a transport's enforcement preview; the
    /// dispatch path revalidates it but never repeats optional routing disclosure.
    pub async fn prepare_scope(
        &self,
        proposal: &ScopeProposal,
    ) -> Result<ValidatedScope, SearchError> {
        if self.0.closed.load(Ordering::Acquire) {
            return Err(SearchError::Closed);
        }
        let _caller = CallerGuard {
            permit: Some(
                self.0
                    .callers
                    .clone()
                    .try_acquire_owned()
                    .map_err(|_| SearchError::Busy)?,
            ),
            done: self.0.callers_done.clone(),
        };
        let policy = self.current_policy(self.0.deps.clock.capture().0)?;
        let cancellation = SearchCancellation::default();
        self.authorize_proposal(proposal, &policy, &mut cancellation.0.subscribe())
            .await
    }
    async fn authorize_proposal(
        &self,
        proposal: &ScopeProposal,
        policy: &PolicyService,
        cancel: &mut watch::Receiver<bool>,
    ) -> Result<ValidatedScope, SearchError> {
        let advice = crate::jev::prepare_optional(
            self.0.deps.jev.as_deref(),
            proposal,
            policy,
            self.0.deps.routing_cache.as_ref(),
        );
        let mut stop = self.0.stop.subscribe();
        tokio::select! {
            biased;
            _ = cancelled(&mut stop) => Err(SearchError::Closed),
            _ = cancelled(cancel) => Err(SearchError::Cancelled),
            prepared = tokio::time::timeout(self.0.config.optional_port_timeout, advice) => match prepared {
                Ok(result) => { let prepared = result.map_err(SearchError::Policy)?; classify_availability(policy, &prepared.preview)?; prepared.authorize(policy).map_err(SearchError::Policy) },
                Err(_) => { let preview = policy.preview(proposal).map_err(SearchError::Policy)?; classify_availability(policy, &preview)?; policy.authorize(proposal.clone()).map_err(SearchError::Policy) },
            }
        }
    }
    pub async fn search_prepared(
        &self,
        authority: ValidatedScope,
        cancellation: SearchCancellation,
    ) -> Result<SearchResponse, SearchError> {
        self.search_inner(None, Some(authority), cancellation).await
    }
    pub async fn search_snapshot(
        &self,
        mut proposal: ScopeProposal,
        snapshots: &ssx_application::snapshot::SnapshotService,
        parents: &[ssx_domain::ArtifactRef],
    ) -> Result<CapturedSearch, SearchError> {
        let include = proposal.request.include.clone();
        let max = proposal.request.max_results;
        proposal.request.include = ["results", "suggestions", "engine_status", "diagnostics"]
            .into_iter()
            .map(str::to_owned)
            .collect();
        proposal.request.max_results = None;
        let full = self.search(proposal).await?;
        let cached = full.cached;
        let canonical = CanonicalResponse::new(full);
        let snapshot = if let Ok(permit) = self.0.callers.clone().try_acquire_owned() {
            let _caller = CallerGuard {
                permit: Some(permit),
                done: self.0.callers_done.clone(),
            };
            let mut stop = self.0.stop.subscribe();
            tokio::select! {
                biased;
                _ = cancelled(&mut stop) => Err(ssx_application::snapshot::SnapshotFailure::CompletionUnknown),
                result = tokio::time::timeout(self.0.config.optional_port_timeout, snapshots.create(&canonical, parents)) => result.unwrap_or(Err(ssx_application::snapshot::SnapshotFailure::CompletionUnknown)),
            }
        } else {
            Err(ssx_application::snapshot::SnapshotFailure::CompletionUnknown)
        };
        let response = canonical
            .view(include, max.and_then(|v| i64::try_from(v).ok()), cached)
            .to_response();
        Ok(CapturedSearch {
            canonical,
            response,
            snapshot,
        })
    }
    pub async fn search(&self, proposal: ScopeProposal) -> Result<SearchResponse, SearchError> {
        self.search_with_cancellation(proposal, SearchCancellation::default())
            .await
    }
    /// Explicit caller cancellation joins work when this was the final waiter.
    /// Dropping a Rust future signals cancellation via RAII; shutdown also joins.
    pub async fn search_with_cancellation(
        &self,
        proposal: ScopeProposal,
        cancellation: SearchCancellation,
    ) -> Result<SearchResponse, SearchError> {
        self.search_inner(Some(proposal), None, cancellation).await
    }
    async fn search_inner(
        &self,
        proposal: Option<ScopeProposal>,
        prepared: Option<ValidatedScope>,
        cancellation: SearchCancellation,
    ) -> Result<SearchResponse, SearchError> {
        let start = Instant::now();
        if *cancellation.0.borrow() {
            return Err(SearchError::Cancelled);
        }
        if self.0.closed.load(Ordering::Acquire) {
            return Err(SearchError::Closed);
        }
        // No task is created for excess callers; queued futures have a finite bound.
        let _caller = CallerGuard {
            permit: Some(
                self.0
                    .callers
                    .clone()
                    .try_acquire_owned()
                    .map_err(|_| SearchError::Busy)?,
            ),
            done: self.0.callers_done.clone(),
        };
        let (now, calendar_date) = self.0.deps.clock.capture();
        if ssx_application::filters::publication_day(&calendar_date).is_none() {
            return Err(SearchError::InvalidConfiguration);
        }
        let policy = self.current_policy(now)?;
        let mut preflight_cancel = cancellation.0.subscribe();
        let mut stop = self.0.stop.subscribe();
        let authority = match prepared {
            Some(authority) => authority,
            None => {
                self.authorize_proposal(
                    proposal.as_ref().ok_or(SearchError::InvalidConfiguration)?,
                    &policy,
                    &mut preflight_cancel,
                )
                .await?
            }
        };
        let preview = policy
            .revalidate_preview(&authority)
            .map_err(SearchError::Policy)?;
        classify_availability(&policy, &preview)?;
        let (mut request, scope) = policy
            .with_revalidated(&authority, |command| {
                (command.request().clone(), command.preview().scope.clone())
            })
            .map_err(SearchError::Policy)?;
        // The trusted calendar anchor is resolved once, independent of caller input.
        request.time_range_anchor = request
            .time_range
            .as_deref()
            .and_then(ssx_application::filters::time_range_days)
            .map(|_| calendar_date);
        if scope.selected_engines.len() > self.0.config.max_selected_engines
            || scope
                .selected_engines
                .iter()
                .any(|n| !self.0.deps.engines.contains_key(n))
        {
            return Err(SearchError::InvalidConfiguration);
        }
        if let (Some(admission), Some(client)) =
            (&self.0.deps.client_admission, &request.client_identifier)
            && !client.is_empty()
        {
            let allowed = tokio::select! { biased; _ = cancelled(&mut stop) => return Err(SearchError::Closed), _ = cancelled(&mut preflight_cancel) => return Err(SearchError::Cancelled), allowed = tokio::time::timeout(self.0.config.optional_port_timeout, admission.acquire(client)) => allowed.unwrap_or(false) };
            if !allowed {
                return Err(SearchError::ClientLimited);
            }
        }
        let mut random = [0; 6];
        aws_lc_rs::rand::fill(&mut random).map_err(|_| SearchError::InvalidConfiguration)?;
        let query_id: String = random.iter().map(|b| format!("{b:02x}")).collect();
        let key = cache_key(&self.0, &request, &policy, &scope);
        let fresh = request.freshness == "prefer_fresh";
        let expected_date_proof = publication_date_proof(&request, &scope)?;
        let cache_categories = request_categories(&request).to_vec();
        if let Some(cache) = &self.0.deps.cache {
            let entry = tokio::select! { biased; _ = cancelled(&mut stop) => return Err(SearchError::Closed), _ = cancelled(&mut preflight_cancel) => return Err(SearchError::Cancelled), entry = tokio::time::timeout(self.0.config.optional_port_timeout, cache.read(&key, fresh)) => entry.ok().flatten() };
            let entry = entry.filter(|entry| {
                expected_date_proof.is_none()
                    || match entry {
                        CacheEntry::Canonical(canonical) => {
                            canonical.response().date_filter == expected_date_proof
                                && canonical.response().scope.selected_engines
                                    == scope.selected_engines
                        }
                        CacheEntry::Negative { .. } => false,
                    }
            });
            if let Some(entry) = entry {
                let context = ssx_application::cache::CachedViewContext {
                    query: request.query.clone(),
                    query_id: ssx_domain::QueryId::new(query_id.clone())
                        .map_err(|_| SearchError::InvalidConfiguration)?,
                    ranking_explanation: ranking_explanation(&self.0.config.ranking_strategy)
                        .into(),
                    include: request.include.clone(),
                    max_results: request.max_results.and_then(|v| i64::try_from(v).ok()),
                };
                let mut response = entry.view(context).to_response();
                personalize(&mut response, &request, scope, query_id, start);
                return Ok(response);
            }
        }
        let identities: Vec<_> = scope
            .selected_engines
            .iter()
            .map(|n| (n, &self.0.deps.engines[n].generation))
            .collect();
        let flight_key = ssx_json::to_string(&(
            key.as_str(),
            identities,
            &scope.jev_scores,
            &request.execution_isolation_key,
            fresh.then_some(&query_id),
        ))
        .expect("closed identity primitives");
        let flight = {
            let mut flights = self.0.flights.lock().unwrap();
            flights.retain(|_, f| {
                *f.waiters.lock().unwrap() != 0
                    || !f
                        .task
                        .lock()
                        .unwrap()
                        .as_ref()
                        .is_some_and(JoinHandle::is_finished)
            });
            if self.0.closed.load(Ordering::Acquire) {
                return Err(SearchError::Closed);
            }
            if let Some(flight) = flights.get(&flight_key) {
                let mut waiters = flight.waiters.lock().unwrap();
                if *flight.cancel.borrow() {
                    return Err(SearchError::Busy);
                }
                *waiters += 1;
                drop(waiters);
                flight.clone()
            } else {
                if flights.len() >= self.0.config.max_flights {
                    return Err(SearchError::Busy);
                }
                let flight = Arc::new(Flight {
                    waiters: Mutex::new(1),
                    cancel: watch::channel(false).0,
                    result: watch::channel(None).0,
                    task: Mutex::new(None),
                    done: watch::channel(false).0,
                });
                let owner = self.0.clone();
                let worker = flight.clone();
                let req = request.clone();
                let selected = scope.clone();
                let task = tokio::spawn(async move {
                    let _completion = Completion(worker.done.clone());
                    owner.deps.supervisor.observed(TaskEvent {
                        task: "search_flight",
                        state: TaskState::Started,
                    });
                    let result = execute(
                        &owner,
                        req,
                        selected,
                        Instant::now(),
                        worker.cancel.subscribe(),
                    )
                    .await;
                    if let Ok((canonical, admission)) = &result
                        && let Some(cache) = &owner.deps.cache
                    {
                        let write = cache.write(&key, canonical, &cache_categories, *admission);
                        // Optional persistence cannot outlive cancellation indefinitely.
                        let mut cancel = worker.cancel.subscribe();
                        tokio::select! { _ = tokio::time::timeout(owner.config.optional_port_timeout, write) => {}, _ = cancelled(&mut cancel) => {} }
                    }
                    owner.deps.supervisor.observed(TaskEvent {
                        task: "search_flight",
                        state: if matches!(result, Err(SearchError::Cancelled)) {
                            TaskState::Cancelled
                        } else if result.is_ok() {
                            TaskState::Completed
                        } else {
                            TaskState::Failed
                        },
                    });
                    worker
                        .result
                        .send_replace(Some(result.map(|(canonical, _)| canonical)));
                });
                *flight.task.lock().unwrap() = Some(task);
                flights.insert(flight_key, flight.clone());
                flight
            }
        };
        let waiter = Waiter(flight.clone());
        let mut rx = flight.result.subscribe();
        let mut done = flight.done.subscribe();
        let mut caller_cancel = cancellation.0.subscribe();
        let canonical = loop {
            if let Some(result) = rx.borrow_and_update().clone() {
                break result?;
            }
            tokio::select! {
                _ = cancelled(&mut caller_cancel) => {
                    drop(waiter);
                    if *flight.cancel.borrow() { cancelled(&mut done).await; }
                    return Err(SearchError::Cancelled);
                }
                _ = cancelled(&mut done) => { if rx.borrow().is_none() { return Err(SearchError::Cancelled); } }
                result = rx.changed() => { result.map_err(|_| SearchError::Cancelled)?; }
            }
        };
        let mut response = canonical
            .view(
                request.include.clone(),
                request.max_results.and_then(|v| i64::try_from(v).ok()),
                false,
            )
            .to_response();
        personalize(&mut response, &request, scope, query_id, start);
        self.0.deps.audit.record("search_query");
        drop(waiter);
        Ok(response)
    }
    /// Cancels all owned flights and joins their engine/suggestion descendants.
    /// Concurrent/repeated shutdown is safe; transport owners close after joins.
    pub async fn shutdown(&self) {
        self.0.closed.store(true, Ordering::Release);
        self.0.callers.close();
        self.0.stop.send_replace(true);
        let flights: Vec<_> = self.0.flights.lock().unwrap().values().cloned().collect();
        for flight in &flights {
            flight.cancel.send_replace(true);
        }
        for flight in flights {
            let task = flight.task.lock().unwrap().take();
            if let Some(task) = task {
                let _ = task.await;
            } else {
                cancelled(&mut flight.done.subscribe()).await;
            }
        }
        loop {
            let settled = self.0.callers_done.notified();
            if self.0.callers.available_permits() == self.0.config.max_callers {
                break;
            }
            settled.await;
        }
        if let Some(s) = &self.0.deps.suggestions {
            s.shutdown().await;
        }
        if let Some(j) = &self.0.deps.jev {
            j.shutdown().await;
        }
        for http in &self.0.deps.owned_http {
            http.shutdown_and_wait().await;
        }
        for store in &self.0.deps.owned_stores {
            store.close().await;
        }
    }
}
fn request_categories(request: &SearchRequest) -> &[String] {
    request.categories.as_deref().unwrap_or(&[])
}
fn classify_availability(
    policy: &PolicyService,
    preview: &ScopePreview,
) -> Result<(), SearchError> {
    let reason = if preview.scope.selected_engines.is_empty() {
        Some(UnavailableReason::NoEligibleEngines)
    } else if preview.scope.selected_engines.iter().any(|name| {
        policy
            .catalog
            .get(name)
            .is_some_and(|engine| !engine.available)
    }) {
        Some(UnavailableReason::NativeImplementationUnavailable)
    } else {
        None
    };
    match reason {
        Some(reason) => Err(SearchError::Unavailable(SearchUnavailable {
            preview: Box::new(preview.clone()),
            reason,
        })),
        None => Ok(()),
    }
}
fn cache_key(
    inner: &Inner,
    request: &SearchRequest,
    policy: &PolicyService,
    scope: &ScopeDecision,
) -> ssx_application::cache::SearchCacheKey {
    let window = request
        .time_range
        .as_deref()
        .and_then(ssx_application::filters::time_range_days)
        .and_then(|days| {
            request
                .time_range_anchor
                .as_deref()
                .and_then(ssx_application::filters::publication_day)
                .and_then(|day| ssx_application::iso_timestamp((day - days) as f64 * 86400.0).ok())
                .map(|v| {
                    (
                        v[..10].to_owned(),
                        request.time_range_anchor.clone().unwrap(),
                    )
                })
        });
    let mut routing = inner.deps.routing_identity.clone();
    let generations: Vec<_> = inner
        .deps
        .engines
        .iter()
        .map(|(name, engine)| (name, &engine.generation))
        .collect();
    let with_health = policy
        .resolver
        .budget
        .as_ref()
        .is_some_and(|b| b.max_engines > 0);
    let eligibility: Vec<_> = policy
        .catalog
        .engines()
        .iter()
        .map(|e| {
            (
                &e.name,
                e.active,
                e.enabled,
                e.auth_ready,
                e.circuit_open,
                &e.cost_class,
                with_health.then_some((&e.health_status, e.health_stale)),
            )
        })
        .collect();
    let jev = inner.deps.jev.as_ref().map(|j| j.config().cache_identity());
    routing.routing_extension = Some(
        ssx_json::to_string(&(
            &routing.routing_extension,
            generations,
            eligibility,
            &scope.selected_engines,
            &scope.jev_scores,
            jev,
            request.page,
        ))
        .expect("logical identity"),
    );
    composed_key(
        &CacheKeyInput {
            query: request.query.clone(),
            language: request.language.clone(),
            safesearch: request.safesearch.into(),
            categories: request.categories.clone().unwrap_or_default(),
            engines: request.engines.clone().unwrap_or_default(),
            page: i64::try_from(request.page).unwrap_or(i64::MAX),
            time_range: request.time_range.clone(),
            media_type: request.media_type.clone(),
            date_from: request.date_from.clone(),
            date_to: request.date_to.clone(),
            relative_window: window,
        },
        &routing,
        request.interactive_timeout_ms,
        request.generate_suggestions,
    )
}
fn personalize(
    response: &mut SearchResponse,
    request: &SearchRequest,
    mut scope: ScopeDecision,
    query_id: String,
    start: Instant,
) {
    response.query = request.query.clone();
    response.query_id = query_id;
    if response.deadline_exceeded
        && !scope
            .warnings
            .iter()
            .any(|w| w.contains("Interactive deadline"))
    {
        scope.warnings.push(
            "Interactive deadline reached; captured results or suggestions may be incomplete."
                .into(),
        );
    }
    response.scope = scope;
    response.response_time_ms = i64::try_from(start.elapsed().as_millis()).unwrap_or(i64::MAX);
}
async fn cancelled(rx: &mut watch::Receiver<bool>) {
    loop {
        if *rx.borrow_and_update() {
            return;
        }
        if rx.changed().await.is_err() {
            return;
        }
    }
}
fn synthetic(status: EngineStatus, message: &str) -> AdapterResponse {
    AdapterResponse {
        status,
        synthetic: true,
        error_message: Some(message.into()),
        ..Default::default()
    }
}
struct Finished(usize, AdapterResponse);
async fn execute(
    inner: &Arc<Inner>,
    mut request: SearchRequest,
    scope: ScopeDecision,
    start: Instant,
    mut cancel: watch::Receiver<bool>,
) -> Result<(CanonicalResponse, CacheAdmission), SearchError> {
    let date_filter = publication_date_proof(&request, &scope)?;
    let mut names = Vec::new();
    for name in &scope.selected_engines {
        if !names.contains(name) {
            names.push(name.clone());
        }
    }
    let timeouts: Vec<_> = names
        .iter()
        .map(|n| inner.deps.engines[n].timeout)
        .collect();
    let budget = Duration::from_secs(10)
        .max(timeouts.iter().copied().max().unwrap_or_default())
        .max(
            timeouts
                .iter()
                .copied()
                .fold(Duration::ZERO, Duration::saturating_add)
                .min(Duration::from_secs(30)),
        );
    let interactive = request.interactive_timeout_ms.is_some();
    let budget = request
        .interactive_timeout_ms
        .map_or(budget, |ms| budget.min(Duration::from_millis(ms)));
    let deadline = start
        .checked_add(budget)
        .ok_or(SearchError::InvalidConfiguration)?;
    // Media intent determines adapter endpoint even with an explicit text category.
    if let Some(media) = request.media_type.as_deref() {
        request.categories = Some(vec![
            if media == "image" { "images" } else { "videos" }.into(),
        ]);
    } else if request.categories.as_ref().is_none_or(Vec::is_empty) {
        request.categories = Some(vec!["general".into()]);
    }
    let request = Arc::new(request);
    let started = Arc::new(Mutex::new(vec![false; names.len()]));
    let mut tasks = JoinSet::new();
    let mut task_ids = HashMap::new();
    for (index, name) in names.iter().enumerate() {
        let binding = inner.deps.engines[name].clone();
        let semaphore = inner.engines.clone();
        let request = request.clone();
        let started = started.clone();
        let owner = inner.clone();
        let name = name.clone();
        let task = tasks.spawn(async move {
            if Instant::now() >= deadline {
                return Finished(
                    index,
                    synthetic(
                        EngineStatus::Unavailable,
                        "not started before the search deadline",
                    ),
                );
            }
            if let Some(cooldowns) = &owner.deps.cooldowns
                && let Ok(key) = ssx_domain::rate::EngineKey::new(&name)
                && let Ok(ssx_domain::rate::SharedCooldown::Active { seconds }) =
                    cooldowns.cooldown(&key).await
                && seconds > 0
            {
                return Finished(
                    index,
                    synthetic(
                        EngineStatus::RateLimited,
                        &format!("upstream cooldown active; retry in {seconds}s"),
                    ),
                );
            }
            let Ok(_permit) = semaphore.acquire_owned().await else {
                return Finished(
                    index,
                    synthetic(EngineStatus::Unavailable, "service closed"),
                );
            };
            if Instant::now() >= deadline {
                return Finished(
                    index,
                    synthetic(
                        EngineStatus::Unavailable,
                        "not started before the search deadline",
                    ),
                );
            }
            if owner
                .health
                .lock()
                .unwrap()
                .get(&name)
                .is_some_and(|h| !h.circuit_allowed(owner.deps.clock.capture().0))
            {
                return Finished(index, synthetic(EngineStatus::Error, "circuit open"));
            }
            started.lock().unwrap()[index] = true;
            let engine_deadline = Instant::now() + binding.timeout;
            let mut response =
                tokio::time::timeout_at(engine_deadline, binding.engine.search(&request))
                    .await
                    .unwrap_or_else(|_| timeout_response(binding.timeout));
            if Instant::now() >= engine_deadline {
                response = timeout_response(binding.timeout);
            }
            Finished(index, response)
        });
        task_ids.insert(task.id(), index);
    }
    let mut suggestion_tasks = JoinSet::new();
    if request.generate_suggestions
        && Instant::now() < deadline
        && let Some(suggestions) = inner.deps.suggestions.clone()
    {
        let query = request.query.clone();
        let timeout = inner.config.optional_port_timeout;
        suggestion_tasks.spawn(async move {
            tokio::time::timeout(timeout, suggestions.fetch(&query))
                .await
                .unwrap_or_default()
        });
    }
    let mut responses: Vec<Option<AdapterResponse>> = vec![None; names.len()];
    // JoinError is a service-owned defect, never an adapter-returned outcome.
    let mut internal_failures = std::collections::BTreeSet::new();
    let mut truncated = false;
    let mut was_cancelled = false;
    while !tasks.is_empty() {
        tokio::select! {
            biased;
            _ = cancelled(&mut cancel) => { was_cancelled = true; break; }
            _ = tokio::time::sleep_until(deadline) => { truncated = true; break; }
            result = tasks.join_next_with_id() => match result {
                Some(Ok((_, Finished(i, response)))) => responses[i] = Some(response),
                Some(Err(error)) => { if let Some(i) = task_ids.get(&error.id()) { internal_failures.insert(*i); responses[*i] = Some(synthetic(EngineStatus::Error, "internal adapter task failure")); } },
                _ => {},
            }
        }
    }
    tasks.shutdown().await;
    if was_cancelled {
        suggestion_tasks.shutdown().await;
        return Err(SearchError::Cancelled);
    }
    let now = inner.deps.clock.capture().0;
    let started = started.lock().unwrap().clone();
    let mut feeds = Feeds::new();
    let mut response = SearchResponse {
        query: request.query.clone(),
        date_filter,
        scope: scope.clone(),
        deadline_exceeded: interactive && truncated,
        dispatched_engine_count: started.iter().filter(|v| **v).count() as i64,
        ranking_explanation: ranking_explanation(&inner.config.ranking_strategy).into(),
        ..Default::default()
    };
    let cooldown = |i: usize| {
        responses[i]
            .as_ref()
            .is_some_and(|r| r.synthetic && r.status == EngineStatus::RateLimited && !started[i])
    };
    let circuit = |i: usize| {
        responses[i].as_ref().is_some_and(|r| {
            r.synthetic && r.error_message.as_deref() == Some("circuit open") && !started[i]
        })
    };
    let mut outcome_order: Vec<_> = (0..names.len())
        .filter(|i| !cooldown(*i) && !circuit(*i))
        .collect();
    outcome_order.extend((0..names.len()).filter(|i| cooldown(*i)));
    outcome_order.extend((0..names.len()).filter(|i| circuit(*i)));
    for i in outcome_order {
        let name = &names[i];
        let mut result = responses[i].take().unwrap_or_else(|| {
            if started[i] && !interactive {
                timeout_response(budget)
            } else {
                let mut response = synthetic(
                    EngineStatus::Unavailable,
                    if interactive {
                        "interactive deadline reached"
                    } else {
                        "not started before the search deadline"
                    },
                );
                if started[i] {
                    response.latency_ms =
                        ssx_domain::FiniteF64::new(budget.as_secs_f64() * 1000.0).unwrap();
                }
                response
            }
        });
        let origin = if internal_failures.contains(&i) {
            None
        } else {
            Some(if !started[i] {
                OutcomeOrigin::Undispatched
            } else if result.synthetic && interactive && result.status == EngineStatus::Unavailable
            {
                OutcomeOrigin::CallerCancelled
            } else if result.synthetic && result.status == EngineStatus::Timeout {
                OutcomeOrigin::DispatchedSyntheticTimeout
            } else {
                OutcomeOrigin::AdapterReturned
            })
        };
        match origin {
            None => {
                inner
                    .deps
                    .metrics
                    .increment("search_engine_internal_failure");
                inner.deps.metrics.increment("search_engine_synthetic");
            }
            Some(origin) => {
                inner
                    .health
                    .lock()
                    .unwrap()
                    .entry(name.clone())
                    .or_default()
                    .observe(origin, &result, now);
                inner
                    .deps
                    .metrics
                    .increment(if matches!(origin, OutcomeOrigin::AdapterReturned) {
                        "search_engine_observed"
                    } else {
                        "search_engine_synthetic"
                    });
            }
        }
        for row in &mut result.results {
            row.tier = if inner.deps.policy.resolver.tier1.contains(name) {
                1
            } else {
                2
            };
        }
        if let Some(range) = &request.time_range
            && inner.deps.policy.catalog.get(name).is_some_and(|e| {
                e.enforced_filters
                    .get("time_range")
                    .is_some_and(|v| v == "local")
            })
            && let Some(day) = request
                .time_range_anchor
                .as_deref()
                .and_then(ssx_application::filters::publication_day)
        {
            result.results =
                ssx_application::filters::filter_results_by_time_range(&result.results, range, day);
        }
        if inner.config.empty_scrape_diagnostics
            && inner.deps.engines[name].scrape
            && result.status == EngineStatus::Ok
            && result.results.is_empty()
        {
            response.empty_engines.push(vec![
                name.clone(),
                "successful scrape returned no results".into(),
            ]);
        }
        response.engine_outcomes.push(EngineOutcome {
            engine: name.clone(),
            status: status(&result.status).into(),
            result_count: result.results.len() as i64,
            latency_ms: Some(
                ssx_domain::FiniteF64::new(if (result.latency_ms.get() * 10.0).is_finite() {
                    (result.latency_ms.get() * 10.0).round_ties_even() / 10.0
                } else {
                    result.latency_ms.get()
                })
                .unwrap(),
            ),
            message: result.error_message,
        });
        response.answers.extend(result.answers);
        response.corrections.extend(result.corrections);
        response.infoboxes.extend(result.infoboxes);
        feeds.insert(name.clone(), result.results);
    }
    response.all_unresponsive = response.engine_outcomes.iter().all(|r| r.status != "ok");
    response.partial = !response.all_unresponsive
        && (response.deadline_exceeded
            || response.engine_outcomes.iter().any(|r| r.status != "ok"));
    response.results = rank(
        &group_publications(&feeds, &request.query),
        &inner.config.ranking_strategy,
        &BTreeMap::new(),
    );
    for row in &mut response.results {
        if let Some(representative) = row
            .work_group
            .as_ref()
            .and_then(|g| g.get("representative_engine"))
            .and_then(ssx_json::Value::as_str)
        {
            row.engine = representative.into();
        }
    }
    let mut rerank_state = RerankState::Skipped;
    let sensitive = &inner.deps.policy.policy.sensitive_engines;
    if let Some(reranker) = &inner.deps.reranker {
        if response.results.len() > 1
            && !names.iter().any(|n| sensitive.contains(n))
            && !response.results.iter().any(|r| {
                sensitive.contains(&r.engine) || r.engines.iter().any(|n| sensitive.contains(n))
            })
        {
            let rerank_deadline = if interactive {
                deadline.min(Instant::now() + Duration::from_secs(1))
            } else {
                Instant::now() + Duration::from_secs(1)
            };
            if rerank_deadline > Instant::now() {
                let mut rerank_tasks = JoinSet::new();
                let stage = reranker.clone();
                let stage_request = request.clone();
                let stage_results = response.results.clone();
                rerank_tasks.spawn(async move {
                    stage
                        .rerank(&stage_request, &stage_results, rerank_deadline)
                        .await
                });
                let decision = tokio::select! { _ = cancelled(&mut cancel) => { rerank_tasks.shutdown().await; suggestion_tasks.shutdown().await; return Err(SearchError::Cancelled); }, decision = rerank_tasks.join_next() => decision, _ = tokio::time::sleep_until(rerank_deadline) => None };
                rerank_tasks.shutdown().await;
                match decision {
                    Some(Ok(Ok(decision))) => {
                        rerank_state = decision.state;
                        response.results = decision.results;
                    }
                    _ => rerank_state = RerankState::Fallback,
                }
            }
        }
    } else if !scope.jev_added_engines.is_empty() {
        response.results = promote_specialists(response.results, &feeds, &scope);
    }
    if rerank_state == RerankState::Applied {
        response.ranking_explanation = "semantic_shortlist_rerank".into();
        for (i, row) in response.results.iter_mut().enumerate() {
            row.position = i as i64 + 1;
        }
    }
    if !suggestion_tasks.is_empty() {
        tokio::select! {
            _ = cancelled(&mut cancel) => { suggestion_tasks.shutdown().await; return Err(SearchError::Cancelled); }
            result = suggestion_tasks.join_next() => { if let Some(Ok(suggestions)) = result { response.suggestions = suggestions; } }
            _ = tokio::time::sleep_until(deadline), if interactive => { response.deadline_exceeded = true; }
        }
    }
    suggestion_tasks.shutdown().await;
    response.partial = !response.all_unresponsive
        && (response.deadline_exceeded
            || response.engine_outcomes.iter().any(|r| r.status != "ok"));
    let admission = if response.deadline_exceeded {
        CacheAdmission::DeadlineTruncated
    } else if rerank_state == RerankState::Fallback {
        CacheAdmission::RerankFallback
    } else if response.all_unresponsive {
        CacheAdmission::AllUnresponsive
    } else if response.partial {
        CacheAdmission::Partial
    } else {
        CacheAdmission::Complete
    };
    Ok((CanonicalResponse::new(response), admission))
}
fn status(status: &EngineStatus) -> &'static str {
    match status {
        EngineStatus::Ok => "ok",
        EngineStatus::RateLimited => "rate_limited",
        EngineStatus::Blocked => "blocked",
        EngineStatus::Error => "error",
        EngineStatus::Timeout => "timeout",
        EngineStatus::Unavailable => "unavailable",
    }
}
fn publication_date_proof(
    request: &SearchRequest,
    scope: &ScopeDecision,
) -> Result<Option<ssx_domain::PublicationDateProof>, SearchError> {
    if request.date_from.is_none() && request.date_to.is_none() {
        return Ok(None);
    }
    if scope.selected_engines != ["openalex"] {
        return Err(SearchError::InvalidConfiguration);
    }
    ssx_domain::PublicationDateProof::openalex(request.date_from.clone(), request.date_to.clone())
        .map(Some)
        .map_err(|_| SearchError::InvalidConfiguration)
}
fn timeout_response(duration: Duration) -> AdapterResponse {
    let mut seconds = duration.as_secs_f64().to_string();
    if !seconds.contains('.') {
        seconds.push_str(".0");
    }
    AdapterResponse {
        status: EngineStatus::Timeout,
        synthetic: true,
        error_message: Some(format!("timed out after {seconds}s")),
        latency_ms: ssx_domain::FiniteF64::new(duration.as_secs_f64() * 1000.0)
            .expect("finite timeout"),
        ..Default::default()
    }
}
fn promote_specialists(
    ranked: Vec<ssx_domain::SearchResult>,
    feeds: &Feeds,
    scope: &ScopeDecision,
) -> Vec<ssx_domain::SearchResult> {
    let mut ordered = scope.jev_added_engines.clone();
    ordered.sort_by(|a, b| {
        scope
            .jev_scores
            .get(b)
            .map_or(0.0, |v| v.get())
            .total_cmp(&scope.jev_scores.get(a).map_or(0.0, |v| v.get()))
            .then_with(|| a.cmp(b))
    });
    let mut seen = std::collections::BTreeSet::new();
    let mut promoted = Vec::new();
    for name in ordered {
        if let Some(rows) = feeds.get(&name) {
            for row in rows {
                let marker = if row.url.is_empty() {
                    format!("{name}:{}", row.title)
                } else {
                    row.url.clone()
                };
                if seen.insert(marker) {
                    promoted.push(row.clone());
                    break;
                }
            }
        }
    }
    for row in ranked {
        let marker = if row.url.is_empty() {
            format!("merged:{}", row.title)
        } else {
            row.url.clone()
        };
        if seen.insert(marker) {
            promoted.push(row);
        }
    }
    promoted
}

/// Native server-authoritative per-client fixed-window admission. A digest keeps
/// raw client identities out of a dedicated, provider-disjoint F10 namespace.
pub struct DistributedClientAdmission {
    pub quota: Arc<crate::valkey::rate::DistributedQuota>,
    pub fail_closed: bool,
}
impl ClientAdmission for DistributedClientAdmission {
    fn acquire<'a>(&'a self, client: &'a str) -> PortFuture<'a, bool> {
        Box::pin(async move {
            let Ok(key) = crate::valkey::rate::ClientQuotaKey::new(client) else {
                return false;
            };
            match self.quota.acquire_client(&key, 1).await {
                Ok(receipt) => receipt.allowed,
                Err(
                    ssx_domain::rate::RateFailure::Closed
                    | ssx_domain::rate::RateFailure::InvalidInput,
                ) => false,
                Err(_) => !self.fail_closed,
            }
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ssx_application::{
        NoopSink,
        policy::SearchPolicy,
        scope::{CatalogSnapshot, ScopeResolver},
    };
    use std::sync::atomic::AtomicUsize;
    struct NeverPolled(Arc<AtomicUsize>);
    impl SearchEngine for NeverPolled {
        fn search<'a>(&'a self, _: &'a SearchRequest) -> PortFuture<'a, AdapterResponse> {
            self.0.fetch_add(1, Ordering::SeqCst);
            Box::pin(async { AdapterResponse::default() })
        }
    }
    #[tokio::test]
    async fn zero_remaining_budget_never_polls_engine() {
        let calls = Arc::new(AtomicUsize::new(0));
        let service = SearchService::new(
            SearchConfig::default(),
            SearchDependencies {
                policy: PolicyService {
                    policy: SearchPolicy::default(),
                    catalog: CatalogSnapshot::new(vec![]).unwrap(),
                    resolver: ScopeResolver::default(),
                },
                engines: BTreeMap::from([(
                    "brave".into(),
                    EngineBinding {
                        engine: Arc::new(NeverPolled(calls.clone())),
                        generation: "zero-budget".into(),
                        timeout: Duration::from_secs(3),
                        scrape: false,
                    },
                )]),
                routing_identity: RoutingIdentity {
                    catalog: None,
                    sensitive: Default::default(),
                    tier1: Default::default(),
                    budget: None,
                    ranking_strategy: "presence".into(),
                    work_group_version: "scholarly-work-v3".into(),
                    rerank_identity: None,
                    routing_extension: None,
                },
                cache: None,
                client_admission: None,
                suggestions: None,
                jev: None,
                routing_cache: None,
                metrics: Arc::new(NoopSink),
                supervisor: Arc::new(NoopSink),
                audit: Arc::new(NoopSink),
                cooldowns: None,
                owned_http: vec![],
                owned_stores: vec![],
                clock: Arc::new(SystemSearchClock),
                reranker: None,
            },
        )
        .unwrap();
        let (_sender, receiver) = watch::channel(false);
        // This is an internal elapsed-execution-budget seam, not a transport input.
        let (response, admission) = execute(
            &service.0,
            SearchRequest {
                query: "zero".into(),
                interactive_timeout_ms: Some(1),
                ..Default::default()
            },
            ScopeDecision {
                selected_engines: vec!["brave".into()],
                ..Default::default()
            },
            Instant::now() - Duration::from_secs(1),
            receiver,
        )
        .await
        .unwrap();
        assert_eq!(calls.load(Ordering::SeqCst), 0);
        assert_eq!(response.response().dispatched_engine_count, 0);
        assert!(response.response().deadline_exceeded);
        assert_eq!(admission, CacheAdmission::DeadlineTruncated);
        service.shutdown().await;
    }
}
