use std::io::Read;
use byteorder::{BigEndian, ReadBytesExt};

use {ErrorKind, Result};
use es::StreamId;
use packet::{Bytes, TransportScramblingControl};
use time::{ClockReference, Timestamp};
use util;

/// Payload for PES(Packetized elementary stream) packets.
#[allow(missing_docs)]
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Pes {
    pub header: PesHeader,
    pub data: Bytes,
}
impl Pes {
    pub(super) fn read_from<R: Read>(mut reader: R) -> Result<Self> {
        let header = track!(PesHeader::read_from(&mut reader))?;
        let data = track!(Bytes::read_from(reader))?;
        Ok(Pes { header, data })
    }
}

/// Header of `Pes`.
///
/// Note that `PesHeader` contains the fields that belong to the optional PES header.
#[allow(missing_docs)]
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct PesHeader {
    pub stream_id: StreamId,

    /// The number of bytes remaining in the packet after this field.
    ///
    /// Can be zero. If the PES packet length is set to zero, the PES packet can be of any length.
    /// A value of zero for the PES packet length can be used only when
    /// the PES packet payload is a video elementary stream.
    pub packet_len: u16,

    pub scrambling_control: TransportScramblingControl,
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

    /// Elementary stream clcok reference.
    pub escr: Option<ClockReference>,
}
impl PesHeader {
    fn read_from<R: Read>(mut reader: R) -> Result<Self> {
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
        let scrambling_control =
            track!(TransportScramblingControl::from_u8((b & 0b0011_0000) >> 4))?;
        let priority = (b & 0b0000_1000) != 0;
        let data_alignment_indicator = (b & 0b0000_0100) != 0;
        let copyright = (b & 0b0000_0010) != 0;
        let original_or_copy = (b & 0b0000_0001) != 0;

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

        Ok(PesHeader {
            stream_id,
            packet_len,
            scrambling_control,
            priority,
            data_alignment_indicator,
            copyright,
            original_or_copy,
            pts,
            dts,
            escr,
        })
    }
}
