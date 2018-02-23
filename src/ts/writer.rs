use std::io::Write;
use ts::TsPacket;

use Result;

/// The `ReadTsPacket` trait allows for writing TS packets to a destination.
pub trait WriteTsPacket {
    /// Writes a TS packet.
    fn write_ts_packet(&mut self, packet: &TsPacket) -> Result<()>;
}

/// TS packet writer.
#[derive(Debug)]
pub struct TsPacketWriter<W> {
    stream: W,
}
impl<W: Write> TsPacketWriter<W> {
    /// Makes a new `TsPacketWriter` instance.
    pub fn new(stream: W) -> Self {
        TsPacketWriter { stream }
    }

    /// Returns a reference to the underlaying byte stream.
    pub fn stream(&self) -> &W {
        &self.stream
    }

    /// Converts `TsPacketWriter` into the underlaying byte stream.
    pub fn into_stream(self) -> W {
        self.stream
    }
}
impl<W: Write> WriteTsPacket for TsPacketWriter<W> {
    fn write_ts_packet(&mut self, packet: &TsPacket) -> Result<()> {
        track!(packet.write_to(&mut self.stream))
    }
}
