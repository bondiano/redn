//! Ordered floating-point wrapper for EDN values.

use std::cmp::Ordering;
use std::fmt;
use std::hash::{Hash, Hasher};

/// An `f64` wrapper implementing total [`Eq`], [`Ord`], and [`Hash`].
///
/// Equality, ordering, and hashing canonicalize `-0.0` to `+0.0` and treat all
/// `NaN` bit patterns as equal and greater than positive infinity, while
/// preserving the original `f64` value for formatting and retrieval.
#[derive(Debug, Clone, Copy)]
#[repr(transparent)]
pub struct Float(f64);

impl Float {
    /// Wraps a raw `f64`.
    pub const fn new(value: f64) -> Self {
        Self(value)
    }

    /// Returns the underlying `f64`.
    pub const fn into_inner(self) -> f64 {
        self.0
    }

    fn canonical_f64(self) -> f64 {
        if self.0 == 0.0 {
            0.0
        } else if self.0.is_nan() {
            f64::NAN
        } else {
            self.0
        }
    }

    fn canonical_bits(self) -> u64 {
        self.canonical_f64().to_bits()
    }
}

impl From<f64> for Float {
    fn from(value: f64) -> Self {
        Self(value)
    }
}

impl From<Float> for f64 {
    fn from(value: Float) -> Self {
        value.0
    }
}

impl PartialEq for Float {
    fn eq(&self, other: &Self) -> bool {
        self.cmp(other) == Ordering::Equal
    }
}

impl Eq for Float {}

impl PartialOrd for Float {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for Float {
    fn cmp(&self, other: &Self) -> Ordering {
        self.canonical_f64().total_cmp(&other.canonical_f64())
    }
}

impl Hash for Float {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.canonical_bits().hash(state);
    }
}

impl fmt::Display for Float {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.0.is_finite() {
            let rendered = format!("{:?}", self.0);
            if rendered.contains(['.', 'e', 'E']) {
                f.write_str(&rendered)
            } else {
                write!(f, "{rendered}.0")
            }
        } else {
            write!(f, "{}", self.0)
        }
    }
}
