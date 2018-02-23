use std::io::{self, Read, Write};

use {ErrorKind, Result};
use crc::Crc32;
use ts::TsPacket;

pub fn consume_stuffing_bytes<R: Read>(mut reader: R) -> Result<()> {
    let mut buf = [0];
    while 1 == track_io!(reader.read(&mut buf))? {
        track_assert_eq!(buf[0], 0xFF, ErrorKind::InvalidInput);
    }
    Ok(())
}

pub fn write_stuffing_bytes<W: Write>(mut writer: W, size: usize) -> Result<()> {
    let buf = [0xFF; TsPacket::SIZE];
    track_io!(writer.write_all(&buf[..size]))?;
    Ok(())
}

#[derive(Debug)]
pub struct WithCrc32<T> {
    stream: T,
    crc32: Crc32,
}
impl<T> WithCrc32<T> {
    pub fn new(stream: T) -> Self {
        WithCrc32 {
            stream,
            crc32: Crc32::new(),
        }
    }
    pub fn crc32(&self) -> u32 {
        self.crc32.value()
    }
}
impl<T: Read> Read for WithCrc32<T> {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        let size = self.stream.read(buf)?;
        self.crc32.update(&buf[..size]);
        Ok(size)
    }
}
impl<T: Write> Write for WithCrc32<T> {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        let size = self.stream.write(buf)?;
        self.crc32.update(&buf[..size]);
        Ok(size)
    }
    fn flush(&mut self) -> io::Result<()> {
        self.stream.flush()
    }
}
