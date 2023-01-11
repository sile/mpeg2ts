use byteorder::{BigEndian, ReadBytesExt, WriteBytesExt};
use std::io::{Cursor, Read, Write};

use super::adaptation_field::AdaptationFieldControl;
use ts::payload::{Bytes, Null, Pat, Pes, Pmt};
use ts::{AdaptationField, ContinuityCounter, Pid, TransportScramblingControl};
use {ErrorKind, Result};

/// Transport stream packet.
#[allow(missing_docs)]
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
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

    pub(super) fn write_to<W: Write>(&self, mut writer: W) -> Result<()> {
        let mut payload_buf = [0; TsPacket::SIZE - 4];
        let payload_len = if let Some(ref payload) = self.payload {
            let mut writer = Cursor::new(&mut payload_buf[..]);
            track!(payload.write_to(&mut writer))?;
            writer.position() as usize
        } else {
            0
        };

        let required_len = self
            .adaptation_field
            .as_ref()
            .map_or(0, |a| a.external_size());
        let free_len = TsPacket::SIZE - 4 - payload_len;
        track_assert!(
            required_len <= free_len,
            ErrorKind::InvalidInput,
            "No space for adaptation field: required={}, free={}",
            required_len,
            free_len,
        );

        let adaptation_field_control = match (
            self.adaptation_field.is_some() || free_len > 0,
            self.payload.is_some(),
        ) {
            (true, true) => AdaptationFieldControl::AdaptationFieldAndPayload,
            (true, false) => AdaptationFieldControl::AdaptationFieldOnly,
            (false, true) => AdaptationFieldControl::PayloadOnly,
            (false, false) => track_panic!(ErrorKind::InvalidInput, "Reserved for future use"),
        };
        let payload_unit_start_indicator = match self.payload {
            Some(TsPayload::Raw(_)) | Some(TsPayload::Null(_)) | None => false,
            _ => true,
        };
        track!(self.header.write_to(
            &mut writer,
            adaptation_field_control,
            payload_unit_start_indicator
        ))?;

        if let Some(ref adaptation_field) = self.adaptation_field {
            let adaptation_field_len = (free_len - 1) as u8;
            track!(adaptation_field.write_to(&mut writer, adaptation_field_len))?;
        } else if free_len > 0 {
            let adaptation_field_len = (free_len - 1) as u8;
            track!(AdaptationField::write_stuffing_bytes(
                &mut writer,
                adaptation_field_len
            ))?;
        }
        track_io!(writer.write_all(&payload_buf[..payload_len]))?;
        Ok(())
    }
}

/// TS packet header.
#[allow(missing_docs)]
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
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

    fn write_to<W: Write>(
        &self,
        mut writer: W,
        adaptation_field_control: AdaptationFieldControl,
        payload_unit_start_indicator: bool,
    ) -> Result<()> {
        track_io!(writer.write_u8(TsPacket::SYNC_BYTE))?;

        let n = ((self.transport_error_indicator as u16) << 15)
            | ((payload_unit_start_indicator as u16) << 14)
            | ((self.transport_priority as u16) << 13)
            | self.pid.as_u16();
        track_io!(writer.write_u16::<BigEndian>(n))?;

        let n = ((self.transport_scrambling_control as u8) << 6)
            | ((adaptation_field_control as u8) << 4)
            | self.continuity_counter.as_u8();
        track_io!(writer.write_u8(n))?;

        Ok(())
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
impl TsPayload {
    fn write_to<W: Write>(&self, writer: W) -> Result<()> {
        match *self {
            TsPayload::Pat(ref x) => track!(x.write_to(writer)),
            TsPayload::Pmt(ref x) => track!(x.write_to(writer)),
            TsPayload::Pes(ref x) => track!(x.write_to(writer)),
            TsPayload::Null(_) => Ok(()),
            TsPayload::Raw(ref x) => track!(x.write_to(writer)),
        }
    }
}
