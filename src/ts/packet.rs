use std::io::Read;
use byteorder::{BigEndian, ReadBytesExt};

use {ErrorKind, Result};
use ts::{AdaptationField, ContinuityCounter, Pid, TransportScramblingControl};
use ts::payload::{Bytes, Null, Pat, Pes, Pmt};
use super::adaptation_field::AdaptationFieldControl;

/// Transport stream packet.
#[allow(missing_docs)]
#[derive(Debug, Clone)]
pub struct TsPacket {
    pub header: TsHeader,
    pub adaptation_field: Option<AdaptationField>,
    pub payload: Option<TsPayload>,
}
impl TsPacket {
    /// Size of a packet in bytes.
    pub const SIZE: usize = 188;

    /// Synchronization byte.
    ///
    /// Each packet starts with this byte.
    pub const SYNC_BYTE: u8 = 0x47;
}

/// TS packet header.
#[allow(missing_docs)]
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TsHeader {
    pub transport_error_indicator: bool,
    pub transport_priority: bool,
    pub pid: Pid,
    pub transport_scrambling_control: TransportScramblingControl,
    pub continuity_counter: ContinuityCounter,
}
impl TsHeader {
    pub(super) fn read_from<R: Read>(
        mut reader: R,
    ) -> Result<(Self, AdaptationFieldControl, bool)> {
        let sync_byte = track_io!(reader.read_u8())?;
        track_assert_eq!(sync_byte, TsPacket::SYNC_BYTE, ErrorKind::InvalidInput);

        let n = track_io!(reader.read_u16::<BigEndian>())?;
        let transport_error_indicator = (n & 0b1000_0000_0000_0000) != 0;
        let payload_unit_start_indicator = (n & 0b0100_0000_0000_0000) != 0;
        let transport_priority = (n & 0b0010_0000_0000_0000) != 0;
        let pid = track!(Pid::new(n & 0b0001_1111_1111_1111))?;

        let n = track_io!(reader.read_u8())?;
        let transport_scrambling_control = track!(TransportScramblingControl::from_u8(n >> 6))?;
        let adaptation_field_control = track!(AdaptationFieldControl::from_u8((n >> 4) & 0b11))?;
        let continuity_counter = track!(ContinuityCounter::from_u8(n & 0b1111))?;

        let header = TsHeader {
            transport_error_indicator,
            transport_priority,
            pid,
            transport_scrambling_control,
            continuity_counter,
        };
        Ok((
            header,
            adaptation_field_control,
            payload_unit_start_indicator,
        ))
    }
}

/// TS packet payload.
#[cfg_attr(feature = "cargo-clippy", allow(large_enum_variant))]
#[allow(missing_docs)]
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum TsPayload {
    Pat(Pat),
    Pmt(Pmt),
    Pes(Pes),
    Null(Null),
    Raw(Bytes),
}
