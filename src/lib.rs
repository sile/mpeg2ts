//! MPEG2-TS decoding/encoding library.
//!
//! # References
//!
//! ### Specification
//!
//! - ISO/IEC 13818-1
//! - ITU-T Rec. H.222.0
//!
//! ### Wikipedia
//!
//! - [MPEG transport stream](https://en.wikipedia.org/wiki/MPEG_transport_stream)
//! - [Program-specific information](https://en.wikipedia.org/wiki/Program-specific_information)
//! - [Packetized elementary stream](https://en.wikipedia.org/wiki/Packetized_elementary_stream)
#![warn(missing_docs)]
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

#[allow(missing_docs)] // TODO
pub mod es;
#[allow(missing_docs)] // TODO
pub mod null;
pub mod packet;
#[allow(missing_docs)] // TODO
pub mod pat;
#[allow(missing_docs)] // TODO
pub mod pes;
#[allow(missing_docs)] // TODO
pub mod pmt;
#[allow(missing_docs)] // TODO
pub mod psi;
pub mod time;

mod error;
mod util;

/// This crate specific `Result` type.
pub type Result<T> = std::result::Result<T, Error>;
