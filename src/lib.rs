extern crate byteorder;
extern crate num_rational;
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

pub mod es;
pub mod null;
pub mod packet;
pub mod pat;
pub mod pes;
pub mod pmt;
pub mod psi;
pub mod time;

mod error;
mod util;

/// This crate specific `Result` type.
pub type Result<T> = std::result::Result<T, Error>;
