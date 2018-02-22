use std::io::{self, Read};

use {ErrorKind, Result};
use crc::Crc32;

pub fn consume_stuffing_bytes<R: Read>(mut reader: R) -> Result<()> {
    let mut buf = [0];
    while 1 == track_io!(reader.read(&mut buf))? {
        track_assert_eq!(buf[0], 0xFF, ErrorKind::InvalidInput);
    }
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
