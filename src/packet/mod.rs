//! Transport stream packet.
//!
//! # References
//!
//! - [MPEG transport stream](https://en.wikipedia.org/wiki/MPEG_transport_stream)
use std::collections::HashMap;
use std::io::Read;
use byteorder::{BigEndian, ReadBytesExt};

pub use self::adaptation_field::{AdaptationExtensionField, AdaptationField};
pub use self::null::Null;
pub use self::pes::{Pes, PesHeader};
pub use self::types::{Bytes, ContinuityCounter, LegalTimeWindow, Pid, PiecewiseRate,
                      SeamlessSplice, TransportScramblingControl};

use {ErrorKind, Result};
use pat::Pat;
use pmt::Pmt;
use self::adaptation_field::AdaptationFieldControl;

mod adaptation_field;
mod null;
mod pes;
mod types;

/// Transport stream packet.
#[allow(missing_docs)]
#[derive(Debug, Clone)]
pub struct Packet {
    pub header: PacketHeader,
    pub adaptation_field: Option<AdaptationField>,
    pub payload: Option<PacketPayload>,
}
impl Packet {
    /// Size of a packet in bytes.
    pub const SIZE: usize = 188;

    /// Synchronization byte.
    ///
    /// Each packet starts with this byte.
    pub const SYNC_BYTE: u8 = 0x47;
}

/// Packet header.
#[allow(missing_docs)]
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PacketHeader {
    pub transport_error_indicator: bool,
    pub transport_priority: bool,
    pub pid: Pid,
    pub transport_scrambling_control: TransportScramblingControl,
    pub continuity_counter: ContinuityCounter,
}
impl PacketHeader {
    fn read_from<R: Read>(mut reader: R) -> Result<(Self, AdaptationFieldControl, bool)> {
        let sync_byte = track_io!(reader.read_u8())?;
        track_assert_eq!(sync_byte, Packet::SYNC_BYTE, ErrorKind::InvalidInput);

        let n = track_io!(reader.read_u16::<BigEndian>())?;
        let transport_error_indicator = (n & 0b1000_0000_0000_0000) != 0;
        let payload_unit_start_indicator = (n & 0b0100_0000_0000_0000) != 0;
        let transport_priority = (n & 0b0010_0000_0000_0000) != 0;
        let pid = track!(Pid::new(n & 0b0001_1111_1111_1111))?;

        let n = track_io!(reader.read_u8())?;
        let transport_scrambling_control = track!(TransportScramblingControl::from_u8(n >> 6))?;
        let adaptation_field_control = track!(AdaptationFieldControl::from_u8((n >> 4) & 0b11))?;
        let continuity_counter = track!(ContinuityCounter::from_u8(n & 0b1111))?;

        let header = PacketHeader {
            transport_error_indicator,
            transport_priority,
            pid,
            transport_scrambling_control,
            continuity_counter,
        };
        Ok((
            header,
            adaptation_field_control,
            payload_unit_start_indicator,
        ))
    }
}

/// Packet payload.
#[allow(missing_docs)]
#[derive(Debug, Clone)]
pub enum PacketPayload {
    Pat(Pat),
    Pmt(Pmt),
    Pes(Pes),
    Null(Null),
    Raw(Bytes),
}

/// Packet reader.
#[derive(Debug)]
pub struct PacketReader<R> {
    stream: R,
    pids: HashMap<Pid, PidKind>,
}
impl<R: Read> PacketReader<R> {
    /// Makes a new `PacketReader` instance.
    pub fn new(stream: R) -> Self {
        PacketReader {
            stream,
            pids: HashMap::new(),
        }
    }

    /// Returns a reference to the underlaying byte stream.
    pub fn stream(&self) -> &R {
        &self.stream
    }

    /// Converts `PacketReader` into the underlaying byte stream `R`.
    pub fn into_stream(self) -> R {
        self.stream
    }

    /// Reads a packet.
    ///
    /// If the end of the stream is reached, it will return `OK (None)`.
    pub fn read_packet(&mut self) -> Result<Option<Packet>> {
        let mut reader = self.stream.by_ref().take(Packet::SIZE as u64);
        let mut peek = [0; 1];
        if track_io!(reader.read(&mut peek))? == 0 {
            return Ok(None);
        }

        let (header, adaptation_field_control, payload_unit_start_indicator) =
            track!(PacketHeader::read_from(peek.chain(&mut reader)))?;

        let adaptation_field = if adaptation_field_control.has_adaptation_field() {
            Some(track!(AdaptationField::read_from(&mut reader))?)
        } else {
            None
        };

        let payload = if adaptation_field_control.has_payload() {
            let payload = match header.pid {
                Pid::PAT => {
                    let pat = track!(Pat::read_from(&mut reader))?;
                    for e in &pat.entries {
                        self.pids.insert(e.program_map_pid, PidKind::Pmt);
                    }
                    PacketPayload::Pat(pat)
                }
                Pid::NULL => {
                    let null = track!(Null::read_from(&mut reader))?;
                    PacketPayload::Null(null)
                }
                pid => {
                    let kind = track_assert_some!(
                        self.pids.get(&pid).cloned(),
                        ErrorKind::InvalidInput,
                        "Unknown PID: header={:?}",
                        header
                    );
                    match kind {
                        PidKind::Pmt => {
                            let pmt = track!(Pmt::read_from(&mut reader))?;
                            for e in &pmt.es_info_entries {
                                self.pids.insert(e.elementary_pid, PidKind::Pes);
                            }
                            PacketPayload::Pmt(pmt)
                        }
                        PidKind::Pes => {
                            if payload_unit_start_indicator {
                                let pes = track!(Pes::read_from(&mut reader))?;
                                PacketPayload::Pes(pes)
                            } else {
                                let bytes = track!(Bytes::read_from(&mut reader))?;
                                PacketPayload::Raw(bytes)
                            }
                        }
                    }
                }
            };
            Some(payload)
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

#[derive(Debug, Clone)]
enum PidKind {
    Pmt,
    Pes,
}
