use trackable::error::{ErrorKind as TrackableErrorKind, TrackableError};

/// This crate specific `Error` type.
#[derive(Debug, Clone, trackable::TrackableError)]
pub struct Error(TrackableError<ErrorKind>);

/// Possible error kinds.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[allow(missing_docs)]
pub enum ErrorKind {
    InvalidInput,
    Unsupported,
    Other,
}
impl TrackableErrorKind for ErrorKind {}
