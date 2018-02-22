use std::io::Read;
use byteorder::{BigEndian, ReadBytesExt};

use {ErrorKind, Result};
use packet::Pid;
use psi::Psi;

#[derive(Debug, Clone)]
pub struct Pmt {
    pub program_num: u16,
    pub pcr_pid: u16,
    pub es_info_entries: Vec<EsInfoEntry>,
}
impl Pmt {
    pub fn read_from<R: Read>(reader: R) -> Result<Self> {
        let mut psi = track!(Psi::read_from(reader))?;
        track_assert_eq!(psi.tables.len(), 1, ErrorKind::InvalidInput);

        let table = psi.tables.pop().expect("Never fails");
        track_assert!(!table.header.private_bit, ErrorKind::InvalidInput);

        let syntax = track_assert_some!(table.syntax.as_ref(), ErrorKind::InvalidInput);
        let mut reader = &syntax.table_data[..];

        let n = track_io!(reader.read_u16::<BigEndian>())?;
        track_assert_eq!(
            n & 0b1110_0000_0000_0000,
            0b1110_0000_0000_0000,
            ErrorKind::InvalidInput,
            "Unexpected reserved bits"
        );
        let pcr_pid = n & 0b0001_1111_1111_1111;

        let n = track_io!(reader.read_u16::<BigEndian>())?;
        track_assert_eq!(
            n & 0b1111_0000_0000_0000,
            0b1111_0000_0000_0000,
            ErrorKind::InvalidInput,
            "Unexpected reserved bits"
        );
        track_assert_eq!(
            n & 0b0000_1100_0000_0000,
            0,
            ErrorKind::InvalidInput,
            "Unexpected program info length unused bits"
        );
        let program_info_len = n & 0b0000_0011_1111_1111;
        track_assert_eq!(program_info_len, 0, ErrorKind::Unsupported);

        let mut es_info_entries = Vec::new();
        while !reader.is_empty() {
            es_info_entries.push(track!(EsInfoEntry::read_from(&mut reader))?);
        }
        Ok(Pmt {
            program_num: syntax.table_id_extension,
            pcr_pid,
            es_info_entries,
        })
    }
}

#[derive(Debug, Clone)]
pub struct EsInfoEntry {
    // TODO: enum
    pub stream_type: u8,
    pub elementary_pid: Pid,
    pub descriptors: Vec<Descriptor>,
}
impl EsInfoEntry {
    pub fn read_from<R: Read>(mut reader: R) -> Result<Self> {
        let stream_type = track_io!(reader.read_u8())?;

        let n = track_io!(reader.read_u16::<BigEndian>())?;
        track_assert_eq!(
            n & 0b1110_0000_0000_0000,
            0b1110_0000_0000_0000,
            ErrorKind::InvalidInput,
            "Unexpected reserved bits"
        );
        let elementary_pid = track!(Pid::new(n & 0b0001_1111_1111_1111))?;

        let n = track_io!(reader.read_u16::<BigEndian>())?;
        track_assert_eq!(
            n & 0b1111_0000_0000_0000,
            0b1111_0000_0000_0000,
            ErrorKind::InvalidInput,
            "Unexpected reserved bits"
        );
        track_assert_eq!(
            n & 0b0000_1100_0000_0000,
            0,
            ErrorKind::InvalidInput,
            "Unexpected ES info length unused bits"
        );
        let es_info_len = n & 0b0000_0011_1111_1111;

        let mut reader = reader.take(u64::from(es_info_len));
        let mut descriptors = Vec::new();
        while reader.limit() > 0 {
            let d = track!(Descriptor::read_from(&mut reader))?;
            descriptors.push(d);
        }
        track_assert_eq!(reader.limit(), 0, ErrorKind::InvalidInput);

        Ok(EsInfoEntry {
            stream_type,
            elementary_pid,
            descriptors,
        })
    }
}

#[derive(Debug, Clone)]
pub struct Descriptor {
    // TODO: enum
    pub tag: u8,
    pub data: Vec<u8>,
}
impl Descriptor {
    pub fn read_from<R: Read>(mut reader: R) -> Result<Self> {
        let tag = track_io!(reader.read_u8())?;
        let len = track_io!(reader.read_u8())?;
        let mut data = vec![0; len as usize];
        track_io!(reader.read_exact(&mut data))?;
        Ok(Descriptor { tag, data })
    }
}
