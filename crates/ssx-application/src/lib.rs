//! Consumer-owned injection seams and pure deterministic search algorithms.
use ssx_domain::{AdapterOutcome, AdapterResponse, SearchRequest, StoreRead};
use std::{future::Future, pin::Pin, sync::Arc};
pub type PortFuture<'a, T> = Pin<Box<dyn Future<Output = T> + Send + 'a>>;
/// Application-owned search port implemented by concrete engine adapters.
/// This executable port is distinct from `Engine`, which is only a foundation
/// readiness probe and does not represent provider search behavior.
pub trait SearchEngine: Send + Sync {
    fn search<'a>(&'a self, request: &'a SearchRequest) -> PortFuture<'a, AdapterResponse>;
}
/// Application-owned pre-dispatch admission result, kept independent of Valkey.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AdmissionDecision {
    Allowed,
    Denied,
    Unavailable,
}
/// Shared quota/cooldown authority injected into provider adapter factories.
pub trait SearchAdmission: Send + Sync {
    fn admit<'a>(&'a self, engine: &'static str) -> PortFuture<'a, AdmissionDecision>;
}
/// Foundation readiness probe only; never use this as the adapter search API.
pub trait Engine: Send + Sync {
    fn probe(&self) -> PortFuture<'_, AdapterOutcome>;
}
pub trait Store: Send + Sync {
    fn probe(&self) -> PortFuture<'_, StoreRead<()>>;
}
pub trait Clock: Send + Sync {
    /// Persisted expiry uses wall time; never use this for elapsed deadlines.
    fn unix_seconds(&self) -> u64;
    /// Process-local elapsed ticks; never serialize these as durable timestamps.
    fn monotonic_elapsed(&self) -> std::time::Duration;
}
pub trait Metrics: Send + Sync {
    fn increment(&self, name: &'static str);
}
pub trait Audit: Send + Sync {
    fn record(&self, event: &'static str);
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TaskState {
    Started,
    Completed,
    Failed,
    Cancelled,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TaskEvent {
    pub task: &'static str,
    pub state: TaskState,
}
/// F14 owns spawn/cancellation/join. This sink observes typed lifecycle outcomes.
pub trait Supervisor: Send + Sync {
    fn observed(&self, event: TaskEvent);
}
#[derive(Default)]
pub struct NoopSink;
impl Metrics for NoopSink {
    fn increment(&self, _: &'static str) {}
}
impl Audit for NoopSink {
    fn record(&self, _: &'static str) {}
}
impl Supervisor for NoopSink {
    fn observed(&self, _: TaskEvent) {}
}
/// State-factory equivalent: surfaces receive the same explicit port bundle.
#[derive(Clone)]
pub struct Runtime {
    pub engine: Arc<dyn Engine>,
    pub store: Arc<dyn Store>,
    pub clock: Arc<dyn Clock>,
    pub metrics: Arc<dyn Metrics>,
    pub audit: Arc<dyn Audit>,
    pub supervisor: Arc<dyn Supervisor>,
}
impl Runtime {
    pub async fn probe(&self) -> bool {
        self.metrics.increment("foundation_readiness");
        self.audit.record("foundation_readiness");
        self.supervisor.observed(TaskEvent {
            task: "foundation_readiness",
            state: TaskState::Started,
        });
        let _now = self.clock.unix_seconds();
        let ready = self.engine.probe().await == AdapterOutcome::Success
            && matches!(self.store.probe().await, StoreRead::Found(()));
        self.supervisor.observed(TaskEvent {
            task: "foundation_readiness",
            state: if ready {
                TaskState::Completed
            } else {
                TaskState::Failed
            },
        });
        ready
    }
}

pub mod ranking;
pub mod retrieval;
pub mod scholarly;
mod urls;
pub use urls::normalise_url;

mod text;

mod calendar;
pub mod health;
pub mod rate;
pub mod research;
pub mod retry_after;

pub mod browser_identity;
pub mod filters;
pub mod policy;
pub mod scope;

pub mod oauth;

pub mod cache;

pub mod snapshot;
