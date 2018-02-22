use std::io::Read;
use byteorder::{BigEndian, ReadBytesExt};

use {ErrorKind, Result};
use packet::Pid;
use psi::Psi;

#[derive(Debug, Clone)]
pub struct PatEntry {
    pub program_num: u16,
    pub program_map_pid: Pid,
}
impl PatEntry {
    pub fn read_from<R: Read>(mut reader: R) -> Result<Self> {
        let program_num = track_io!(reader.read_u16::<BigEndian>())?;

        let n = track_io!(reader.read_u16::<BigEndian>())?;
        track_assert_eq!(
            n & 0b1110_0000_0000_0000,
            0b1110_0000_0000_0000,
            ErrorKind::InvalidInput,
            "Unexpected reserved bits"
        );
        let program_map_pid = track!(Pid::new(n & 0b0001_1111_1111_1111))?;
        Ok(PatEntry {
            program_num,
            program_map_pid,
        })
    }
}

#[derive(Debug, Clone)]
pub struct Pat {
    pub transport_stream_id: u16,
    pub entries: Vec<PatEntry>,
}
impl Pat {
    pub const TABLE_ID: u8 = 0;

    pub fn read_from<R: Read>(reader: R) -> Result<Self> {
        let mut psi = track!(Psi::read_from(reader))?;
        track_assert_eq!(psi.tables.len(), 1, ErrorKind::InvalidInput);

        let table = psi.tables.pop().expect("Never fails");
        track_assert_eq!(
            table.header.table_id,
            Self::TABLE_ID,
            ErrorKind::InvalidInput
        );
        track_assert!(!table.header.private_bit, ErrorKind::InvalidInput);

        let syntax = track_assert_some!(table.syntax.as_ref(), ErrorKind::InvalidInput);

        let mut reader = &syntax.table_data[..];
        let mut entries = Vec::new();
        while !reader.is_empty() {
            entries.push(track!(PatEntry::read_from(&mut reader))?);
        }
        Ok(Pat {
            transport_stream_id: syntax.table_id_extension,
            entries,
        })
    }
}
