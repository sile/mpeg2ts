//! Transport stream.
//!
//! # References
//!
//! - [MPEG transport stream](https://en.wikipedia.org/wiki/MPEG_transport_stream)
pub use self::adaptation_field::{AdaptationExtensionField, AdaptationField};
pub use self::packet::{TsHeader, TsPacket, TsPayload};
pub use self::pat::ProgramAssociation;
pub use self::pmt::{Descriptor, EsInfo};
pub use self::reader::TsPacketReader;
pub use self::types::{ContinuityCounter, LegalTimeWindow, Pid, PiecewiseRate, SeamlessSplice,
                      TransportScramblingControl, VersionNumber};

pub mod payload {
    //! Transport stream payloads.

    pub use super::null::Null;
    pub use super::pat::Pat;
    pub use super::pes::Pes;
    pub use super::pmt::Pmt;
    pub use super::types::Bytes;
}

mod adaptation_field;
mod null;
mod packet;
mod pat;
mod pes;
mod pmt;
mod psi;
mod reader;
mod types;
