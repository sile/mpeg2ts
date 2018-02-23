use std::io::Read;
use byteorder::{BigEndian, ReadBytesExt};

use {ErrorKind, Result};
use ts::VersionNumber;
use util::{self, WithCrc32};

/// Program-specific information.
#[derive(Debug)]
pub struct Psi {
    pub tables: Vec<PsiTable>,
}
impl Psi {
    pub fn read_from<R: Read>(mut reader: R) -> Result<Self> {
        let pointer_field = track_io!(reader.read_u8())?;
        track_assert_eq!(pointer_field, 0, ErrorKind::Unsupported);

        let mut tables = Vec::new();
        loop {
            let mut peek = [0];
            let eos = track_io!(reader.read(&mut peek))? == 0;
            if eos {
                break;
            }
            if !tables.is_empty() && peek[0] == 0xFF {
                track!(util::consume_stuffing_bytes(&mut reader))?;
                break;
            }
            let table = track!(PsiTable::read_from(peek.chain(&mut reader)))?;
            tables.push(table);
        }
        Ok(Psi { tables })
    }
}

#[derive(Debug)]
pub struct PsiTable {
    pub header: PsiTableHeader,
    pub syntax: Option<PsiTableSyntax>,
}
impl PsiTable {
    pub fn read_from<R: Read>(reader: R) -> Result<Self> {
        let mut reader = WithCrc32::new(reader);
        let (header, syntax_section_len) = track!(PsiTableHeader::read_from(&mut reader))?;
        let syntax = if syntax_section_len > 0 {
            let syntax = {
                track_assert!(syntax_section_len >= 4, ErrorKind::InvalidInput);
                let reader = reader.by_ref().take(u64::from(syntax_section_len - 4));
                track!(PsiTableSyntax::read_from(reader))?
            };
            let crc32 = reader.crc32();
            let expected_crc32 = track_io!(reader.read_u32::<BigEndian>())?;
            track_assert_eq!(crc32, expected_crc32, ErrorKind::InvalidInput);
            Some(syntax)
        } else {
            None
        };
        Ok(PsiTable { header, syntax })
    }
}

#[derive(Debug)]
pub struct PsiTableHeader {
    pub table_id: u8,
    pub private_bit: bool,
}
impl PsiTableHeader {
    pub fn read_from<R: Read>(mut reader: R) -> Result<(Self, u16)> {
        let table_id = track_io!(reader.read_u8())?;

        let n = track_io!(reader.read_u16::<BigEndian>())?;
        let syntax_section_indicator = (n & 0b1000_0000_0000_0000) != 0;
        let private_bit = (n & 0b0100_0000_0000_0000) != 0;
        track_assert_eq!(
            n & 0b0011_0000_0000_0000,
            0b0011_0000_0000_0000,
            ErrorKind::InvalidInput,
            "Unexpected reserved bits"
        );
        track_assert_eq!(
            n & 0b0000_1100_0000_0000,
            0,
            ErrorKind::InvalidInput,
            "Unexpected section length unused bits"
        );
        let syntax_section_len = n & 0b0000_0011_1111_1111;
        track_assert!(syntax_section_len <= 1021, ErrorKind::InvalidInput);
        if syntax_section_indicator {
            track_assert_ne!(syntax_section_len, 0, ErrorKind::InvalidInput);
        }

        let header = PsiTableHeader {
            table_id,
            private_bit,
        };
        Ok((header, syntax_section_len))
    }
}

#[derive(Debug)]
pub struct PsiTableSyntax {
    pub table_id_extension: u16,
    pub version_number: VersionNumber,
    pub current_next_indicator: bool,
    pub section_number: u8,
    pub last_section_number: u8,
    pub table_data: Vec<u8>,
}
impl PsiTableSyntax {
    pub fn read_from<R: Read>(mut reader: R) -> Result<Self> {
        let table_id_extension = track_io!(reader.read_u16::<BigEndian>())?;

        let b = track_io!(reader.read_u8())?;
        track_assert_eq!(
            b & 0b1100_0000,
            0b1100_0000,
            ErrorKind::InvalidInput,
            "Unexpected reserved bits"
        );
        let version_number = track!(VersionNumber::from_u8((b & 0b0011_1110) >> 1))?;
        let current_next_indicator = (b & 0b0000_0001) != 0;

        let section_number = track_io!(reader.read_u8())?;
        let last_section_number = track_io!(reader.read_u8())?;

        let mut table_data = Vec::new();
        track_io!(reader.read_to_end(&mut table_data))?;

        Ok(PsiTableSyntax {
            table_id_extension,
            version_number,
            current_next_indicator,
            section_number,
            last_section_number,
            table_data,
        })
    }
}
