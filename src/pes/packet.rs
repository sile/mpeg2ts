use std::io::Read;
use byteorder::{BigEndian, ReadBytesExt};

use {ErrorKind, Result};
use es::StreamId;
use time::{ClockReference, Timestamp};
use util;

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
        track_assert_eq!(packet_start_code_prefix, 0x00_0001, ErrorKind::InvalidInput);

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
}
