//! Typed data-layer errors.

/// All provider/cache/search failures.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum DataError {
    #[error("http status {0}")]
    Http(u16),
    #[error("parse failed: {0}")]
    Parse(String),
    #[error("rate limited")]
    RateLimited,
    #[error("provider unavailable: {0}")]
    Unavailable(String),
    #[error("empty data: {0}")]
    Empty(String),
    #[error("not supported: {0}")]
    NotSupported(&'static str),
    #[error("stream disconnected")]
    StreamClosed,
}

impl From<std::io::Error> for DataError {
    fn from(e: std::io::Error) -> Self {
        Self::Parse(e.to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn messages_are_lowercase_and_specific() {
        assert_eq!(DataError::Http(429).to_string(), "http status 429");
        assert_eq!(
            DataError::Parse("bad json".into()).to_string(),
            "parse failed: bad json"
        );
        assert_eq!(DataError::RateLimited.to_string(), "rate limited");
        assert_eq!(
            DataError::Unavailable("yahoo".into()).to_string(),
            "provider unavailable: yahoo"
        );
        assert_eq!(
            DataError::Empty("AAPL D".into()).to_string(),
            "empty data: AAPL D"
        );
        assert_eq!(DataError::NotSupported("x").to_string(), "not supported: x");
    }
}
