//! Closed observer vocabulary; composition adapters must preserve these outcomes.
use super::codec::EventType;
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ReadOutcome {
    Delivered,
    Empty,
    Gap,
    Redacted,
    Rejected,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AckOutcome {
    Advanced,
    Idempotent,
    Rejected,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Capacity {
    Stream,
    Consumer,
}
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct BacklogAge(f64);
impl BacklogAge {
    pub fn new(seconds: f64) -> Result<Self, ssx_domain::DomainError> {
        if !seconds.is_finite() || seconds < 0.0 {
            return Err(ssx_domain::DomainError("invalid event backlog age"));
        }
        Ok(Self(seconds))
    }
    pub fn seconds(self) -> f64 {
        self.0
    }
}
#[derive(Debug, Clone, PartialEq)]
pub enum EventObservation {
    Published(EventType),
    Read {
        outcome: ReadOutcome,
        oldest_age_seconds: Option<BacklogAge>,
    },
    Ack(AckOutcome),
    Capacity(Capacity),
}
pub trait EventObserver: Send + Sync {
    fn observe(&self, event: EventObservation);
}
/// Explicit absence of an exporter; this does not establish O01 metric parity.
#[derive(Default)]
pub struct NoopEventObserver;
impl EventObserver for NoopEventObserver {
    fn observe(&self, _: EventObservation) {}
}
