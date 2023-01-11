use crate::ts::payload::Bytes;
use crate::Result;
use std::io::Read;

/// Payload for null packets.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Null;
impl Null {
    pub(super) fn read_from<R: Read>(reader: R) -> Result<Self> {
        let _ = track!(Bytes::read_from(reader))?;
        Ok(Null)
    }
}
