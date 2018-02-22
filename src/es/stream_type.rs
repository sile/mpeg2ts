use {ErrorKind, Result};

/// Elementary stream type.
#[allow(missing_docs)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum StreamType {
    Mpeg1Video = 0x01,
    Mpeg2Video = 0x02,
    Mpeg1Audio = 0x03,
    Mpeg2HalvedSampleRateAudio = 0x04,
    Mpeg2TabledData = 0x05,
    Mpeg2PacketizedData = 0x06,
    Mheg = 0x07,
    DsmCc = 0x08,
    AuxiliaryData09 = 0x09,
    DsmCcMultiprotocolEncapsulation = 0x0A,
    DsmCcUnMessages = 0x0B,
    DsmCcStreamDescriptors = 0x0C,
    DsmCcTabledData = 0x0D,
    AuxiliaryData0e = 0x0E,
    AdtsAac = 0x0F,
    Mpeg4H263BasedVideo = 0x10,
    Mpeg4LoasMultiFormatFramedAudio = 0x11,
    Mpeg4FlexMux = 0x12,
    Mpeg4FlexMuxInTable = 0x13,
    DsmCcSynchronizedDownloadProtocol = 0x14,
    PacketizedMetadata = 0x15,
    SectionedMetadata = 0x16,
    DsmCcDataCarouselMetadata = 0x17,
    DsmCcObjectCarouselMetadata = 0x18,
    SynchronizedDownloadProtocolMetadata = 0x19,
    Ipmp = 0x1A,
    H264 = 0x1B,
    H265 = 0x24,
    ChineseVideoStandard = 0x42,
    PcmAudio = 0x80,
    DolbyDigitalUpToSixChannelAudio = 0x81,
    Dts6ChannelAudio = 0x82,
    DolbyTrueHdLosslessAudio = 0x83,
    DolbyDigitalPlusUpTo16ChannelAudio = 0x84,
    Dts8ChannelAudio = 0x85,
    Dts8ChannelLosslessAudio = 0x86,
    DolbyDigitalPlusUpTo16ChannelAudioForAtsc = 0x87,
    PresentationGraphicStream = 0x90,
    AtscDsmCcNetworkResourcesTable = 0x91,
    DigiCipher2Text = 0xC0,
    DolbyDigitalUpToSixChannelAudioWithAes128Cbc = 0xC1,
    DolbyDigitalPlusUpToSixChannelAudioWithAes128Cbc = 0xC2,
    AdtsAacWithAes128Cbc = 0xCF,
    UltraHdVideo = 0xD1,
    H264WithAes128Cbc = 0xDB,
    MicrosoftWindowsMediaVideo9 = 0xEA,
}
impl StreamType {
    /// Makes a `StreamType` instance that associated with the given number.
    pub fn from_u8(n: u8) -> Result<Self> {
        Ok(match n {
            0x01 => StreamType::Mpeg1Video,
            0x02 => StreamType::Mpeg2Video,
            0x03 => StreamType::Mpeg1Audio,
            0x04 => StreamType::Mpeg2HalvedSampleRateAudio,
            0x05 => StreamType::Mpeg2TabledData,
            0x06 => StreamType::Mpeg2PacketizedData,
            0x07 => StreamType::Mheg,
            0x08 => StreamType::DsmCc,
            0x09 => StreamType::AuxiliaryData09,
            0x0A => StreamType::DsmCcMultiprotocolEncapsulation,
            0x0B => StreamType::DsmCcUnMessages,
            0x0C => StreamType::DsmCcStreamDescriptors,
            0x0D => StreamType::DsmCcTabledData,
            0x0E => StreamType::AuxiliaryData0e,
            0x0F => StreamType::AdtsAac,
            0x10 => StreamType::Mpeg4H263BasedVideo,
            0x11 => StreamType::Mpeg4LoasMultiFormatFramedAudio,
            0x12 => StreamType::Mpeg4FlexMux,
            0x13 => StreamType::Mpeg4FlexMuxInTable,
            0x14 => StreamType::DsmCcSynchronizedDownloadProtocol,
            0x15 => StreamType::PacketizedMetadata,
            0x16 => StreamType::SectionedMetadata,
            0x17 => StreamType::DsmCcDataCarouselMetadata,
            0x18 => StreamType::DsmCcObjectCarouselMetadata,
            0x19 => StreamType::SynchronizedDownloadProtocolMetadata,
            0x1A => StreamType::Ipmp,
            0x1B => StreamType::H264,
            0x24 => StreamType::H265,
            0x42 => StreamType::ChineseVideoStandard,
            0x80 => StreamType::PcmAudio,
            0x81 => StreamType::DolbyDigitalUpToSixChannelAudio,
            0x82 => StreamType::Dts6ChannelAudio,
            0x83 => StreamType::DolbyTrueHdLosslessAudio,
            0x84 => StreamType::DolbyDigitalPlusUpTo16ChannelAudio,
            0x85 => StreamType::Dts8ChannelAudio,
            0x86 => StreamType::Dts8ChannelLosslessAudio,
            0x87 => StreamType::DolbyDigitalPlusUpTo16ChannelAudioForAtsc,
            0x90 => StreamType::PresentationGraphicStream,
            0x91 => StreamType::AtscDsmCcNetworkResourcesTable,
            0xC0 => StreamType::DigiCipher2Text,
            0xC1 => StreamType::DolbyDigitalUpToSixChannelAudioWithAes128Cbc,
            0xC2 => StreamType::DolbyDigitalPlusUpToSixChannelAudioWithAes128Cbc,
            0xCF => StreamType::AdtsAacWithAes128Cbc,
            0xD1 => StreamType::UltraHdVideo,
            0xDB => StreamType::H264WithAes128Cbc,
            0xEA => StreamType::MicrosoftWindowsMediaVideo9,
            _ => track_panic!(ErrorKind::InvalidInput, "Unknown stream type: {}", n),
        })
    }
}