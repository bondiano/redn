//! Generic wrapper for tagged and untagged EDN elements.

/// An EDN value optionally wrapped with a `#namespace/name` or `#name` tag.
///
/// `Tagged` is declared before `NoTag`, so derived [`Ord`] orders tagged values
/// ahead of untagged ones in maps and sets.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Tagged<T, A> {
    /// A tagged element such as `#prefix/tag value` (empty namespace for `#tag value`).
    Tagged {
        /// Tag namespace, or empty when the tag has no namespace prefix.
        namespace: T,
        /// Tag name.
        name: T,
        /// Underlying value.
        value: A,
    },
    /// An untagged element.
    NoTag(A),
}

impl<T, A> Tagged<T, A> {
    /// Constructs a tagged element with the given namespace, tag name, and value.
    pub fn new(namespace: impl Into<T>, name: impl Into<T>, value: A) -> Self {
        Self::Tagged {
            namespace: namespace.into(),
            name: name.into(),
            value,
        }
    }

    /// Constructs an untagged element.
    pub const fn untagged(value: A) -> Self {
        Self::NoTag(value)
    }

    /// Borrows the inner value, ignoring any tag.
    pub const fn value(&self) -> &A {
        match self {
            Self::Tagged { value, .. } | Self::NoTag(value) => value,
        }
    }

    /// Mutably borrows the inner value, ignoring any tag.
    pub const fn value_mut(&mut self) -> &mut A {
        match self {
            Self::Tagged { value, .. } | Self::NoTag(value) => value,
        }
    }

    /// Unwraps the inner value, discarding any tag.
    pub fn into_value(self) -> A {
        match self {
            Self::Tagged { value, .. } | Self::NoTag(value) => value,
        }
    }

    /// Returns `(namespace, name)` if this element is tagged, or `None` if untagged.
    pub const fn tag(&self) -> Option<(&T, &T)> {
        match self {
            Self::Tagged {
                namespace, name, ..
            } => Some((namespace, name)),
            Self::NoTag(_) => None,
        }
    }

    /// Applies `f` to the inner value while preserving any tag (`Functor` map).
    pub fn map<B>(self, f: impl FnOnce(A) -> B) -> Tagged<T, B> {
        match self {
            Self::Tagged {
                namespace,
                name,
                value,
            } => Tagged::Tagged {
                namespace,
                name,
                value: f(value),
            },
            Self::NoTag(value) => Tagged::NoTag(f(value)),
        }
    }
}

/// Unwraps the inner value from a [`Tagged`] wrapper.
pub fn strip_tag<T, A>(tagged: Tagged<T, A>) -> A {
    tagged.into_value()
}
