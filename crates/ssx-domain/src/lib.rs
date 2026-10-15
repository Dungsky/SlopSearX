//! Internal foundation values; public and durable DTOs live elsewhere.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum StoreRead<T> {
    Found(T),
    Missing,
    Expired,
    Unavailable,
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AdapterOutcome {
    Success,
    Unavailable,
    InvalidRequest,
    InternalError,
}
