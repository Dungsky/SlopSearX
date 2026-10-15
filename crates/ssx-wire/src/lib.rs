//! Foundation-only DTOs. F05 owns search and durable contract codecs.
use serde::{Deserialize, Serialize};
#[derive(Debug, Deserialize, Serialize, PartialEq, Eq)]
pub struct Readiness {
    pub status: String,
    pub mode: String,
}
impl Readiness {
    pub fn foundation() -> Self {
        Self {
            status: "ok".into(),
            mode: "foundation".into(),
        }
    }
}
