extern crate byteorder;
#[macro_use]
extern crate trackable;

pub use error::{Error, ErrorKind};

macro_rules! track_io {
    ($expr:expr) => {
        $expr.map_err(|e: ::std::io::Error| {
            use trackable::error::ErrorKindExt;
            track!(::Error::from(::ErrorKind::Other.cause(e)))
        })
    }
}

pub mod packet;
pub mod pat;
pub mod psi;

mod error;
mod util;

/// This crate specific `Result` type.
pub type Result<T> = std::result::Result<T, Error>;
