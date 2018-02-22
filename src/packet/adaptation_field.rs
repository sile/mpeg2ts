use std::io::Read;
use byteorder::ReadBytesExt;

use {ErrorKind, Result};
use packet::{LegalTimeWindow, PiecewiseRate, SeamlessSplice};
use time::ClockReference;
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

    pub pcr: Option<ClockReference>,
    pub opcr: Option<ClockReference>,

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
            Some(track!(ClockReference::read_from(&mut reader))?)
        } else {
            None
        };
        let opcr = if opcr_flag {
            Some(track!(ClockReference::read_from(&mut reader))?)
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
