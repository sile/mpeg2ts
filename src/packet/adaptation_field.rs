use std::io::Read;
use byteorder::{BigEndian, ReadBytesExt};

use {ErrorKind, Result};
use time::{ProgramClockReference, Timestamp};
use util;

/// Adaptation field.
#[allow(missing_docs)]
#[derive(Debug, Clone)]
pub struct AdaptationField {
    /// Set `true` if current TS packet is in a discontinuity state with respect to
    /// either the continuity counter or the program clock reference.
    pub discontinuity_indicator: bool,

    /// Set `true` when the stream may be decoded without errors from this point.
    pub random_access_indicator: bool,

    /// Set `true` when this stream should be considered "high priority".
    pub es_priority_indicator: bool,

    pub pcr: Option<ProgramClockReference>,
    pub opcr: Option<ProgramClockReference>,

    /// Indicates how many TS packets from this one a splicing point occurs.
    pub splice_countdown: Option<i8>,

    pub transport_private_data: Vec<u8>,
    pub extension: Option<AdaptationExtensionField>,
}
impl AdaptationField {
    pub(super) fn read_from<R: Read>(mut reader: R) -> Result<Self> {
        let adaptation_field_len = track_io!(reader.read_u8())?;
        if adaptation_field_len == 0 {
            return Ok(AdaptationField::default());
        }
        let mut reader = reader.take(u64::from(adaptation_field_len));

        let b = track_io!(reader.read_u8())?;
        let discontinuity_indicator = (b & 0b1000_0000) != 0;
        let random_access_indicator = (b & 0b0100_0000) != 0;
        let es_priority_indicator = (b & 0b0010_0000) != 0;
        let pcr_flag = (b & 0b0001_0000) != 0;
        let opcr_flag = (b & 0b0000_1000) != 0;
        let splicing_point_flag = (b & 0b0000_0100) != 0;
        let transport_private_data_flag = (b & 0b0000_0010) != 0;
        let extension_flag = (b & 0b0000_0001) != 0;

        let pcr = if pcr_flag {
            Some(track!(ProgramClockReference::read_from(&mut reader))?)
        } else {
            None
        };
        let opcr = if opcr_flag {
            Some(track!(ProgramClockReference::read_from(&mut reader))?)
        } else {
            None
        };
        let splice_countdown = if splicing_point_flag {
            Some(track_io!(reader.read_i8())?)
        } else {
            None
        };
        let transport_private_data = if transport_private_data_flag {
            let len = track_io!(reader.read_u8())?;
            let mut buf = vec![0; len as usize];
            track_io!(reader.read_exact(&mut buf))?;
            buf
        } else {
            Vec::new()
        };
        let extension = if extension_flag {
            Some(track!(AdaptationExtensionField::read_from(&mut reader))?)
        } else {
            None
        };
        track!(util::consume_stuffing_bytes(reader))?;

        Ok(AdaptationField {
            discontinuity_indicator,
            random_access_indicator,
            es_priority_indicator,
            pcr,
            opcr,
            splice_countdown,
            transport_private_data,
            extension,
        })
    }
}
impl Default for AdaptationField {
    fn default() -> Self {
        AdaptationField {
            discontinuity_indicator: false,
            random_access_indicator: false,
            es_priority_indicator: false,
            pcr: None,
            opcr: None,
            splice_countdown: None,
            transport_private_data: Vec::new(),
            extension: None,
        }
    }
}

/// Adaptation extension field.
#[allow(missing_docs)]
#[derive(Debug, Clone)]
pub struct AdaptationExtensionField {
    pub legal_time_window: Option<LegalTimeWindow>,
    pub piecewise_rate: Option<PiecewiseRate>,
    pub seamless_splice: Option<SeamlessSplice>,
}
impl AdaptationExtensionField {
    fn read_from<R: Read>(mut reader: R) -> Result<Self> {
        let extension_len = track_io!(reader.read_u8())?;
        let mut reader = reader.take(u64::from(extension_len));

        let b = track_io!(reader.read_u8())?;
        let legal_time_window_flag = (b & 0b1000_0000) != 0;
        let piecewise_rate_flag = (b & 0b0100_0000) != 0;
        let seamless_splice_flag = (b & 0b0010_0000) != 0;

        let legal_time_window = if legal_time_window_flag {
            Some(track!(LegalTimeWindow::read_from(&mut reader))?)
        } else {
            None
        };
        let piecewise_rate = if piecewise_rate_flag {
            Some(track!(PiecewiseRate::read_from(&mut reader))?)
        } else {
            None
        };
        let seamless_splice = if seamless_splice_flag {
            Some(track!(SeamlessSplice::read_from(&mut reader))?)
        } else {
            None
        };

        track_assert_eq!(reader.limit(), 0, ErrorKind::InvalidInput);
        Ok(AdaptationExtensionField {
            legal_time_window,
            piecewise_rate,
            seamless_splice,
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

    fn read_from<R: Read>(mut reader: R) -> Result<Self> {
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

    fn read_from<R: Read>(mut reader: R) -> Result<Self> {
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

    fn read_from<R: Read>(mut reader: R) -> Result<Self> {
        let n = track_io!(reader.read_uint::<BigEndian>(5))?;
        Ok(SeamlessSplice {
            splice_type: (n >> 36) as u8,
            dts_next_access_unit: track!(Timestamp::from_u64(n & 0x0F_FFFF_FFFF))?,
        })
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum AdaptationFieldControl {
    PayloadOnly = 0b01,
    AdaptationFieldOnly = 0b10,
    AdaptationFieldAndPayload = 0b11,
}
impl AdaptationFieldControl {
    pub fn has_adaptation_field(&self) -> bool {
        *self != AdaptationFieldControl::PayloadOnly
    }

    pub fn has_payload(&self) -> bool {
        *self != AdaptationFieldControl::AdaptationFieldOnly
    }

    pub fn from_u8(n: u8) -> Result<Self> {
        Ok(match n {
            0b01 => AdaptationFieldControl::PayloadOnly,
            0b10 => AdaptationFieldControl::AdaptationFieldOnly,
            0b11 => AdaptationFieldControl::AdaptationFieldAndPayload,
            0b00 => track_panic!(ErrorKind::InvalidInput, "Reserved for future use"),
            _ => track_panic!(ErrorKind::InvalidInput, "Unexpected value: {}", n),
        })
    }
}
