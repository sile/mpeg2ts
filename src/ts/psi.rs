use std::io::{Read, Write};
use byteorder::{BigEndian, ReadBytesExt, WriteBytesExt};

use {ErrorKind, Result};
use ts::VersionNumber;
use util::{self, WithCrc32};

const MAX_SYNTAX_SECTION_LEN: usize = 1021;

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

    pub fn write_to<W: Write>(&self, mut writer: W) -> Result<()> {
        track_io!(writer.write_u8(0))?; // pointer field
        for table in &self.tables {
            track!(table.write_to(&mut writer))?;
        }
        Ok(())
    }
}

#[derive(Debug)]
pub struct PsiTable {
    pub header: PsiTableHeader,
    pub syntax: Option<PsiTableSyntax>,
}
impl PsiTable {
    fn read_from<R: Read>(reader: R) -> Result<Self> {
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

    fn write_to<W: Write>(&self, writer: W) -> Result<()> {
        let mut writer = WithCrc32::new(writer);

        let syntax_section_len = self.syntax.as_ref().map_or(0, |s| s.external_size());
        track!(self.header.write_to(&mut writer, syntax_section_len))?;
        if let Some(ref x) = self.syntax {
            track!(x.write_to(&mut writer))?;

            let crc32 = writer.crc32();
            track_io!(writer.write_u32::<BigEndian>(crc32))?;
        }
        Ok(())
    }
}

#[derive(Debug)]
pub struct PsiTableHeader {
    pub table_id: u8,
    pub private_bit: bool,
}
impl PsiTableHeader {
    fn read_from<R: Read>(mut reader: R) -> Result<(Self, u16)> {
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
        track_assert!(
            (syntax_section_len as usize) <= MAX_SYNTAX_SECTION_LEN,
            ErrorKind::InvalidInput
        );
        if syntax_section_indicator {
            track_assert_ne!(syntax_section_len, 0, ErrorKind::InvalidInput);
        }

        let header = PsiTableHeader {
            table_id,
            private_bit,
        };
        Ok((header, syntax_section_len))
    }

    fn write_to<W: Write>(&self, mut writer: W, syntax_section_len: usize) -> Result<()> {
        track_assert!(
            syntax_section_len <= MAX_SYNTAX_SECTION_LEN,
            ErrorKind::InvalidInput
        );

        track_io!(writer.write_u8(self.table_id))?;

        let n = (((syntax_section_len != 0) as u16) << 15) | ((self.private_bit as u16) << 14)
            | 0b0011_0000_0000_0000 | syntax_section_len as u16;
        track_io!(writer.write_u16::<BigEndian>(n))?;

        Ok(())
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
    fn external_size(&self) -> usize {
        2 /* table_id_extension */ +
            1 /* version_number and current_next_indicator */ +
            1 /* section_number */ +
            1 /* last_section_number */ +
            self.table_data.len() /* table_data */ +
            4 /* CRC32 */
    }

    fn read_from<R: Read>(mut reader: R) -> Result<Self> {
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

    fn write_to<W: Write>(&self, mut writer: W) -> Result<()> {
        track_io!(writer.write_u16::<BigEndian>(self.table_id_extension))?;

        let n =
            0b1100_0000 | (self.version_number.as_u8() << 1) | self.current_next_indicator as u8;
        track_io!(writer.write_u8(n))?;

        track_io!(writer.write_u8(self.section_number))?;
        track_io!(writer.write_u8(self.last_section_number))?;
        track_io!(writer.write_all(&self.table_data))?;

        Ok(())
    }
}
