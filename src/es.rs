use std::collections::HashMap;
use std::io::Read;

use {ErrorKind, Result};
use packet::{PacketPayload, PacketReader, Pid};
use time::{ClockReference, Timestamp};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct StreamId(u8);
impl StreamId {
    pub fn new(n: u8) -> Self {
        StreamId(n)
    }
    pub fn as_u8(&self) -> u8 {
        self.0
    }
    pub fn is_audio(&self) -> bool {
        0xC0 <= self.0 && self.0 <= 0xDF
    }
    pub fn is_video(&self) -> bool {
        0xE0 <= self.0 && self.0 <= 0xEF
    }
}

// TODO: name
#[derive(Debug)]
pub struct EsFrame {
    pub stream_id: StreamId,
    pub pts: Option<Timestamp>,
    pub dts: Option<Timestamp>,
    pub data: Vec<u8>,

    // TODO:
    pid: Pid,
}

#[derive(Debug)]
pub struct EsFrameReader<R> {
    packet_reader: PacketReader<R>,
    es_frames: HashMap<Pid, EsFrame>,
    pcr: ClockReference,
}
impl<R: Read> EsFrameReader<R> {
    pub fn new(packet_reader: PacketReader<R>) -> Self {
        EsFrameReader {
            packet_reader,
            es_frames: HashMap::new(),
            pcr: ClockReference::from(0), // TODO
        }
    }
    pub fn pcr(&self) -> ClockReference {
        self.pcr
    }
    pub fn read_es_frame(&mut self) -> Result<Option<EsFrame>> {
        loop {
            let packet = if let Some(packet) = track!(self.packet_reader.read_packet())? {
                packet
            } else {
                if let Some(key) = self.es_frames.keys().nth(0).cloned() {
                    return Ok(self.es_frames.remove(&key));
                }
                return Ok(None);
            };
            if let Some(pcr) = packet.adaptation_field.as_ref().and_then(|a| a.pcr) {
                self.pcr = pcr;
            }
            match packet.payload {
                Some(PacketPayload::Pes(ref pes)) => {
                    let stream_id = pes.header.stream_id;

                    let mut data = Vec::new();
                    data.extend_from_slice(&pes.data);
                    let frame = EsFrame {
                        stream_id,
                        pts: pes.header.pts,
                        dts: pes.header.dts,
                        data,
                        pid: packet.header.pid,
                    };
                    if let Some(pred) = self.es_frames.insert(packet.header.pid, frame) {
                        return Ok(Some(pred));
                    }
                }
                Some(PacketPayload::Raw(ref data)) => {
                    let frame = track_assert_some!(
                        self.es_frames.get_mut(&packet.header.pid),
                        ErrorKind::InvalidInput
                    );
                    frame.data.extend_from_slice(data);
                }
                _ => {}
            }
        }
    }
}
