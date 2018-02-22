//! Time-related constituent elements.
use std::io::Read;
use byteorder::{BigEndian, ReadBytesExt};

use {ErrorKind, Result};

/// Timestamp type for PTS/DTS.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct Timestamp(u64);
impl Timestamp {
    /// 90 kHz.
    pub const RESOLUTION: u64 = 90_000;

    /// Maximum timestamp value.
    pub const MAX: u64 = (1 << 33) - 1;

    /// Makes a new `Timestamp` instance.
    ///
    /// # Errors
    ///
    /// If `n` exceeds `Timestamp::MAX`, it will return an `ErrorKind::InvalidInput` error.
    pub fn new(n: u64) -> Result<Self> {
        track_assert!(
            n <= Self::MAX,
            ErrorKind::InvalidInput,
            "Too large value: {}",
            n
        );
        Ok(Timestamp(n))
    }

    /// Returns the value of the timestamp.
    pub fn as_u64(&self) -> u64 {
        self.0
    }

    pub(crate) fn from_u64(n: u64) -> Result<Self> {
        track_assert!(
            (n & 1) != 0,
            ErrorKind::InvalidInput,
            "Unexpected marker bit"
        );
        track_assert!(
            ((n >> 16) & 1) != 0,
            ErrorKind::InvalidInput,
            "Unexpected marker bit"
        );
        track_assert!(
            ((n >> 32) & 1) != 0,
            ErrorKind::InvalidInput,
            "Unexpected marker bit"
        );

        let n0 = n >> (32 + 1) & ((1 << 3) - 1);
        let n1 = n >> (16 + 1) & ((1 << 15) - 1);
        let n2 = n >> 1 & ((1 << 15) - 1);
        Ok(Timestamp((n0 << 30) | (n1 << 15) | n2))
    }

    pub(crate) fn read_from<R: Read>(mut reader: R, check_bits: u8) -> Result<Self> {
        let n = track_io!(reader.read_uint::<BigEndian>(5))?;
        track_assert_eq!((n >> 36) as u8, check_bits, ErrorKind::InvalidInput);
        track!(Self::from_u64(n))
    }
}
impl From<u32> for Timestamp {
    fn from(n: u32) -> Self {
        Timestamp(u64::from(n))
    }
}

/// Timestamp type for PCR/OPCR/ESCR.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct ClockReference(u64);
impl ClockReference {
    /// 27MHz.
    pub const RESOLUTION: u64 = 27_000_000;

    /// Maximum PCR value.
    pub const MAX: u64 = ((1 << 33) - 1) * 300 + 0b1_1111_1111;

    /// Makes a new `ClockReference` instance.
    ///
    /// # Errors
    ///
    /// If `n` exceeds `ClockReference::MAX`, it will return an `ErrorKind::InvalidInput` error.
    pub fn new(n: u64) -> Result<Self> {
        track_assert!(
            n <= Self::MAX,
            ErrorKind::InvalidInput,
            "Too large value: {}",
            n
        );
        Ok(ClockReference(n))
    }

    /// Returns the value of the PCR.
    pub fn as_u64(&self) -> u64 {
        self.0
    }

    pub(crate) fn read_pcr_from<R: Read>(mut reader: R) -> Result<Self> {
        let n = track_io!(reader.read_uint::<BigEndian>(6))?;
        let base = n >> 15;
        let extension = n & 0b1_1111_1111;
        Ok(ClockReference(base * 300 + extension))
    }

    pub(crate) fn read_escr_from<R: Read>(mut reader: R) -> Result<Self> {
        let n = track_io!(reader.read_uint::<BigEndian>(6))?;
        track_assert_eq!(n >> 46, 0, ErrorKind::InvalidInput);

        track_assert_eq!(n & 1, 1, ErrorKind::InvalidInput);
        let extension = n & 0b1_1111_1111;

        let n = n >> 10;
        track_assert_eq!(n & 1, 1, ErrorKind::InvalidInput);
        track_assert_eq!((n >> 16) & 1, 1, ErrorKind::InvalidInput);
        track_assert_eq!((n >> 32) & 1, 1, ErrorKind::InvalidInput);

        let n0 = (n >> 1) & ((1 << 15) - 1);
        let n1 = (n >> 17) & ((1 << 15) - 1);
        let n2 = (n >> 33) & ((1 << 3) - 1);
        let base = n0 | (n1 << 15) | (n2 << 30);
        Ok(ClockReference(base * 300 + extension))
    }
}
impl From<u32> for ClockReference {
    fn from(n: u32) -> Self {
        ClockReference(u64::from(n))
    }
}
impl From<Timestamp> for ClockReference {
    fn from(f: Timestamp) -> Self {
        ClockReference(f.0 * 300)
    }
}
