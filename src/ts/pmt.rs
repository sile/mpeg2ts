use crate::es::StreamType;
use crate::ts::psi::{Psi, PsiTable, PsiTableHeader, PsiTableSyntax};
use crate::ts::{Pid, VersionNumber};
use crate::{ErrorKind, Result};
use byteorder::{BigEndian, ReadBytesExt, WriteBytesExt};
use std::io::{Read, Write};

/// Program Map Table.
#[allow(missing_docs)]
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Pmt {
    pub program_num: u16,

    /// The packet identifier that contains the program clock reference (PCR).
    ///
    /// The PCR is used to improve the random access accuracy of the stream's timing
    /// that is derived from the program timestamp.
    pub pcr_pid: Option<Pid>,

    pub version_number: VersionNumber,
    pub program_info: Vec<Descriptor>,
    pub es_info: Vec<EsInfo>,
}
impl Pmt {
    const TABLE_ID: u8 = 2;

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

        let pcr_pid = track!(Pid::read_from(&mut reader))?;
        let pcr_pid = if pcr_pid.as_u16() == 0b0001_1111_1111_1111 {
            None
        } else {
            Some(pcr_pid)
        };

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
        let mut program_info = Vec::new();
        let (mut program_info_reader, mut reader) = reader.split_at(program_info_len as usize);
        while !program_info_reader.is_empty() {
            program_info.push(track!(Descriptor::read_from(&mut program_info_reader))?);
        }

        let mut es_info = Vec::new();
        while !reader.is_empty() {
            es_info.push(track!(EsInfo::read_from(&mut reader))?);
        }
        Ok(Pmt {
            program_num: syntax.table_id_extension,
            pcr_pid,
            version_number: syntax.version_number,
            program_info,
            es_info,
        })
    }

    pub(super) fn write_to<W: Write>(&self, writer: W) -> Result<()> {
        track!(self.to_psi().and_then(|psi| psi.write_to(writer)))
    }

    fn to_psi(&self) -> Result<Psi> {
        let mut table_data = Vec::new();
        if let Some(pid) = self.pcr_pid {
            track_assert_ne!(pid.as_u16(), 0b0001_1111_1111_1111, ErrorKind::InvalidInput);
            track!(pid.write_to(&mut table_data))?;
        } else {
            track_io!(table_data.write_u16::<BigEndian>(0xFFFF))?;
        }

        let program_info_len: usize = self
            .program_info
            .iter()
            .map(|desc| desc.data.len() + 2)
            .sum();
        track_assert!(
            program_info_len <= 0b0000_0011_1111_1111,
            ErrorKind::InvalidInput,
            "program info length too large"
        );
        let n = 0b1111_0000_0000_0000 | program_info_len as u16;
        track_io!(table_data.write_u16::<BigEndian>(n))?;

        for desc in &self.program_info {
            track!(desc.write_to(&mut table_data))?;
        }

        for info in &self.es_info {
            track!(info.write_to(&mut table_data))?;
        }

        let header = PsiTableHeader {
            table_id: Self::TABLE_ID,
            private_bit: false,
        };
        let syntax = Some(PsiTableSyntax {
            table_id_extension: self.program_num,
            version_number: self.version_number,
            current_next_indicator: true,
            section_number: 0,
            last_section_number: 0,
            table_data,
        });
        let tables = vec![PsiTable { header, syntax }];
        Ok(Psi { tables })
    }
}

/// Elementary stream information.
#[allow(missing_docs)]
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct EsInfo {
    pub stream_type: StreamType,

    /// The packet identifier that contains the stream type data.
    pub elementary_pid: Pid,

    pub descriptors: Vec<Descriptor>,
}
impl EsInfo {
    fn read_from<R: Read>(mut reader: R) -> Result<Self> {
        let stream_type = track_io!(reader.read_u8()).and_then(StreamType::from_u8)?;
        let elementary_pid = track!(Pid::read_from(&mut reader))?;

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

        Ok(EsInfo {
            stream_type,
            elementary_pid,
            descriptors,
        })
    }

    fn write_to<W: Write>(&self, mut writer: W) -> Result<()> {
        track_io!(writer.write_u8(self.stream_type as u8))?;
        track!(self.elementary_pid.write_to(&mut writer))?;

        let es_info_len: usize = self.descriptors.iter().map(|d| 2 + d.data.len()).sum();
        track_assert!(es_info_len <= 0b0011_1111_1111, ErrorKind::InvalidInput);

        let n = 0b1111_0000_0000_0000 | es_info_len as u16;
        track_io!(writer.write_u16::<BigEndian>(n))?;

        for d in &self.descriptors {
            track!(d.write_to(&mut writer))?;
        }
        Ok(())
    }
}

/// Program or elementary stream descriptor.
#[allow(missing_docs)]
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Descriptor {
    pub tag: u8,
    pub data: Vec<u8>,
}
impl Descriptor {
    fn read_from<R: Read>(mut reader: R) -> Result<Self> {
        let tag = track_io!(reader.read_u8())?;
        let len = track_io!(reader.read_u8())?;
        let mut data = vec![0; len as usize];
        track_io!(reader.read_exact(&mut data))?;
        Ok(Descriptor { tag, data })
    }

    fn write_to<W: Write>(&self, mut writer: W) -> Result<()> {
        track_io!(writer.write_u8(self.tag))?;
        track_io!(writer.write_u8(self.data.len() as u8))?;
        track_io!(writer.write_all(&self.data))?;
        Ok(())
    }
}
