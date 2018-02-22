use std::fmt;
use std::io::{Read, Write};
use std::ops::Deref;

use {ErrorKind, Result};
use packet::Packet;

/// Packet Identifier.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct Pid(u16);
impl Pid {
    /// Maximum PID value.
    pub const MAX: u16 = (1 << 13) - 1;

    /// PID of the Program Association Table (PAT) packet.
    pub const PAT: Pid = Pid(0);

    /// PID of the null packet.
    pub const NULL: Pid = Pid(0x1FFF);

    /// Makes a new `Pid` instance.
    ///
    /// # Errors
    ///
    /// If `pid` exceeds `Pid::MAX`, it will return an `ErrorKind::InvalidInput` error.
    pub fn new(pid: u16) -> Result<Self> {
        track_assert!(
            pid <= Self::MAX,
            ErrorKind::InvalidInput,
            "Too large PID: {}",
            pid
        );
        Ok(Pid(pid))
    }

    /// Returns the value of the `Pid`.
    pub fn as_u16(&self) -> u16 {
        self.0
    }
}
impl From<u8> for Pid {
    fn from(f: u8) -> Self {
        Pid(u16::from(f))
    }
}

/// Byte sequence used to represent packet payload data.
#[derive(Clone)]
pub struct Bytes {
    buf: [u8; Bytes::MAX_SIZE],
    len: usize,
}
impl Bytes {
    /// Maximum size of a byte sequence.
    pub const MAX_SIZE: usize = Packet::SIZE - 4 /* the size of the sync byte and a header */;

    /// Makes a new `Bytes` instance.
    ///
    /// # Errors
    ///
    /// If the length of `bytes` exceeds `Bytes::MAX_SIZE`,
    /// it will return an `ErrorKind::InvalidInput` error.
    pub fn new(bytes: &[u8]) -> Result<Self> {
        track_assert!(
            bytes.len() <= Self::MAX_SIZE,
            ErrorKind::InvalidInput,
            "Too large: actual={} bytes, max={} bytes",
            bytes.len(),
            Self::MAX_SIZE
        );

        let len = bytes.len();
        let mut buf = [0; Self::MAX_SIZE];
        (&mut buf[..len]).copy_from_slice(bytes);
        Ok(Bytes { buf, len })
    }

    /// Reads a `Bytes` instance from `reader`.
    pub fn read_from<R: Read>(mut reader: R) -> Result<Self> {
        let mut offset = 0;
        let mut buf = [0; Self::MAX_SIZE];
        loop {
            let read_size = track_io!(reader.read(&mut buf[offset..]))?;
            if read_size == 0 {
                break;
            }
            offset += read_size;
        }
        Ok(Bytes { buf, len: offset })
    }

    /// Writes this `Bytes` to `writer`.
    pub fn write_to<W: Write>(&self, mut writer: W) -> Result<()> {
        track_io!(writer.write_all(self))
    }
}
impl Deref for Bytes {
    type Target = [u8];
    fn deref(&self) -> &Self::Target {
        &self.buf[..self.len]
    }
}
impl AsRef<[u8]> for Bytes {
    fn as_ref(&self) -> &[u8] {
        self.deref()
    }
}
impl fmt::Debug for Bytes {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "Bytes({:?})", self.deref())
    }
}
impl PartialEq for Bytes {
    fn eq(&self, other: &Self) -> bool {
        self.as_ref() == other.as_ref()
    }
}
impl Eq for Bytes {}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn bytes_read_from() {
        let bytes = Bytes::read_from(&[1, 2, 3][..]).unwrap();
        assert_eq!(bytes.as_ref(), [1, 2, 3]);

        let bytes = Bytes::read_from(&[0; 200][..]).unwrap();
        assert_eq!(bytes.as_ref(), &[0; Bytes::MAX_SIZE][..]);
    }

    #[test]
    fn bytes_write_to() {
        let bytes = Bytes::new(&[1, 2, 3]).unwrap();
        let mut buf = Vec::new();
        bytes.write_to(&mut buf).unwrap();
        assert_eq!(bytes.as_ref(), &buf[..]);
    }
}
