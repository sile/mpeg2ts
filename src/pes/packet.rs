use byteorder::{BigEndian, ReadBytesExt, WriteBytesExt};
use std::io::{Read, Write};

use es::StreamId;
use time::{ClockReference, Timestamp};
use util;
use {ErrorKind, Result};

const PACKET_START_CODE_PREFIX: u64 = 0x00_0001;

/// PES packet.
#[allow(missing_docs)]
#[derive(Debug, Clone)]
pub struct PesPacket<B> {
    pub header: PesHeader,
    pub data: B,
}

/// PES packet header.
///
/// Note that `PesHeader` contains the fields that belong to the optional PES header.
#[allow(missing_docs)]
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct PesHeader {
    pub stream_id: StreamId,
    pub priority: bool,

    /// `true` indicates that the PES packet header is immediately followed by
    /// the video start code or audio syncword.
    pub data_alignment_indicator: bool,

    /// `true` implies copyrighted.
    pub copyright: bool,

    /// `true` implies original.
    pub original_or_copy: bool,

    pub pts: Option<Timestamp>,
    pub dts: Option<Timestamp>,

    /// Elementary stream clock reference.
    pub escr: Option<ClockReference>,
}
impl PesHeader {
    pub(super) fn optional_header_len(&self) -> u16 {
        3 + self.pts.map_or(0, |_| 5) + self.dts.map_or(0, |_| 5) + self.escr.map_or(0, |_| 6)
    }

    pub(crate) fn read_from<R: Read>(mut reader: R) -> Result<(Self, u16)> {
        let packet_start_code_prefix = track_io!(reader.read_uint::<BigEndian>(3))?;
        track_assert_eq!(
            packet_start_code_prefix,
            PACKET_START_CODE_PREFIX,
            ErrorKind::InvalidInput
        );

        let stream_id = StreamId::new(track_io!(reader.read_u8())?);
        let packet_len = track_io!(reader.read_u16::<BigEndian>())?;

        let b = track_io!(reader.read_u8())?;
        track_assert_eq!(
            b & 0b1100_0000,
            0b1000_0000,
            ErrorKind::InvalidInput,
            "Unexpected marker bits"
        );
        let scrambling_control = (b & 0b0011_0000) >> 4;
        let priority = (b & 0b0000_1000) != 0;
        let data_alignment_indicator = (b & 0b0000_0100) != 0;
        let copyright = (b & 0b0000_0010) != 0;
        let original_or_copy = (b & 0b0000_0001) != 0;
        track_assert_eq!(scrambling_control, 0, ErrorKind::Unsupported);

        let b = track_io!(reader.read_u8())?;
        let pts_flag = (b & 0b1000_0000) != 0;
        let dts_flag = (b & 0b0100_0000) != 0;
        track_assert_ne!((pts_flag, dts_flag), (false, true), ErrorKind::InvalidInput);

        let escr_flag = (b & 0b0010_0000) != 0;
        let es_rate_flag = (b & 0b0001_0000) != 0;
        let dsm_trick_mode_flag = (b & 0b0000_1000) != 0;
        let additional_copy_info_flag = (b & 0b0000_0100) != 0;
        let crc_flag = (b & 0b0000_0010) != 0;
        let extension_flag = (b & 0b0000_0001) != 0;
        track_assert!(!es_rate_flag, ErrorKind::Unsupported);
        track_assert!(!dsm_trick_mode_flag, ErrorKind::Unsupported);
        track_assert!(!additional_copy_info_flag, ErrorKind::Unsupported);
        track_assert!(!crc_flag, ErrorKind::Unsupported);
        track_assert!(!extension_flag, ErrorKind::Unsupported);

        let pes_header_len = track_io!(reader.read_u8())?;

        let mut reader = reader.take(u64::from(pes_header_len));
        let pts = if pts_flag {
            let check_bits = if dts_flag { 3 } else { 2 };
            Some(track!(Timestamp::read_from(&mut reader, check_bits))?)
        } else {
            None
        };
        let dts = if dts_flag {
            let check_bits = 1;
            Some(track!(Timestamp::read_from(&mut reader, check_bits))?)
        } else {
            None
        };
        let escr = if escr_flag {
            Some(track!(ClockReference::read_escr_from(&mut reader))?)
        } else {
            None
        };
        track!(util::consume_stuffing_bytes(reader))?;

        let header = PesHeader {
            stream_id,
            priority,
            data_alignment_indicator,
            copyright,
            original_or_copy,
            pts,
            dts,
            escr,
        };
        Ok((header, packet_len))
    }

    pub(crate) fn write_to<W: Write>(&self, mut writer: W, pes_header_len: u16) -> Result<()> {
        track_io!(writer.write_uint::<BigEndian>(PACKET_START_CODE_PREFIX, 3))?;
        track_io!(writer.write_u8(self.stream_id.as_u8()))?;
        track_io!(writer.write_u16::<BigEndian>(pes_header_len))?;

        let n = 0b1000_0000
            | ((self.priority as u8) << 3)
            | ((self.data_alignment_indicator as u8) << 2)
            | ((self.copyright as u8) << 1)
            | self.original_or_copy as u8;
        track_io!(writer.write_u8(n))?;

        if self.dts.is_some() {
            track_assert!(self.pts.is_some(), ErrorKind::InvalidInput);
        }
        let n = ((self.pts.is_some() as u8) << 7)
            | ((self.dts.is_some() as u8) << 6)
            | ((self.escr.is_some() as u8) << 5);
        track_io!(writer.write_u8(n))?;

        let pes_header_len = self.optional_header_len() as u8 - 3;
        track_io!(writer.write_u8(pes_header_len))?;
        if let Some(x) = self.pts {
            let check_bits = if self.dts.is_some() { 3 } else { 2 };
            track!(x.write_to(&mut writer, check_bits))?;
        }
        if let Some(x) = self.dts {
            let check_bits = 1;
            track!(x.write_to(&mut writer, check_bits))?;
        }
        if let Some(x) = self.escr {
            track!(x.write_escr_to(&mut writer))?;
        }

        Ok(())
    }
}
