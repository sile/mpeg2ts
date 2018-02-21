use std::io::Read;
use byteorder::{BigEndian, ReadBytesExt};
use num_rational::Ratio;

use {ErrorKind, Result};

// Timestamp for PTS/DTS.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct Timestamp(u32);
impl Timestamp {
    /// 90 kHz.
    pub const RESOLUTION: u32 = 90000;

    pub fn read_from<R: Read>(mut reader: R, check_bits: u8) -> Result<Self> {
        let n0 = track_io!(reader.read_u8())?;
        let n1 = track_io!(reader.read_u16::<BigEndian>())?;
        let n2 = track_io!(reader.read_u16::<BigEndian>())?;

        track_assert_eq!(
            n0 >> 4,
            check_bits,
            ErrorKind::InvalidInput,
            "Unexpected check bits: actual={}, expected={}",
            n0 >> 4,
            check_bits
        );
        track_assert_eq!(n0 & 1, 1, ErrorKind::InvalidInput, "Unexpected marker bit");
        track_assert_eq!(n1 & 1, 1, ErrorKind::InvalidInput, "Unexpected marker bit");
        track_assert_eq!(n2 & 1, 1, ErrorKind::InvalidInput, "Unexpected marker bit");

        let t = u32::from((n0 & 0b0000_1110) >> 1) | u32::from(n1 >> 1) | u32::from(n2 >> 1);
        Ok(Timestamp(t))
    }

    pub fn new(n: u32) -> Self {
        Timestamp(n)
    }
    pub fn as_u32(&self) -> u32 {
        self.0
    }
    pub fn as_ratio(&self) -> Ratio<u32> {
        Ratio::new(self.0, Self::RESOLUTION)
    }
}

/// PCR.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct ProgramClockReference(u64);
impl ProgramClockReference {
    /// 27kHz.
    pub const RESOLUTION: u64 = 27000;

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
    pub fn as_ratio(&self) -> Ratio<u64> {
        Ratio::new(self.0, Self::RESOLUTION)
    }
}
impl From<u32> for ProgramClockReference {
    fn from(n: u32) -> Self {
        ProgramClockReference(u64::from(n))
    }
}
