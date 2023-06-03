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
    use crate::es::StreamType;

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
    fn pmt() {
        let mut bytes = Vec::new();
        bytes.extend(pat_packet_bytes());
        bytes.extend(pmt_packet_bytes());
        let mut reader = TsPacketReader::new(&bytes[..]);
        let mut writer = TsPacketWriter::new(Vec::new());

        let packet = track_try_unwrap!(reader.read_ts_packet()).unwrap();
        track_try_unwrap!(writer.write_ts_packet(&packet));

        let packet = track_try_unwrap!(reader.read_ts_packet()).unwrap();
        assert_eq!(packet.header, pmt_packet().header);
        assert_eq!(packet.payload, pmt_packet().payload);
        track_try_unwrap!(writer.write_ts_packet(&packet));

        let mut reader = TsPacketReader::new(&writer.stream()[..]);
        track_try_unwrap!(reader.read_ts_packet()).unwrap();
        let packet = track_try_unwrap!(reader.read_ts_packet()).unwrap();
        assert_eq!(packet.header, pmt_packet().header);
        assert_eq!(packet.payload, pmt_packet().payload);
        assert_eq!(track_try_unwrap!(reader.read_ts_packet()), None);
    }

    fn pmt_packet_bytes() -> &'static [u8] {
        &[
            71, 65, 224, 48, 0, 0, 2, 176, 34, 0, 1, 193, 0, 0, 225, 2, 240, 6, 5, 4, 67, 85, 69,
            73, 134, 225, 3, 240, 0, 15, 225, 1, 240, 0, 27, 225, 2, 240, 0, 225, 243, 90, 60, 255,
            255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255,
            255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255,
            255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255,
            255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255,
            255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255,
            255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255,
            255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255,
            255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255,
            255, 255, 255, 255, 255, 255, 255, 255,
        ][..]
    }

    fn pmt_packet() -> TsPacket {
        TsPacket {
            header: TsHeader {
                transport_error_indicator: false,
                transport_priority: false,
                pid: Pid::new(480).unwrap(),
                transport_scrambling_control: TransportScramblingControl::NotScrambled,
                continuity_counter: ContinuityCounter::from_u8(0).unwrap(),
            },
            adaptation_field: None,
            payload: Some(TsPayload::Pmt(payload::Pmt {
                program_num: 1,
                pcr_pid: Some(Pid::new(258).unwrap()),
                version_number: VersionNumber::new(),
                program_info: vec![Descriptor {
                    tag: 5,
                    data: b"CUEI".to_vec(),
                }],
                es_info: vec![
                    EsInfo {
                        stream_type: StreamType::Dts8ChannelLosslessAudio,
                        elementary_pid: Pid::new(259).unwrap(),
                        descriptors: vec![],
                    },
                    EsInfo {
                        stream_type: StreamType::AdtsAac,
                        elementary_pid: Pid::new(257).unwrap(),
                        descriptors: vec![],
                    },
                    EsInfo {
                        stream_type: StreamType::H264,
                        elementary_pid: Pid::new(258).unwrap(),
                        descriptors: vec![],
                    },
                ],
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
