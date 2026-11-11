//! Holds the handoff point for a session's context, and how a token
//! count prints.

/// The context, in tokens, at which a session should hand off.
pub(crate) const HANDOFF_TOKENS: u64 = 150_000;

/// Shows tokens in thousands, such as `98k`, or `-` when unknown.
pub(crate) fn format_tokens(tokens: Option<u64>) -> String {
    match tokens {
        Some(tokens) => format!("{}k", tokens / 1000),
        None => "-".to_owned(),
    }
}
