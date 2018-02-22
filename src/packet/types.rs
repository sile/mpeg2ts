use std::fmt;
use std::hash::{Hash, Hasher};
use std::io::{Read, Write};
use std::ops::Deref;
use byteorder::{BigEndian, ReadBytesExt};

use {ErrorKind, Result};
use packet::Packet;
use time::Timestamp;

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

    // TODO: pub(super)
    pub(crate) fn read_from<R: Read>(mut reader: R) -> Result<Self> {
        let n = track_io!(reader.read_u16::<BigEndian>())?;
        track_assert_eq!(
            n & 0b1110_0000_0000_0000,
            0b1110_0000_0000_0000,
            ErrorKind::InvalidInput,
            "Unexpected reserved bits"
        );
        Ok(Pid(n & 0b0001_1111_1111_1111))
    }
}
impl From<u8> for Pid {
    fn from(f: u8) -> Self {
        Pid(u16::from(f))
    }
}

/// Continuity counter.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct ContinuityCounter(u8);
impl ContinuityCounter {
    /// Maximum counter value.
    pub const MAX: u8 = (1 << 4) - 1;

    /// Makes a new `ContinuityCounter` instance that has the value `0`.
    pub fn new() -> Self {
        ContinuityCounter(0)
    }

    /// Makes a new `ContinuityCounter` instance with the given value.
    ///
    /// # Errors
    ///
    /// If `n` exceeds `ContinuityCounter::MAX`, it will return an `ErrorKind::InvalidInput` error.
    pub fn from_u8(n: u8) -> Result<Self> {
        track_assert!(
            n <= Self::MAX,
            ErrorKind::InvalidInput,
            "Too large counter: {}",
            n
        );
        Ok(ContinuityCounter(n))
    }

    /// Returns the value of the counter.
    pub fn as_u8(&self) -> u8 {
        self.0
    }

    /// Increments the counter.
    ///
    /// # Examples
    ///
    /// ```
    /// use mpeg2ts::packet::ContinuityCounter;
    ///
    /// let mut c = ContinuityCounter::new();
    /// assert_eq!(c.as_u8(), 0);
    ///
    /// for _ in 0..5 { c.increment(); }
    /// assert_eq!(c.as_u8(), 5);
    ///
    /// for _ in 0..11 { c.increment(); }
    /// assert_eq!(c.as_u8(), 0);
    /// ```
    pub fn increment(&mut self) {
        self.0 = (self.0 + 1) & Self::MAX;
    }
}
impl Default for ContinuityCounter {
    fn default() -> Self {
        Self::new()
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

    // TODO: priv
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

    // TODO: priv
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
impl Hash for Bytes {
    fn hash<H: Hasher>(&self, hasher: &mut H) {
        self.as_ref().hash(hasher);
    }
}

/// Transport scrambling control.
#[allow(missing_docs)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum TransportScramblingControl {
    NotScrambled = 0b00,
    ScrambledWithEvenKey = 0b10,
    ScrambledWithOddKey = 0b11,
}
impl TransportScramblingControl {
    pub(super) fn from_u8(n: u8) -> Result<Self> {
        Ok(match n {
            0b00 => TransportScramblingControl::NotScrambled,
            0b10 => TransportScramblingControl::ScrambledWithEvenKey,
            0b11 => TransportScramblingControl::ScrambledWithOddKey,
            0b01 => track_panic!(ErrorKind::InvalidInput, "Reserved for future use"),
            _ => track_panic!(ErrorKind::InvalidInput, "Unexpected value: {}", n),
        })
    }
}

/// Legal time window.
#[allow(missing_docs)]
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct LegalTimeWindow {
    is_valid: bool,
    offset: u16,
}
impl LegalTimeWindow {
    /// Maximum offset value.
    pub const MAX_OFFSET: u16 = (1 << 15) - 1;

    /// Makes a new `LegalTimeWindow` instance.
    ///
    /// # Errors
    ///
    /// If `offset` exceeds `LegalTimeWindow::MAX_OFFSET`, it will return an `ErrorKind::InvalidInput` error.
    pub fn new(is_valid: bool, offset: u16) -> Result<Self> {
        track_assert!(
            offset <= Self::MAX_OFFSET,
            ErrorKind::InvalidInput,
            "Too large offset: {}",
            offset
        );
        Ok(LegalTimeWindow { is_valid, offset })
    }

    /// Returns `true` if the window is valid, otherwise `false`.
    pub fn is_valid(&self) -> bool {
        self.is_valid
    }

    /// Returns the offset of the window.
    pub fn offset(&self) -> u16 {
        self.offset
    }

    pub(super) fn read_from<R: Read>(mut reader: R) -> Result<Self> {
        let n = track_io!(reader.read_u16::<BigEndian>())?;
        Ok(LegalTimeWindow {
            is_valid: (n & 0b1000_0000_0000_0000) != 0,
            offset: n & 0b0111_1111_1111_1111,
        })
    }
}

/// Piecewise rate.
#[allow(missing_docs)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct PiecewiseRate(u32);
impl PiecewiseRate {
    /// Maximum rate.
    pub const MAX: u32 = (1 << 22) - 1;

    /// Makes a new `PiecewiseRate` instance.
    ///
    /// # Errors
    ///
    /// If `rate` exceeds `PiecewiseRate::MAX`, it will return an `ErrorKind::InvalidInput` error.
    pub fn new(rate: u32) -> Result<Self> {
        track_assert!(
            rate <= Self::MAX,
            ErrorKind::InvalidInput,
            "Too large rate: {}",
            rate
        );
        Ok(PiecewiseRate(rate))
    }

    /// Returns the value of the `PiecewiseRate` instance.
    pub fn as_u32(&self) -> u32 {
        self.0
    }

    pub(super) fn read_from<R: Read>(mut reader: R) -> Result<Self> {
        let n = track_io!(reader.read_uint::<BigEndian>(3))? as u32;
        Ok(PiecewiseRate(n & 0x3FFF_FFFF))
    }
}

/// Seamless splice.
#[allow(missing_docs)]
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct SeamlessSplice {
    splice_type: u8,
    dts_next_access_unit: Timestamp,
}
impl SeamlessSplice {
    /// Maximum splice type value.
    pub const MAX_SPLICE_TYPE: u8 = (1 << 4) - 1;

    /// Makes a new `SeamlessSplice` instance.
    ///
    /// # Errors
    ///
    /// If `splice_type` exceeds `SeamlessSplice::MAX_SPLICE_TYPE`,
    /// it will return an `ErrorKind::InvalidInput` error.
    pub fn new(splice_type: u8, dts_next_access_unit: Timestamp) -> Result<Self> {
        track_assert!(
            splice_type <= Self::MAX_SPLICE_TYPE,
            ErrorKind::InvalidInput,
            "Too large splice type: {}",
            splice_type
        );
        Ok(SeamlessSplice {
            splice_type,
            dts_next_access_unit,
        })
    }

    /// Returns the splice type (i.e., parameters of the H.262 splice).
    pub fn splice_type(&self) -> u8 {
        self.splice_type
    }

    /// Returns the PES DTS of the splice point.
    pub fn dts_next_access_unit(&self) -> Timestamp {
        self.dts_next_access_unit
    }

    pub(super) fn read_from<R: Read>(mut reader: R) -> Result<Self> {
        let n = track_io!(reader.read_uint::<BigEndian>(5))?;
        Ok(SeamlessSplice {
            splice_type: (n >> 36) as u8,
            dts_next_access_unit: track!(Timestamp::from_u64(n & 0x0F_FFFF_FFFF))?,
        })
    }
}

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
