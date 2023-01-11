//! Transport stream.
//!
//! # References
//!
//! - [MPEG transport stream](https://en.wikipedia.org/wiki/MPEG_transport_stream)
pub use self::adaptation_field::{AdaptationExtensionField, AdaptationField};
pub use self::packet::{TsHeader, TsPacket, TsPayload};
pub use self::pat::ProgramAssociation;
pub use self::pmt::{Descriptor, EsInfo};
pub use self::reader::{ReadTsPacket, TsPacketReader};
pub use self::types::{
    ContinuityCounter, LegalTimeWindow, Pid, PiecewiseRate, SeamlessSplice,
    TransportScramblingControl, VersionNumber,
};
pub use self::writer::{TsPacketWriter, WriteTsPacket};

pub mod payload {
    //! Transport stream payloads.

    pub use super::null::Null;
    pub use super::pat::Pat;
    pub use super::pes::Pes;
    pub use super::pmt::Pmt;
    pub use super::types::Bytes;
}

mod adaptation_field;
mod null;
mod packet;
mod pat;
mod pes;
mod pmt;
mod psi;
mod reader;
mod types;
mod writer;

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn pat() {
        let mut reader = TsPacketReader::new(pat_packet_bytes());
        let packet = track_try_unwrap!(reader.read_ts_packet()).unwrap();
        assert_eq!(packet, pat_packet());
        assert_eq!(track_try_unwrap!(reader.read_ts_packet()), None);

        let mut writer = TsPacketWriter::new(Vec::new());
        track_try_unwrap!(writer.write_ts_packet(&packet));

        let mut reader = TsPacketReader::new(&writer.stream()[..]);
        let packet = track_try_unwrap!(reader.read_ts_packet()).unwrap();
        assert_eq!(packet.header, pat_packet().header);
        assert_eq!(packet.payload, pat_packet().payload);
        assert_eq!(track_try_unwrap!(reader.read_ts_packet()), None);
    }

    fn pat_packet_bytes() -> &'static [u8] {
        &[
            71, 64, 0, 17, 0, 0, 176, 13, 0, 0, 195, 0, 0, 0, 1, 225, 224, 232, 95, 116, 236, 255,
            255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255,
            255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255,
            255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255,
            255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255,
            255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255,
            255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255,
            255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255,
            255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255,
            255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255,
            255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255,
        ][..]
    }

    fn pat_packet() -> TsPacket {
        TsPacket {
            header: TsHeader {
                transport_error_indicator: false,
                transport_priority: false,
                pid: Pid::from(0),
                transport_scrambling_control: TransportScramblingControl::NotScrambled,
                continuity_counter: ContinuityCounter::from_u8(1).unwrap(),
            },
            adaptation_field: None,
            payload: Some(TsPayload::Pat(payload::Pat {
                transport_stream_id: 0,
                version_number: VersionNumber::from_u8(1).unwrap(),
                table: vec![ProgramAssociation {
                    program_num: 1,
                    program_map_pid: Pid::new(480).unwrap(),
                }],
            })),
        }
    }

    #[test]
    fn pid17() {
        let mut reader = TsPacketReader::new(pid17_packet_bytes());
        let packet = track_try_unwrap!(reader.read_ts_packet()).unwrap();
        assert_eq!(packet.header.pid, Pid::from(17));
    }

    fn pid17_packet_bytes() -> &'static [u8] {
        &[
            71, 64, 17, 16, 0, 66, 240, 42, 0, 1, 193, 0, 0, 0, 1, 255, 0, 1, 252, 128, 25, 72, 23,
            1, 6, 70, 70, 109, 112, 101, 103, 14, 66, 105, 103, 32, 66, 117, 99, 107, 32, 66, 117,
            110, 110, 121, 182, 64, 83, 76, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255,
            255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255,
            255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255,
            255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255,
            255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255,
            255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255,
            255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255,
            255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255,
            255, 255, 255, 255, 255, 255, 255, 25,
        ][..]
    }
}
