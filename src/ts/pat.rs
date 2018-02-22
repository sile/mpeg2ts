use std::io::Read;
use byteorder::{BigEndian, ReadBytesExt};

use {ErrorKind, Result};
use ts::{Pid, VersionNumber};
use ts::psi::Psi;

/// Payload for PAT(Program Association Table) packets.
#[allow(missing_docs)]
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Pat {
    pub transport_stream_id: u16,
    pub version_number: VersionNumber,
    pub table: Vec<ProgramAssociation>,
}
impl Pat {
    const TABLE_ID: u8 = 0;

    pub(super) fn read_from<R: Read>(reader: R) -> Result<Self> {
        let mut psi = track!(Psi::read_from(reader))?;
        track_assert_eq!(psi.tables.len(), 1, ErrorKind::InvalidInput);

        let table = psi.tables.pop().expect("Never fails");
        let header = table.header;
        track_assert_eq!(header.table_id, Self::TABLE_ID, ErrorKind::InvalidInput);
        track_assert!(!header.private_bit, ErrorKind::InvalidInput);

        let syntax = track_assert_some!(table.syntax.as_ref(), ErrorKind::InvalidInput);
        track_assert_eq!(syntax.section_number, 0, ErrorKind::InvalidInput);
        track_assert_eq!(syntax.last_section_number, 0, ErrorKind::InvalidInput);
        track_assert!(syntax.current_next_indicator, ErrorKind::InvalidInput);

        let mut reader = &syntax.table_data[..];
        let mut table = Vec::new();
        while !reader.is_empty() {
            table.push(track!(ProgramAssociation::read_from(&mut reader))?);
        }
        Ok(Pat {
            transport_stream_id: syntax.table_id_extension,
            version_number: syntax.version_number,
            table,
        })
    }
}

/// An entry of a program association table.
#[allow(missing_docs)]
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct ProgramAssociation {
    pub program_num: u16,

    /// The packet identifier that contains the associated PMT.
    pub program_map_pid: Pid,
}
impl ProgramAssociation {
    fn read_from<R: Read>(mut reader: R) -> Result<Self> {
        let program_num = track_io!(reader.read_u16::<BigEndian>())?;
        let program_map_pid = track!(Pid::read_from(reader))?;
        Ok(ProgramAssociation {
            program_num,
            program_map_pid,
        })
    }
}
