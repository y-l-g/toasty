use super::Error;

/// Error when a uniqueness constraint is violated.
///
/// Drivers should classify duplicate-key failures here:
/// SQLite `SQLITE_CONSTRAINT_UNIQUE` / `SQLITE_CONSTRAINT_PRIMARYKEY`,
/// PostgreSQL SQLSTATE `23505`, MySQL errors `1022`, `1062`, `1169`,
/// `1586`, `1859`, and equivalents on other backends. This covers
/// single-field `#[unique]`, composite unique indices, and primary-key
/// conflicts, without attributing which fields conflicted.
#[derive(Debug)]
pub(super) struct UniqueViolation {
    message: Box<str>,
}

impl std::error::Error for UniqueViolation {}

impl core::fmt::Display for UniqueViolation {
    fn fmt(&self, f: &mut core::fmt::Formatter) -> core::fmt::Result {
        write!(f, "unique violation: {}", self.message)
    }
}

impl Error {
    /// Creates a unique violation error.
    ///
    /// Returned when the database rejects a write because it would duplicate
    /// a unique index or primary key.
    ///
    /// # Examples
    ///
    /// ```
    /// use toasty_core::Error;
    ///
    /// let err = Error::unique_violation("UNIQUE constraint failed: users.email");
    /// assert!(err.is_unique_violation());
    /// ```
    pub fn unique_violation(message: impl Into<String>) -> Error {
        Error::from(super::ErrorKind::UniqueViolation(UniqueViolation {
            message: message.into().into(),
        }))
    }

    /// Returns `true` if this error is a unique violation.
    pub fn is_unique_violation(&self) -> bool {
        matches!(self.kind(), super::ErrorKind::UniqueViolation(_))
    }
}
