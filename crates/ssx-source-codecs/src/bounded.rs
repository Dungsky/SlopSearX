//! Charge-owning output primitives. Full bounded family dispatch is not yet wired.
use ssx_owned_budget::{Budget, Error as ResourceError, OwnedString, OwnedVec};
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Error {
    Resource(ResourceError),
    InvalidCodepoint,
    OutputBound,
    NonAscii,
}
impl From<ResourceError> for Error {
    fn from(error: ResourceError) -> Self {
        Self::Resource(error)
    }
}
/// Owned validated Python codepoints, including surrogates. No Clone or bare
/// Vec extraction: the backing capacity charge stays with the consumer value.
pub struct BudgetText {
    points: OwnedVec<u32>,
}
impl BudgetText {
    pub fn codepoints(&self) -> &[u32] {
        self.points.as_slice()
    }
    pub fn charged_bytes(&self) -> usize {
        self.points.charged_bytes()
    }
    pub fn capacity(&self) -> usize {
        self.points.capacity()
    }
}
/// Fixed output capacity, reserved before allocation. Family consumption proofs
/// provide the limit; exceeding it is an invariant error, never silent growth.
pub struct BudgetTextBuilder {
    points: OwnedVec<u32>,
    limit: usize,
}
impl BudgetTextBuilder {
    pub fn new(budget: &Budget, limit: usize) -> Result<Self, Error> {
        Ok(Self {
            points: OwnedVec::with_capacity(budget, limit)?,
            limit,
        })
    }
    pub fn try_push(&mut self, point: u32) -> Result<(), Error> {
        if point > 0x10ffff {
            return Err(Error::InvalidCodepoint);
        }
        if self.points.len() == self.limit {
            return Err(Error::OutputBound);
        }
        self.points.try_push(point)?;
        Ok(())
    }
    pub fn finish(self) -> BudgetText {
        BudgetText {
            points: self.points,
        }
    }
}
/// ASCII selector result retains its own string capacity charge until dropped.
pub struct BudgetAscii {
    value: OwnedString,
}
impl BudgetAscii {
    pub fn try_from_str(budget: &Budget, value: &str) -> Result<Self, Error> {
        if !value.is_ascii() {
            return Err(Error::NonAscii);
        }
        Ok(Self {
            value: OwnedString::try_from_str(budget, value)?,
        })
    }
    pub fn as_str(&self) -> &str {
        self.value.as_str()
    }
    pub fn charged_bytes(&self) -> usize {
        self.value.charged_bytes()
    }
}
/// Resource failures are returned separately by the future bounded strict
/// dispatcher; they must never become this enum's UnicodeFailure/fallback.
pub enum BoundedStrictAsciiOutcome {
    Ascii(BudgetAscii),
    NonAscii,
    UnicodeFailure,
}
/// Shared fallible emission seam; no separate bounded Unicode algorithm.
pub(crate) trait PointSink {
    fn emit(&mut self, point: u32) -> Result<(), Error>;
}
impl PointSink for Vec<u32> {
    fn emit(&mut self, point: u32) -> Result<(), Error> {
        self.push(point);
        Ok(())
    }
}
impl PointSink for BudgetTextBuilder {
    fn emit(&mut self, point: u32) -> Result<(), Error> {
        self.try_push(point)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn returned_codepoints_keep_exact_capacity_charge_and_preserve_surrogates() {
        let budget = Budget::new(16).unwrap();
        let mut builder = BudgetTextBuilder::new(&budget, 4).unwrap();
        for point in [0, 0xd800, 0xdc00, 0x10ffff] {
            builder.try_push(point).unwrap();
        }
        assert_eq!(builder.try_push(1), Err(Error::OutputBound));
        let result = builder.finish();
        assert_eq!(result.codepoints(), [0, 0xd800, 0xdc00, 0x10ffff]);
        assert_eq!(result.charged_bytes(), 16);
        assert!(budget.reserve(1).is_err());
        drop(result);
        assert_eq!(budget.snapshot().unwrap().used, 0);
    }
    #[test]
    fn preallocation_shortage_overflow_invalid_points_and_early_drop_fail_closed() {
        let budget = Budget::new(15).unwrap();
        assert!(matches!(
            BudgetTextBuilder::new(&budget, 4),
            Err(Error::Resource(ResourceError::Exhausted))
        ));
        assert_eq!(budget.snapshot().unwrap().used, 0);
        assert!(matches!(
            BudgetTextBuilder::new(&budget, usize::MAX),
            Err(Error::Resource(ResourceError::Overflow))
        ));
        let mut builder = BudgetTextBuilder::new(&budget, 3).unwrap();
        assert_eq!(builder.try_push(0x110000), Err(Error::InvalidCodepoint));
        drop(builder);
        assert_eq!(budget.snapshot().unwrap().used, 0);
    }
    #[test]
    fn ascii_result_owns_charge_and_resource_failure_is_not_unicode_failure() {
        let budget = Budget::new(4).unwrap();
        let output = BudgetAscii::try_from_str(&budget, "utf8").unwrap();
        assert_eq!(output.as_str(), "utf8");
        assert_eq!(output.charged_bytes(), 4);
        assert!(matches!(
            BudgetAscii::try_from_str(&budget, "x"),
            Err(Error::Resource(ResourceError::Exhausted))
        ));
        assert!(matches!(
            BudgetAscii::try_from_str(&budget, "é"),
            Err(Error::NonAscii)
        ));
        let result = BoundedStrictAsciiOutcome::Ascii(output);
        assert_eq!(budget.snapshot().unwrap().used, 4);
        drop(result);
        assert_eq!(budget.snapshot().unwrap().used, 0);
    }
}
