//! Time-related constituent elements.
use std::io::Read;
use byteorder::{BigEndian, ReadBytesExt};

use {ErrorKind, Result};

// Timestamp for PTS/DTS.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct Timestamp(u64);
impl Timestamp {
    /// 90 kHz.
    pub const RESOLUTION: u64 = 90_000;

    pub const MAX: u64 = (1 << 33) - 1;

    pub fn from_u64(n: u64) -> Result<Self> {
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
    pub fn read_from<R: Read>(mut reader: R, check_bits: u8) -> Result<Self> {
        let n = track_io!(reader.read_uint::<BigEndian>(5))?;
        track_assert_eq!((n >> 36) as u8, check_bits, ErrorKind::InvalidInput);
        track!(Self::from_u64(n))
    }

    pub fn new(n: u64) -> Result<Self> {
        track_assert!(
            n <= Self::MAX,
            ErrorKind::InvalidInput,
            "Too large value: {}",
            n
        );
        Ok(Timestamp(n))
    }
    pub fn as_u64(&self) -> u64 {
        self.0
    }
}
impl From<u32> for Timestamp {
    fn from(n: u32) -> Self {
        Timestamp(u64::from(n))
    }
}

/// PCR.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct ProgramClockReference(u64);
impl ProgramClockReference {
    /// 27MHz.
    pub const RESOLUTION: u64 = 27_000_000;

    pub const MAX: u64 = ((1 << 33) - 1) * 300 + 0b1_1111_1111;

    pub fn read_from<R: Read>(mut reader: R) -> Result<Self> {
        let n = track_io!(reader.read_uint::<BigEndian>(6))?;
        let base = n >> 15;
        let extension = n & 0b1_1111_1111;
        Ok(ProgramClockReference(base * 300 + extension))
    }

    pub fn new(n: u64) -> Result<Self> {
        track_assert!(
            n <= Self::MAX,
            ErrorKind::InvalidInput,
            "Too large value: {}",
            n
        );
        Ok(ProgramClockReference(n))
    }
    pub fn as_u64(&self) -> u64 {
        self.0
    }
}
impl From<u32> for ProgramClockReference {
    fn from(n: u32) -> Self {
        ProgramClockReference(u64::from(n))
    }
}
