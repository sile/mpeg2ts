use std::io::Read;

use Result;
use packet::Bytes;
use pes::PesHeader;

/// Payload for PES(Packetized elementary stream) packets.
#[allow(missing_docs)]
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Pes {
    pub header: PesHeader,
    pub pes_packet_len: u16,
    pub data: Bytes,
}
impl Pes {
    pub(super) fn read_from<R: Read>(mut reader: R) -> Result<Self> {
        let (header, pes_packet_len) = track!(PesHeader::read_from(&mut reader))?;
        let data = track!(Bytes::read_from(reader))?;
        Ok(Pes {
            header,
            pes_packet_len,
            data,
        })
    }
}
