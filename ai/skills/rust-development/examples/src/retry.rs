//! Retry-count parsing and input errors.

use std::error::Error;
use std::fmt;
use std::io::{self, Read};
use std::num::ParseIntError;

/// Parses a nonnegative retry count, ignoring surrounding whitespace.
///
/// # Errors
/// Returns a parse error for invalid numbers or values outside the `u32` range.
///
/// # Examples
///
/// ```
/// use rust_skill_examples::retry::parse_retry_count;
///
/// assert_eq!(parse_retry_count(" 3 ")?, 3);
/// # Ok::<(), std::num::ParseIntError>(())
/// ```
pub fn parse_retry_count(input: &str) -> Result<u32, ParseIntError> {
    input.trim().parse()
}

/// Reading or parsing a retry count failed.
#[derive(Debug)]
pub enum LoadRetryError {
    /// The input could not be read as UTF-8 text.
    Read(io::Error),
    /// The text was not a supported retry count.
    Invalid(ParseIntError),
}

impl fmt::Display for LoadRetryError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Read(_) => formatter.write_str("could not read retry count"),
            Self::Invalid(_) => formatter.write_str("invalid retry count"),
        }
    }
}

impl Error for LoadRetryError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Read(error) => Some(error),
            Self::Invalid(error) => Some(error),
        }
    }
}

/// Reads a retry count from a small, trusted configuration input.
///
/// Reads until end of input. This function does not limit the input size.
///
/// # Errors
/// Returns [`LoadRetryError::Read`] for read or UTF-8 failures and
/// [`LoadRetryError::Invalid`] for invalid counts.
pub fn read_retry_count(input: &mut impl Read) -> Result<u32, LoadRetryError> {
    let mut text = String::new();
    input
        .read_to_string(&mut text)
        .map_err(LoadRetryError::Read)?;
    parse_retry_count(&text).map_err(LoadRetryError::Invalid)
}

#[cfg(test)]
mod tests {
    use super::{parse_retry_count, read_retry_count, LoadRetryError};
    use std::error::Error;

    #[test]
    fn parses_retry_counts() {
        let cases = [
            ("zero", "0", 0),
            ("ordinary count", "3", 3),
            ("surrounding whitespace", " 3 ", 3),
            ("maximum count", "4294967295", u32::MAX),
        ];

        for (case_name, input, expected) in cases {
            assert_eq!(parse_retry_count(input), Ok(expected), "{case_name}");
        }
    }

    #[test]
    fn rejects_invalid_retry_counts() {
        let cases = [
            ("negative", "-1"),
            ("missing", ""),
            ("not a number", "three"),
            ("too large", "4294967296"),
        ];

        for (case_name, input) in cases {
            assert!(parse_retry_count(input).is_err(), "{case_name}");
        }
    }

    #[test]
    fn reads_count_from_bytes() {
        let mut input: &[u8] = b" 3 \n";

        let count = read_retry_count(&mut input).expect("input contains a valid count");

        assert_eq!(count, 3);
    }

    #[test]
    fn preserves_parse_error_cause() {
        let mut input: &[u8] = b"three";

        let error = read_retry_count(&mut input).expect_err("input is not a number");

        assert!(matches!(&error, LoadRetryError::Invalid(_)));
        assert!(error.source().is_some());
    }

    #[test]
    fn classifies_invalid_utf8_as_read_failure() {
        let mut input: &[u8] = &[0xff];

        let error = read_retry_count(&mut input).expect_err("input is not UTF-8");

        assert!(matches!(error, LoadRetryError::Read(_)));
    }
}
