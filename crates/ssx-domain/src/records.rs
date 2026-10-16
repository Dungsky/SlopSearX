//! Canonical internal search records. Surface DTOs are defined independently.
use crate::DomainError;
use serde::{Deserialize, Serialize};
use ssx_json::Value;
use std::collections::{BTreeMap, BTreeSet};

/// A finite JSON number retaining the caller's integer/float representation.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(transparent)]
pub struct FiniteF64(ssx_json::Number);
impl FiniteF64 {
    pub fn new(value: f64) -> Result<Self, DomainError> {
        if !value.is_finite() {
            return Err(DomainError("number must be finite"));
        }
        let ssx_json::Value::Number(number) =
            ssx_json::to_value(value).map_err(|_| DomainError("invalid number"))?
        else {
            return Err(DomainError("invalid number"));
        };
        Ok(Self(number))
    }
    pub fn from_number(number: ssx_json::Number) -> Result<Self, DomainError> {
        if number.as_f64().is_none() {
            return Err(DomainError("number must be finite"));
        }
        Ok(Self(number))
    }
    pub fn get(&self) -> f64 {
        self.0.as_f64().expect("finite number invariant")
    }
    pub fn as_f64(&self) -> f64 {
        self.get()
    }
    pub fn as_number(&self) -> &ssx_json::Number {
        &self.0
    }
}
impl Default for FiniteF64 {
    fn default() -> Self {
        Self::new(0.0).expect("finite zero")
    }
}
impl<'de> Deserialize<'de> for FiniteF64 {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        Self::from_number(ssx_json::Number::deserialize(deserializer)?)
            .map_err(serde::de::Error::custom)
    }
}
#[derive(Debug, Clone, Default, PartialEq)]
pub struct MediaInfo {
    pub media_type: String,
    pub url: Option<String>,
    pub thumbnail: Option<String>,
    pub source: Option<String>,
    pub width: Option<i64>,
    pub height: Option<i64>,
    pub duration: Option<FiniteF64>,
}
#[derive(Debug, Clone, PartialEq)]
pub struct SearchResult {
    pub url: String,
    pub title: String,
    pub content: String,
    pub engine: String,
    pub engines: BTreeSet<String>,
    pub score: FiniteF64,
    pub position: i64,
    pub category: String,
    pub published_date: Option<String>,
    pub thumbnail: Option<String>,
    pub img_src: Option<String>,
    pub tier: i64,
    pub media: Option<MediaInfo>,
    pub payload: Option<Value>,
    pub work_group: Option<Value>,
}
impl Default for SearchResult {
    fn default() -> Self {
        Self {
            url: String::new(),
            title: String::new(),
            content: String::new(),
            engine: String::new(),
            engines: BTreeSet::new(),
            score: FiniteF64::default(),
            position: 0,
            category: "general".into(),
            published_date: None,
            thumbnail: None,
            img_src: None,
            tier: 1,
            media: None,
            payload: None,
            work_group: None,
        }
    }
}
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EngineStatus {
    #[default]
    Ok,
    RateLimited,
    Blocked,
    Error,
    Timeout,
    Unavailable,
}

#[derive(Debug, Clone, Default, PartialEq)]
pub struct AdapterResponse {
    pub results: Vec<SearchResult>,
    pub status: EngineStatus,
    pub error_message: Option<String>,
    pub latency_ms: FiniteF64,
    pub answers: Vec<Value>,
    pub corrections: Vec<String>,
    pub infoboxes: Vec<Value>,
    pub synthetic: bool,
}
impl AdapterResponse {
    /// Synthetic deadline responses must never enter observed adapter health metrics.
    pub fn observed_latency_ms(&self) -> Option<FiniteF64> {
        (!self.synthetic && self.latency_ms.get() != 0.0).then(|| self.latency_ms.clone())
    }
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EngineExclusion {
    pub engine: String,
    pub reason: String,
    pub stage: String,
}
impl Default for EngineExclusion {
    fn default() -> Self {
        Self {
            engine: String::new(),
            reason: String::new(),
            stage: "unknown".into(),
        }
    }
}
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct RoutingTradeoff {
    pub kind: String,
    pub detail: String,
}
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct ScopeDecision {
    pub selected_engines: Vec<String>,
    pub resolved_categories: Vec<String>,
    pub routing_rule: String,
    pub matched_topic: Option<String>,
    pub warnings: Vec<String>,
    pub excluded_engines: Vec<EngineExclusion>,
    pub routing_fallback: bool,
    pub routing_budget_applied: bool,
    pub routing_tradeoffs: Vec<RoutingTradeoff>,
    pub jev_added_engines: Vec<String>,
    pub jev_scores: BTreeMap<String, FiniteF64>,
}
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct EngineOutcome {
    pub engine: String,
    pub status: String,
    pub result_count: i64,
    pub latency_ms: Option<FiniteF64>,
    pub message: Option<String>,
}
#[derive(Debug, Clone, PartialEq)]
pub struct SearchResponse {
    pub query: String,
    pub results: Vec<SearchResult>,
    pub scope: ScopeDecision,
    pub engine_outcomes: Vec<EngineOutcome>,
    pub suggestions: Vec<String>,
    pub answers: Vec<Value>,
    pub corrections: Vec<String>,
    pub infoboxes: Vec<Value>,
    pub query_id: String,
    pub cached: bool,
    pub response_time_ms: i64,
    pub partial: bool,
    pub deadline_exceeded: bool,
    pub all_unresponsive: bool,
    pub empty_engines: Vec<Vec<String>>,
    pub cached_error: bool,
    pub ranking_explanation: String,
    pub dispatched_engine_count: i64,
    pub date_filter: Option<crate::PublicationDateProof>,
}
impl Default for SearchResponse {
    fn default() -> Self {
        Self {
            query: String::new(),
            results: vec![],
            scope: ScopeDecision::default(),
            engine_outcomes: vec![],
            suggestions: vec![],
            answers: vec![],
            corrections: vec![],
            infoboxes: vec![],
            query_id: String::new(),
            cached: false,
            response_time_ms: 0,
            partial: false,
            deadline_exceeded: false,
            all_unresponsive: false,
            empty_engines: vec![],
            cached_error: false,
            ranking_explanation: "tier_then_cross_engine_presence".into(),
            dispatched_engine_count: 0,
            date_filter: None,
        }
    }
}
#[derive(Debug, Clone, PartialEq)]
pub struct SearchRequest {
    pub query: String,
    pub categories: Option<Vec<String>>,
    pub engines: Option<Vec<String>>,
    pub language: String,
    pub page: u64,
    pub time_range: Option<String>,
    pub safesearch: u8,
    pub media_type: Option<String>,
    pub max_results: Option<u64>,
    pub include: BTreeSet<String>,
    pub freshness: String,
    pub client_identifier: Option<String>,
    pub interactive_timeout_ms: Option<u64>,
    pub generate_suggestions: bool,
    pub execution_isolation_key: Option<String>,
    pub date_from: Option<String>,
    pub date_to: Option<String>,
    pub time_range_anchor: Option<String>,
}
impl Default for SearchRequest {
    fn default() -> Self {
        Self {
            query: String::new(),
            categories: None,
            engines: None,
            language: "en".into(),
            page: 1,
            time_range: None,
            safesearch: 0,
            media_type: None,
            max_results: None,
            include: ["results", "suggestions", "engine_status", "diagnostics"]
                .into_iter()
                .map(str::to_owned)
                .collect(),
            freshness: "no_preference".into(),
            client_identifier: None,
            interactive_timeout_ms: None,
            generate_suggestions: true,
            execution_isolation_key: None,
            date_from: None,
            date_to: None,
            time_range_anchor: None,
        }
    }
}

#[cfg(test)]
mod finite_number_tests {
    use super::*;
    #[test]
    fn representation_and_large_integer_identity_are_preserved() {
        let integer = FiniteF64::from_number(ssx_json::Number::new("1").unwrap()).unwrap();
        let float = FiniteF64::new(1.0).unwrap();
        assert_ne!(integer, float);
        let first =
            FiniteF64::from_number(ssx_json::Number::new("9007199254740992").unwrap()).unwrap();
        let second =
            FiniteF64::from_number(ssx_json::Number::new("9007199254740993").unwrap()).unwrap();
        assert_ne!(first, second);
        assert_eq!(first.get(), second.get());
        assert_eq!(ssx_json::to_string(&second).unwrap(), "9007199254740993");
    }
}
