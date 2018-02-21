use std::collections::HashSet;
use std::fmt;
use std::io::Read;
use std::ops::Deref;
use byteorder::{BigEndian, ReadBytesExt};

use {ErrorKind, Result};
use null::Null;
use pat::Pat;
use pes::Pes;
use pmt::Pmt;
use time::ProgramClockReference;
use util;

const PACKET_LEN: u64 = 188;
const SYNC_BYTE: u8 = 0x47;

pub type Pid = u16;

#[derive(Debug)]
pub enum Payload {
    Pat(Pat),
    Pmt(Pmt),
    Pes(Pes),
    Null(Null),
    Data(Data), // TODO
    Todo(Vec<u8>),
}

pub struct Data {
    buf: [u8; 188],
    len: usize,
}
impl Data {
    pub fn read_from<R: Read>(mut reader: R) -> Result<Self> {
        // TODO: read_to_end
        let mut buf = [0; 188];
        let len = track_io!(reader.read(&mut buf))?;
        Ok(Data { buf, len })
    }
}
impl Deref for Data {
    type Target = [u8];
    fn deref(&self) -> &Self::Target {
        &self.buf[..self.len]
    }
}
impl AsRef<[u8]> for Data {
    fn as_ref(&self) -> &[u8] {
        self.deref()
    }
}
impl fmt::Debug for Data {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "Data({:?})", self.deref())
    }
}

#[derive(Debug)]
pub struct PacketReader<R> {
    stream: R,
    pmt_pids: HashSet<Pid>,
    es_pids: HashSet<Pid>,
}
impl<R: Read> PacketReader<R> {
    pub fn new(stream: R) -> Self {
        PacketReader {
            stream,
            pmt_pids: HashSet::new(),
            es_pids: HashSet::new(),
        }
    }
    pub fn read_packet(&mut self) -> Result<Option<Packet>> {
        let mut reader = self.stream.by_ref().take(PACKET_LEN);

        let mut peek_buf = [0; 1];
        if track_io!(reader.read(&mut peek_buf))? == 0 {
            return Ok(None);
        }

        let (header, adaptation_field_control) =
            track!(PacketHeader::read_from(peek_buf.chain(&mut reader)))?;

        let adaptation_field = if adaptation_field_control.has_adaptation_field() {
            Some(track!(AdaptationField::read_from(&mut reader))?)
        } else {
            None
        };
        let payload = if adaptation_field_control.has_payload() {
            if header.pid == Pat::PID {
                let pat = track!(Pat::read_from(&mut reader))?;
                for e in &pat.entries {
                    self.pmt_pids.insert(e.program_map_pid);
                }
                Some(Payload::Pat(pat))
            } else if self.pmt_pids.contains(&header.pid) {
                let pmt = track!(Pmt::read_from(&mut reader))?;
                for e in &pmt.es_info_entries {
                    self.es_pids.insert(e.elementary_pid);
                }
                Some(Payload::Pmt(pmt))
            } else if self.es_pids.contains(&header.pid) {
                if header.payload_unit_start_indicator {
                    let pes = track!(Pes::read_from(&mut reader))?;
                    Some(Payload::Pes(pes))
                } else {
                    let data = track!(Data::read_from(&mut reader))?;
                    Some(Payload::Data(data))
                }
            } else if header.pid == Null::PID {
                let null = track!(Null::read_from(&mut reader))?;
                Some(Payload::Null(null))
            } else {
                let mut buf = vec![0; reader.limit() as usize];
                track_io!(reader.read_exact(&mut buf))?;
                Some(Payload::Todo(buf))
            }
        } else {
            None
        };

        track_assert_eq!(reader.limit(), 0, ErrorKind::InvalidInput);
        Ok(Some(Packet {
            header,
            adaptation_field,
            payload,
        }))
    }
}

#[derive(Debug)]
pub struct AdaptationField {
    pub discontinuity_indicator: bool,
    pub random_access_indicator: bool,
    pub es_priority_indicator: bool,
    pub pcr: Option<ProgramClockReference>,
    pub opcr: Option<ProgramClockReference>,
    pub splice_countdown: Option<u8>,
    pub transport_private_data: Vec<u8>,
    pub adaptation_extension: Option<AdaptationExtension>,
}
impl AdaptationField {
    pub fn read_from<R: Read>(mut reader: R) -> Result<Self> {
        let adaptation_field_len = track_io!(reader.read_u8())?;
        let mut reader = reader.take(u64::from(adaptation_field_len));

        let flag = track_io!(reader.read_u8())?;
        let discontinuity_indicator = (flag & 0b1000_0000) != 0;
        let random_access_indicator = (flag & 0b0100_0000) != 0;
        let es_priority_indicator = (flag & 0b0010_0000) != 0;
        let pcr_flag = (flag & 0b0001_0000) != 0;
        let opcr_flag = (flag & 0b0000_1000) != 0;
        let splicing_point_flag = (flag & 0b0000_0100) != 0;
        let transport_private_data_flag = (flag & 0b0000_0010) != 0;
        let adaptation_extension_flag = (flag & 0b0000_0001) != 0;

        let pcr = if pcr_flag {
            Some(track!(ProgramClockReference::read_from(&mut reader))?)
        } else {
            None
        };
        let opcr = if opcr_flag {
            Some(track!(ProgramClockReference::read_from(&mut reader))?)
        } else {
            None
        };
        let splice_countdown = if splicing_point_flag {
            Some(track_io!(reader.read_u8())?)
        } else {
            None
        };
        let transport_private_data = if transport_private_data_flag {
            let len = track_io!(reader.read_u8())?;
            let mut buf = vec![0; len as usize];
            track_io!(reader.read_exact(&mut buf))?;
            buf
        } else {
            Vec::new()
        };
        let adaptation_extension = if adaptation_extension_flag {
            Some(track!(AdaptationExtension::read_from(&mut reader))?)
        } else {
            None
        };
        track!(util::consume_stuffing_bytes(reader))?;

        Ok(AdaptationField {
            discontinuity_indicator,
            random_access_indicator,
            es_priority_indicator,
            pcr,
            opcr,
            splice_countdown,
            transport_private_data,
            adaptation_extension,
        })
    }
}

#[derive(Debug)]
pub struct AdaptationExtension {
    pub legal_time_window: Option<LegalTimeWindow>,
    pub piecewise_rate: Option<u32>,
    pub seamless_splice: Option<SeamlessSplice>,
}
impl AdaptationExtension {
    pub fn read_from<R: Read>(mut reader: R) -> Result<Self> {
        let adaptation_extension_len = track_io!(reader.read_u8())?;
        let mut reader = reader.take(u64::from(adaptation_extension_len));

        let flag = track_io!(reader.read_u8())?;
        let legal_time_window_flag = (flag & 0b1000_0000) != 0;
        let piecewise_rate_flag = (flag & 0b0100_0000) != 0;
        let seamless_splice_flag = (flag & 0b0010_0000) != 0;

        let legal_time_window = if legal_time_window_flag {
            let n = track_io!(reader.read_u16::<BigEndian>())?;
            Some(LegalTimeWindow {
                is_valid: (n & 0b1000_0000_0000_0000) != 0,
                offset: n & 0b0111_1111_1111_1111,
            })
        } else {
            None
        };
        let piecewise_rate = if piecewise_rate_flag {
            let n = track_io!(reader.read_uint::<BigEndian>(3))? as u32;
            Some(n & 0x3FFF_FFFF)
        } else {
            None
        };
        let seamless_splice = if seamless_splice_flag {
            let n = track_io!(reader.read_uint::<BigEndian>(5))?;
            Some(SeamlessSplice {
                splice_type: (n >> 36) as u8,
                dts_next_access_unit: n & 0x0F_FFFF_FFFF,
            })
        } else {
            None
        };

        track_assert_eq!(reader.limit(), 0, ErrorKind::InvalidInput);
        Ok(AdaptationExtension {
            legal_time_window,
            piecewise_rate,
            seamless_splice,
        })
    }
}

#[derive(Debug)]
pub struct LegalTimeWindow {
    pub is_valid: bool,
    pub offset: u16,
}

#[derive(Debug)]
pub struct SeamlessSplice {
    pub splice_type: u8,
    pub dts_next_access_unit: u64,
}

#[derive(Debug)]
pub struct Packet {
    pub header: PacketHeader,
    pub adaptation_field: Option<AdaptationField>,
    pub payload: Option<Payload>,
}

#[derive(Debug)]
pub struct PacketHeader {
    pub transport_error_indicator: bool,
    pub payload_unit_start_indicator: bool,
    pub transport_priority: bool,
    pub pid: u16,
    pub transport_scrambling_control: u8,
    pub continuity_counter: u8,
}
impl PacketHeader {
    pub fn read_from<R: Read>(mut reader: R) -> Result<(Self, AdaptationFieldControl)> {
        let sync_byte = track_io!(reader.read_u8())?;
        track_assert_eq!(sync_byte, SYNC_BYTE, ErrorKind::InvalidInput);

        let n = track_io!(reader.read_u16::<BigEndian>())?;
        let transport_error_indicator = (n & 0b1000_0000_0000_0000) != 0;
        let payload_unit_start_indicator = (n & 0b0100_0000_0000_0000) != 0;
        let transport_priority = (n & 0b0010_0000_0000_0000) != 0;
        let pid = n & 0b0001_1111_1111_1111;

        let n = track_io!(reader.read_u8())?;
        let transport_scrambling_control = n >> 6;
        let adaptation_field_control = match (n >> 4) & 0b11 {
            0b01 => AdaptationFieldControl::PayloadOnly,
            0b10 => AdaptationFieldControl::AdaptationFieldOnly,
            0b11 => AdaptationFieldControl::AdaptationFieldAndPayload,
            v => track_panic!(ErrorKind::InvalidInput, "{}", v),
        };
        let continuity_counter = n & 0b1111;

        let header = PacketHeader {
            transport_error_indicator,
            payload_unit_start_indicator,
            transport_priority,
            pid,
            transport_scrambling_control,
            continuity_counter,
        };
        Ok((header, adaptation_field_control))
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum AdaptationFieldControl {
    PayloadOnly = 1,
    AdaptationFieldOnly = 2,
    AdaptationFieldAndPayload = 3,
}
impl AdaptationFieldControl {
    pub fn has_adaptation_field(&self) -> bool {
        *self != AdaptationFieldControl::PayloadOnly
    }
    pub fn has_payload(&self) -> bool {
        *self != AdaptationFieldControl::AdaptationFieldOnly
    }
}
