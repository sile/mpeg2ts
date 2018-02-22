use std::collections::HashMap;
use std::io::Read;

use {ErrorKind, Result};
use ts::{AdaptationField, Pid, TsHeader, TsPacket, TsPayload};
use ts::payload::{Bytes, Null, Pat, Pes, Pmt};

/// TS packet reader.
#[derive(Debug)]
pub struct TsPacketReader<R> {
    stream: R,
    pids: HashMap<Pid, PidKind>,
}
impl<R: Read> TsPacketReader<R> {
    /// Makes a new `TsPacketReader` instance.
    pub fn new(stream: R) -> Self {
        TsPacketReader {
            stream,
            pids: HashMap::new(),
        }
    }

    /// Returns a reference to the underlaying byte stream.
    pub fn stream(&self) -> &R {
        &self.stream
    }

    /// Converts `TsPacketReader` into the underlaying byte stream `R`.
    pub fn into_stream(self) -> R {
        self.stream
    }

    /// Reads a TS packet.
    ///
    /// If the end of the stream is reached, it will return `Ok(None)`.
    pub fn read_packet(&mut self) -> Result<Option<TsPacket>> {
        let mut reader = self.stream.by_ref().take(TsPacket::SIZE as u64);
        let mut peek = [0; 1];
        if track_io!(reader.read(&mut peek))? == 0 {
            return Ok(None);
        }

        let (header, adaptation_field_control, payload_unit_start_indicator) =
            track!(TsHeader::read_from(peek.chain(&mut reader)))?;

        let adaptation_field = if adaptation_field_control.has_adaptation_field() {
            Some(track!(AdaptationField::read_from(&mut reader))?)
        } else {
            None
        };

        let payload = if adaptation_field_control.has_payload() {
            let payload = match header.pid {
                Pid::PAT => {
                    let pat = track!(Pat::read_from(&mut reader))?;
                    for pa in &pat.table {
                        self.pids.insert(pa.program_map_pid, PidKind::Pmt);
                    }
                    TsPayload::Pat(pat)
                }
                Pid::NULL => {
                    let null = track!(Null::read_from(&mut reader))?;
                    TsPayload::Null(null)
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
                            for es in &pmt.table {
                                self.pids.insert(es.elementary_pid, PidKind::Pes);
                            }
                            TsPayload::Pmt(pmt)
                        }
                        PidKind::Pes => {
                            if payload_unit_start_indicator {
                                let pes = track!(Pes::read_from(&mut reader))?;
                                TsPayload::Pes(pes)
                            } else {
                                let bytes = track!(Bytes::read_from(&mut reader))?;
                                TsPayload::Raw(bytes)
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
        Ok(Some(TsPacket {
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
