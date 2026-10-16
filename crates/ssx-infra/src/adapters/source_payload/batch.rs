//! Narrow science-result assembler. Source-row/scratch ownership is separate.
//! Requested backing accounting ends explicitly at into_adapter_response return;
//! G02 owns retained public response/clone/flight/cache lifetime qualification.
use super::{BudgetError, BudgetPayload, Projection};
use ssx_domain::{AdapterResponse, EngineStatus, FiniteF64, SearchResult};
use ssx_owned_budget::Budget;
use std::{collections::BTreeSet, time::Instant};

pub struct Skeleton<'a> {
    pub url: &'a str,
    pub title: &'a str,
    pub content: &'a str,
    pub engine: &'a str,
    pub category: &'a str,
    pub score: f64,
    pub position: i64,
    pub tier: i64,
    pub published_date: Option<&'a str>,
}
pub struct Outcome<'a> {
    pub status: EngineStatus,
    pub error_message: Option<&'a str>,
    pub latency_ms: f64,
    pub synthetic: bool,
}
/// Capacity is the caller's exact retained-source-row upper bound, checked and
/// reserved before allocation. Skipped rows leave conservative capacity charged
/// until handoff. This does not account earlier arbitrary String construction.
pub struct ProducedBatch<'a> {
    results: Vec<SearchResult>,
    state: Projection<'a>,
}
impl<'a> ProducedBatch<'a> {
    pub fn new(
        budget: &'a Budget,
        capacity: usize,
        deadline: Option<Instant>,
    ) -> Result<Self, BudgetError> {
        let bytes = std::alloc::Layout::array::<SearchResult>(capacity)
            .map_err(|_| BudgetError::Resource)?
            .size();
        let charge = budget
            .reserve(bytes.checked_add(25).ok_or(BudgetError::Resource)?)
            .map_err(|_| BudgetError::Resource)?;
        let mut results = Vec::new();
        results
            .try_reserve_exact(capacity)
            .map_err(|_| BudgetError::Resource)?;
        if results.capacity() != capacity {
            return Err(BudgetError::Resource);
        }
        Ok(Self {
            results,
            state: Projection {
                budget,
                charge,
                deadline,
            },
        })
    }
    /// Read-only access for inspection. Cloning outside accounting is forbidden
    /// in mandatory consumers; borrowed public types cannot prevent all clones.
    pub fn results(&self) -> &[SearchResult] {
        &self.results
    }
    pub fn charged_bytes(&self) -> usize {
        self.state.charge.bytes()
    }
    /// Fields are copied into charged storage while the caller retains their
    /// original owners: both copies must remain in the SAME operation budget.
    pub async fn push(
        &mut self,
        row: Skeleton<'_>,
        mut payload: Option<BudgetPayload>,
    ) -> Result<(), BudgetError> {
        self.state.checkpoint().await?;
        if self.results.len() == self.results.capacity() {
            return Err(BudgetError::Resource);
        }
        if !row.score.is_finite() {
            return Err(BudgetError::Projection(super::ProjectionError::Nonfinite));
        }
        if let Some(payload) = payload.as_mut() {
            self.state
                .charge
                .merge_from(&mut payload._charge)
                .map_err(|_| BudgetError::Resource)?;
        }
        let url = self.state.scalar(row.url).await?;
        let title = self.state.scalar(row.title).await?;
        let content = self.state.scalar(row.content).await?;
        let engine = self.state.scalar(row.engine).await?;
        let category = self.state.scalar(row.category).await?;
        let published_date = match row.published_date {
            Some(value) => Some(self.state.scalar(value).await?),
            None => None,
        };
        self.state.reserve(25)?;
        let score = FiniteF64::from_python_float(row.score).map_err(|_| BudgetError::Resource)?;
        let result = SearchResult {
            url,
            title,
            content,
            engine,
            engines: BTreeSet::new(),
            score,
            position: row.position,
            category,
            published_date,
            thumbnail: None,
            img_src: None,
            tier: row.tier,
            media: None,
            payload: payload.map(|payload| payload.value),
            work_group: None,
        };
        self.state.checkpoint().await?;
        self.results.push(result);
        Ok(())
    }
    /// Named final ownership boundary. Moves existing storage without clone or
    /// reallocation; its public lifetime and clones are then owned by G02.
    /// No serialization or response-retention bound is claimed beyond return.
    pub async fn into_adapter_response(
        mut self,
        outcome: Outcome<'_>,
    ) -> Result<AdapterResponse, BudgetError> {
        self.state.checkpoint().await?;
        if !outcome.latency_ms.is_finite() {
            return Err(BudgetError::Projection(super::ProjectionError::Nonfinite));
        }
        let error_message = match outcome.error_message {
            Some(value) => Some(self.state.scalar(value).await?),
            None => None,
        };
        let latency_ms =
            FiniteF64::from_python_float(outcome.latency_ms).map_err(|_| BudgetError::Resource)?;
        if outcome.status != EngineStatus::Ok {
            self.results.clear();
        }
        self.state.checkpoint().await?;
        let results = std::mem::take(&mut self.results);
        Ok(AdapterResponse {
            results,
            status: outcome.status,
            error_message,
            latency_ms,
            answers: Vec::new(),
            corrections: Vec::new(),
            infoboxes: Vec::new(),
            synthetic: outcome.synthetic,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::adapters::source_json::{Text, Value};
    fn row(content: &str) -> Skeleton<'_> {
        Skeleton {
            url: "https://example.test/1",
            title: "title",
            content,
            engine: "test",
            category: "general",
            score: 1.0,
            position: 1,
            tier: 1,
            published_date: Some("2024-02-29"),
        }
    }
    fn outcome(status: EngineStatus) -> Outcome<'static> {
        Outcome {
            status,
            error_message: None,
            latency_ms: 0.0,
            synthetic: false,
        }
    }
    #[tokio::test]
    async fn batch_owns_projection_capacity_overlap_and_named_handoff() {
        let budget = Budget::new(1024 * 1024).unwrap();
        let value = Value::String(Text::from_points(vec![0xd800]));
        let payload = super::super::project_field_budgeted_optional(&value, &budget, None)
            .await
            .unwrap();
        let field_charge = payload.charged_bytes();
        let mut batch = ProducedBatch::new(&budget, 2, None).unwrap();
        batch.push(row("content"), Some(payload)).await.unwrap();
        assert_eq!(budget.snapshot().unwrap().used, batch.charged_bytes());
        assert!(batch.charged_bytes() > field_charge);
        // A simultaneous backend reply buffer must reserve against the SAME
        // ledger while source results remain live; no serializer proof implied.
        let reply = ssx_owned_budget::OwnedVec::<u8>::with_capacity(&budget, 4096).unwrap();
        assert_eq!(
            budget.snapshot().unwrap().used,
            batch.charged_bytes() + 4096
        );
        drop(reply);
        let response = batch
            .into_adapter_response(outcome(EngineStatus::Ok))
            .await
            .unwrap();
        assert_eq!(response.results.len(), 1);
        assert_eq!(
            response.results[0].payload,
            Some(ssx_json::Value::String("\u{fffd}".into()))
        );
        assert_eq!(budget.snapshot().unwrap().used, 0); // Explicit public handoff.
    }
    #[tokio::test]
    async fn wrong_ledger_refusal_and_fixed_capacity_preserve_storage_ownership() {
        let a = Budget::new(1024 * 1024).unwrap();
        let b = Budget::new(1024 * 1024).unwrap();
        let value = Value::String(Text::from("payload"));
        let payload = super::super::project_field_budgeted_optional(&value, &b, None)
            .await
            .unwrap();
        let mut batch = ProducedBatch::new(&a, 1, None).unwrap();
        let before = a.snapshot().unwrap().used;
        assert!(matches!(
            batch.push(row("x"), Some(payload)).await,
            Err(BudgetError::Resource)
        ));
        assert_eq!(a.snapshot().unwrap().used, before);
        assert_eq!(b.snapshot().unwrap().used, 0);
        assert!(batch.results().is_empty());
        batch.push(row("x"), None).await.unwrap();
        assert!(matches!(
            batch.push(row("y"), None).await,
            Err(BudgetError::Resource)
        ));
        assert_eq!(batch.results().len(), 1);
        drop(batch);
        assert_eq!(a.snapshot().unwrap().used, 0);
        let tiny = Budget::new(1).unwrap();
        assert!(matches!(
            ProducedBatch::new(&tiny, 1, None),
            Err(BudgetError::Resource)
        ));
        assert_eq!(tiny.snapshot().unwrap().used, 0);
        assert!(matches!(
            ProducedBatch::new(&a, usize::MAX, None),
            Err(BudgetError::Resource)
        ));
        assert_eq!(a.snapshot().unwrap().used, 0);
    }
    #[tokio::test]
    async fn cancelled_push_drops_partial_storage_before_batch_charge() {
        let budget = Budget::new(16 * 1024 * 1024).unwrap();
        let content = "x".repeat(4 * 1024 * 1024);
        let mut batch = ProducedBatch::new(&budget, 1, None).unwrap();
        let initial = batch.charged_bytes();
        {
            let job = batch.push(row(&content), None);
            tokio::pin!(job);
            tokio::select! {
                r=&mut job=>panic!("must observe allocation before completion: {}",r.is_ok()),
                _=async{loop{if budget.snapshot().unwrap().used>=initial+content.len(){break;}tokio::task::yield_now().await;}}=>{}
            }
        }
        assert!(batch.results().is_empty());
        assert!(batch.charged_bytes() >= initial + content.len());
        drop(batch);
        assert_eq!(budget.snapshot().unwrap().used, 0);
    }
    #[tokio::test]
    async fn non_success_handoff_has_no_partial_rows_and_charges_error_copy() {
        let budget = Budget::new(1024 * 1024).unwrap();
        let mut batch = ProducedBatch::new(&budget, 1, None).unwrap();
        batch.push(row("x"), None).await.unwrap();
        let mut result = outcome(EngineStatus::Error);
        result.error_message = Some("source error");
        let response = batch.into_adapter_response(result).await.unwrap();
        assert!(response.results.is_empty());
        assert_eq!(response.error_message.as_deref(), Some("source error"));
        assert_eq!(budget.snapshot().unwrap().used, 0);
    }
}
