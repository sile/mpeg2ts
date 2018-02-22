use std::collections::HashMap;
use std::io::Read;

use {ErrorKind, Result};
use packet::{PacketPayload, PacketReader, Pid};
use time::{ProgramClockReference, Timestamp};

// TODO: name
#[derive(Debug)]
pub struct EsFrame {
    pub stream_id: u8,
    pub pts: Option<Timestamp>,
    pub dts: Option<Timestamp>,
    pub data: Vec<u8>,

    // TODO:
    pid: Pid,
    frame_len: Option<usize>,
}

#[derive(Debug)]
pub struct EsFrameReader<R> {
    packet_reader: PacketReader<R>,
    es_frames: HashMap<Pid, EsFrame>,
    pcr: ProgramClockReference,
}
impl<R: Read> EsFrameReader<R> {
    pub fn new(packet_reader: PacketReader<R>) -> Self {
        EsFrameReader {
            packet_reader,
            es_frames: HashMap::new(),
            pcr: ProgramClockReference::from(0), // TODO
        }
    }
    pub fn pcr(&self) -> ProgramClockReference {
        self.pcr
    }
    pub fn read_es_frame(&mut self) -> Result<Option<EsFrame>> {
        loop {
            let packet = if let Some(packet) = track!(self.packet_reader.read_packet())? {
                packet
            } else {
                return Ok(None);
            };
            if let Some(pcr) = packet.adaptation_field.as_ref().and_then(|a| a.pcr) {
                self.pcr = pcr;
            }
            match packet.payload {
                Some(PacketPayload::Pes(ref pes)) => {
                    let stream_id = pes.header.stream_id;

                    let frame_len = if pes.header.pes_packet_len != 0 {
                        Some(pes.header.pes_packet_len as usize)
                    } else {
                        None
                    };
                    let mut data = Vec::with_capacity(frame_len.unwrap_or_else(|| pes.data.len()));
                    data.extend_from_slice(&pes.data);
                    let frame = EsFrame {
                        stream_id,
                        pts: pes.header.pts,
                        dts: pes.header.dts,
                        data,
                        pid: packet.header.pid,
                        frame_len,
                    };
                    if frame_len == Some(frame.data.len()) {
                        return Ok(Some(frame));
                    } else {
                        if let Some(pred) = self.es_frames.insert(packet.header.pid, frame) {
                            return Ok(Some(pred));
                        }
                    }
                }
                Some(PacketPayload::Raw(ref data)) => {
                    let mut frame = track_assert_some!(
                        self.es_frames.remove(&packet.header.pid),
                        ErrorKind::InvalidInput
                    );
                    frame.data.extend_from_slice(data);
                    if let Some(len) = frame.frame_len {
                        track_assert!(frame.data.len() <= len, ErrorKind::InvalidInput);
                        if frame.data.len() == len {
                            return Ok(Some(frame));
                        } else {
                            self.es_frames.insert(packet.header.pid, frame);
                        }
                    } else {
                        self.es_frames.insert(packet.header.pid, frame);
                    }
                }
                _ => {}
            }
        }
    }
}
