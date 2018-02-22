use std::io::Read;
use byteorder::{BigEndian, ReadBytesExt};

use {ErrorKind, Result};
use packet::Data;
use time::Timestamp;
use util;

/// Packetized elementary stream.
#[derive(Debug, Clone)]
pub struct Pes {
    pub header: PesHeader,
    pub data: Data,
}
impl Pes {
    pub fn read_from<R: Read>(mut reader: R) -> Result<Self> {
        let header = track!(PesHeader::read_from(&mut reader))?;
        let data = track!(Data::read_from(reader))?;
        Ok(Pes { header, data })
    }
}

#[derive(Debug, Clone)]
pub struct PesHeader {
    pub stream_id: u8,
    pub pes_packet_len: u16, // NOTE: `0` means ...

    // TODO: OptionalPesHeader
    pub scrambling_control: u8,
    pub priority: bool,
    pub data_alignment_indicator: bool,
    pub copyright: bool,
    pub original_or_copy: bool,
    pub pts: Option<Timestamp>,
    pub dts: Option<Timestamp>,
    pub escr: Option<u64>,
    pub es_rate: Option<u32>,
    pub dsm_trick_mode: Option<u8>,
    pub additional_copy_info: Option<u8>,
    pub crc: Option<u16>,
}
impl PesHeader {
    pub fn read_from<R: Read>(mut reader: R) -> Result<Self> {
        let packet_start_code_prefix = track_io!(reader.read_uint::<BigEndian>(3))?;
        track_assert_eq!(packet_start_code_prefix, 0x00_0001, ErrorKind::InvalidInput);

        let stream_id = track_io!(reader.read_u8())?;
        let pes_packet_len = track_io!(reader.read_u16::<BigEndian>())?;

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
            Some(track_io!(reader.read_uint::<BigEndian>(5))?)
        } else {
            None
        };
        let es_rate = if es_rate_flag {
            Some(track_io!(reader.read_uint::<BigEndian>(3))? as u32)
        } else {
            None
        };
        let dsm_trick_mode = if dsm_trick_mode_flag {
            Some(track_io!(reader.read_u8())?)
        } else {
            None
        };
        let additional_copy_info = if additional_copy_info_flag {
            Some(track_io!(reader.read_u8())?)
        } else {
            None
        };
        let crc = if crc_flag {
            Some(track_io!(reader.read_u16::<BigEndian>())?)
        } else {
            None
        };
        track!(util::consume_stuffing_bytes(reader))?;

        Ok(PesHeader {
            stream_id,
            pes_packet_len,
            scrambling_control,
            priority,
            data_alignment_indicator,
            copyright,
            original_or_copy,
            pts,
            dts,
            escr,
            es_rate,
            dsm_trick_mode,
            additional_copy_info,
            crc,
        })
    }
}
