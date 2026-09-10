//! Distinct identifiers and checked construction.

use std::error::Error;
use std::fmt;
use std::num::NonZeroU64;

/// An application's nonzero user identifier.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct UserId(NonZeroU64);

/// A user identifier was zero.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct InvalidUserId;

impl fmt::Display for InvalidUserId {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("user ID must not be zero")
    }
}

impl Error for InvalidUserId {}

impl TryFrom<u64> for UserId {
    type Error = InvalidUserId;

    /// Converts a raw identifier into a user identifier.
    ///
    /// # Errors
    /// Returns [`InvalidUserId`] when `value` is zero.
    fn try_from(value: u64) -> Result<Self, Self::Error> {
        let Some(value) = NonZeroU64::new(value) else {
            return Err(InvalidUserId);
        };
        Ok(Self(value))
    }
}

impl From<UserId> for u64 {
    fn from(user_id: UserId) -> Self {
        user_id.0.get()
    }
}

#[cfg(test)]
mod tests {
    use super::{InvalidUserId, UserId};

    #[test]
    fn rejects_zero_user_id() {
        assert_eq!(UserId::try_from(0), Err(InvalidUserId));
    }

    #[test]
    fn preserves_nonzero_user_id() {
        let user_id = UserId::try_from(42).expect("42 is a valid nonzero ID");

        assert_eq!(u64::from(user_id), 42);
    }
}
