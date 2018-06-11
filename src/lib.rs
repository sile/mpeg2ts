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
    };
}

pub mod es;
pub mod pes;
pub mod time;
pub mod ts;

mod crc;
mod error;
mod util;

/// This crate specific `Result` type.
pub type Result<T> = std::result::Result<T, Error>;
