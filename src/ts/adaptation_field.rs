use crate::time::ClockReference;
use crate::ts::{LegalTimeWindow, PiecewiseRate, SeamlessSplice};
use crate::util;
use crate::{ErrorKind, Result};
use byteorder::{ReadBytesExt, WriteBytesExt};
use std::io::{Read, Write};

/// Adaptation field.
#[allow(missing_docs)]
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
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
    pub(super) fn external_size(&self) -> usize {
        let mut n = 1 /* adaptation_field_len */ + 1 /* flags */;
        if self.pcr.is_some() {
            n += 6;
        }
        if self.opcr.is_some() {
            n += 6;
        }
        if self.splice_countdown.is_some() {
            n += 1;
        }
        n += self.transport_private_data.len();
        if let Some(ref x) = self.extension {
            n += x.external_size();
        }
        n
    }

    pub(super) fn read_from<R: Read>(mut reader: R) -> Result<Option<Self>> {
        let adaptation_field_len = track_io!(reader.read_u8())?;
        if adaptation_field_len == 0 {
            return Ok(None);
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
            Some(track!(ClockReference::read_pcr_from(&mut reader))?)
        } else {
            None
        };
        let opcr = if opcr_flag {
            Some(track!(ClockReference::read_pcr_from(&mut reader))?)
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

        Ok(Some(AdaptationField {
            discontinuity_indicator,
            random_access_indicator,
            es_priority_indicator,
            pcr,
            opcr,
            splice_countdown,
            transport_private_data,
            extension,
        }))
    }

    pub(super) fn write_stuffing_bytes<W: Write>(mut writer: W, field_len: u8) -> Result<()> {
        track_io!(writer.write_u8(field_len))?;
        if field_len == 0 {
            return Ok(());
        }
        track_io!(writer.write_u8(0))?;
        track!(util::write_stuffing_bytes(
            &mut writer,
            (field_len - 1) as usize
        ))?;
        Ok(())
    }

    pub(super) fn write_to<W: Write>(&self, mut writer: W, field_len: u8) -> Result<()> {
        track_io!(writer.write_u8(field_len))?;

        let n = ((self.discontinuity_indicator as u8) << 7)
            | ((self.random_access_indicator as u8) << 6)
            | ((self.es_priority_indicator as u8) << 5)
            | ((self.pcr.is_some() as u8) << 4)
            | ((self.opcr.is_some() as u8) << 3)
            | ((self.splice_countdown.is_some() as u8) << 2)
            | (((!self.transport_private_data.is_empty()) as u8) << 1)
            | self.extension.is_some() as u8;
        track_io!(writer.write_u8(n))?;

        if let Some(ref x) = self.pcr {
            track!(x.write_pcr_to(&mut writer))?;
        }
        if let Some(ref x) = self.opcr {
            track!(x.write_pcr_to(&mut writer))?;
        }
        if let Some(x) = self.splice_countdown {
            track_io!(writer.write_i8(x))?;
        }
        track_io!(writer.write_all(&self.transport_private_data))?;
        if let Some(ref x) = self.extension {
            track!(x.write_to(&mut writer))?;
        }

        let stuffing_len = (field_len + 1) as usize - self.external_size();
        track!(util::write_stuffing_bytes(writer, stuffing_len))?;
        Ok(())
    }
}

/// Adaptation extension field.
#[allow(missing_docs)]
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct AdaptationExtensionField {
    pub legal_time_window: Option<LegalTimeWindow>,
    pub piecewise_rate: Option<PiecewiseRate>,
    pub seamless_splice: Option<SeamlessSplice>,
}
impl AdaptationExtensionField {
    fn external_size(&self) -> usize {
        let mut n = 1 /* length */ + 1 /* flags */;
        if self.legal_time_window.is_some() {
            n += 2;
        }
        if self.piecewise_rate.is_some() {
            n += 3;
        }
        if self.seamless_splice.is_some() {
            n += 5;
        }
        n
    }

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

    fn write_to<W: Write>(&self, mut writer: W) -> Result<()> {
        track_io!(writer.write_u8(self.external_size() as u8 - 1))?;

        let n = ((self.legal_time_window.is_some() as u8) << 7)
            | ((self.piecewise_rate.is_some() as u8) << 6)
            | ((self.seamless_splice.is_some() as u8) << 5);
        track_io!(writer.write_u8(n))?;

        if let Some(ref x) = self.legal_time_window {
            track!(x.write_to(&mut writer))?;
        }
        if let Some(ref x) = self.piecewise_rate {
            track!(x.write_to(&mut writer))?;
        }
        if let Some(ref x) = self.seamless_splice {
            track!(x.write_to(&mut writer))?;
        }
        Ok(())
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
