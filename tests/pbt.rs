//! Property-based tests for mpeg2ts, driven by noprop.
//!
//! Coverage:
//!
//! - Full TS packet round-trip: header bits, adaptation field (PCR / OPCR /
//!   splice countdown / transport private data / adaptation extension) and
//!   raw payload must survive write then read byte-exactly.
//! - PAT / PMT / PES start payload round-trips through the PID state machine
//!   (`TsPacketReader` only parses payloads on PIDs registered by earlier
//!   PAT / PMT packets).
//! - Stateful PES reassembly: logical PES packets split across an arbitrary
//!   number of TS packets (start + continuations, interleaved with null
//!   packets) must be reassembled by `PesPacketReader` into exactly the
//!   generated packets.
//! - Error handling: unknown PID, truncated streams, a PES length shorter
//!   than its optional header, and DTS without PTS are all rejected.
//! - Feedback-guided reassembly: semantic buckets (total PES data length
//!   band, TS packet count band) steer the search toward the corners that
//!   uniform sampling would under-explore.
//!
//! PAT / PMT / PES tests compare header and payload only, not the whole
//! packet: the writer pads spare space with a stuffing adaptation field, so
//! a packet written without one reads back with a synthetic (all-flag-off)
//! adaptation field. The full-equality test therefore generates explicit
//! adaptation fields instead.
//!
//! `TsPayload::Section` is excluded: `TsPacketReader` never produces it, so
//! no round-trip is possible.
//!
//! Inputs are capped so each case stays small: at most 8 logical PES packets
//! of at most 4 KiB each, and PSI tables that always fit in one TS packet.

use std::cell::Cell;

use mpeg2ts::es::{StreamId, StreamType};
use mpeg2ts::pes::{PesHeader, PesPacketReader, ReadPesPacket};
use mpeg2ts::time::{ClockReference, Timestamp};
use mpeg2ts::ts::payload::{Bytes, Null, Pat, Pes, Pmt};
use mpeg2ts::ts::{
    AdaptationExtensionField, AdaptationField, ContinuityCounter, Descriptor, EsInfo,
    LegalTimeWindow, Pid, PiecewiseRate, ProgramAssociation, ReadTsPacket, SeamlessSplice,
    TransportScramblingControl, TsHeader, TsPacket, TsPacketReader, TsPacketWriter, TsPayload,
    VersionNumber, WriteTsPacket,
};
use noprop::TestCaseContext;

// --- Runner config ---------------------------------------------------

const CASES: usize = 64;
const MAX_PES_DATA: usize = 4096;

/// Seed from `MPEG2TS_PBT_SEED` (decimal or hex) when set, for deterministic
/// reproduction of a reported failure; a fresh time-derived seed otherwise.
fn seed() -> noprop::TestResult<u64> {
    noprop::seed_from_env_or_time("MPEG2TS_PBT_SEED")
}

fn run<F>(f: F) -> noprop::TestResult
where
    F: Fn(&mut TestCaseContext) -> noprop::TestResult,
{
    noprop::Runner::new(seed()?).run(CASES, f)?;
    Ok(())
}

fn run_feedback<F>(cases: usize, f: F) -> noprop::TestResult
where
    F: Fn(&mut TestCaseContext) -> noprop::TestResult,
{
    let mut runner = noprop::Runner::new(seed()?);
    runner.run_feedback_guided(cases, f)?;
    // Gate on the feedback machinery itself: if the buckets ever become
    // no-ops (e.g. someone switches this test back to plain `run`), the
    // search would silently lose its corner-concentrating behavior while
    // the reassembly assertion still passes.
    let stats = runner.stats();
    assert!(
        stats.discovered_features > 0,
        "feedback-guided run discovered no features; feedback is not active"
    );
    Ok(())
}

// --- Small value generators ------------------------------------------

fn sample_pid(ctx: &mut TestCaseContext) -> Pid {
    // Only 0x20..=0x1FFA is usable for payload PIDs: 0x01..=0x1F and 0x1FFB
    // are parsed as raw bytes, 0x0000 as PAT, and 0x1FFF as null packets.
    Pid::new(noprop::sample_usize_in(ctx, 0x20..=0x1FFA) as u16).expect("pid within range")
}

/// `n` consecutive PIDs so they are distinct by construction.
fn sample_pids(ctx: &mut TestCaseContext, n: usize) -> Vec<Pid> {
    let base = noprop::sample_usize_in(ctx, 0x20..=(0x1FFA - (n - 1)));
    (0..n)
        .map(|i| Pid::new((base + i) as u16).expect("pid within range"))
        .collect()
}

/// PID used for raw-byte payloads: any value the reader parses as
/// `TsPayload::Raw`.
fn sample_raw_pid(ctx: &mut TestCaseContext) -> Pid {
    if noprop::sample_ratio(ctx, noprop::Ratio::new(1, 8)) {
        Pid::new(0x1FFB).expect("0x1FFB is a valid pid")
    } else {
        Pid::from(noprop::sample_usize_in(ctx, 0x01..=0x1F) as u8)
    }
}

fn sample_timestamp(ctx: &mut TestCaseContext) -> Timestamp {
    let n = noprop::sample_with_boundaries(
        ctx,
        &[0u64, 1, Timestamp::MAX],
        noprop::Ratio::one_nth(4),
        |ctx| noprop::sample_u64(ctx) & ((1 << 33) - 1),
    );
    Timestamp::new(n).expect("masked draw stays within Timestamp::MAX")
}

fn sample_clock_reference(ctx: &mut TestCaseContext) -> ClockReference {
    let base = noprop::sample_u64(ctx) & ((1 << 33) - 1);
    let extension = noprop::sample_u64(ctx) & 0b1_1111_1111;
    ClockReference::new(base * 300 + extension)
        .expect("base and extension stay within ClockReference::MAX")
}

fn sample_version_number(ctx: &mut TestCaseContext) -> VersionNumber {
    VersionNumber::from_u8(noprop::sample_usize_in(ctx, 0..=31) as u8)
        .expect("draw stays within VersionNumber::MAX")
}

fn sample_stream_type(ctx: &mut TestCaseContext) -> StreamType {
    const STREAM_TYPES: &[StreamType] = &[
        StreamType::Mpeg1Video,
        StreamType::Mpeg2Video,
        StreamType::Mpeg1Audio,
        StreamType::Mpeg2PacketizedData,
        StreamType::Mheg,
        StreamType::AdtsAac,
        StreamType::Mpeg4H263BasedVideo,
        StreamType::H264,
        StreamType::H265,
        StreamType::PcmAudio,
        StreamType::Dts8ChannelLosslessAudio,
    ];
    noprop::sample_choice(ctx, STREAM_TYPES)
}

fn sample_pes_header(ctx: &mut TestCaseContext) -> PesHeader {
    let stream_id = if noprop::sample_bool(ctx) {
        StreamId::new_audio(noprop::sample_usize_in(
            ctx,
            usize::from(StreamId::AUDIO_MIN)..=usize::from(StreamId::AUDIO_MAX),
        ) as u8)
        .expect("audio stream id range")
    } else {
        StreamId::new_video(noprop::sample_usize_in(
            ctx,
            usize::from(StreamId::VIDEO_MIN)..=usize::from(StreamId::VIDEO_MAX),
        ) as u8)
        .expect("video stream id range")
    };
    // DTS is only generated together with PTS: the writer rejects DTS
    // without PTS (asserted separately in `dts_without_pts_rejected`).
    let (pts, dts, escr) = match noprop::sample_weighted_index(ctx, &[2, 3, 2, 1, 1]) {
        0 => (None, None, None),
        1 => (Some(sample_timestamp(ctx)), None, None),
        2 => (
            Some(sample_timestamp(ctx)),
            Some(sample_timestamp(ctx)),
            None,
        ),
        3 => (
            Some(sample_timestamp(ctx)),
            None,
            Some(sample_clock_reference(ctx)),
        ),
        _ => (
            Some(sample_timestamp(ctx)),
            Some(sample_timestamp(ctx)),
            Some(sample_clock_reference(ctx)),
        ),
    };
    PesHeader {
        stream_id,
        priority: noprop::sample_bool(ctx),
        data_alignment_indicator: noprop::sample_bool(ctx),
        copyright: noprop::sample_bool(ctx),
        original_or_copy: noprop::sample_bool(ctx),
        pts,
        dts,
        escr,
    }
}

/// Total bytes the PES header occupies in a TS payload
/// (packet start code prefix, stream id, packet length, and the optional
/// PES header fields).
fn pes_header_bytes(header: &PesHeader) -> usize {
    9 + 5 * usize::from(header.pts.is_some())
        + 5 * usize::from(header.dts.is_some())
        + 6 * usize::from(header.escr.is_some())
}

/// The PES `PES_packet_length` value for a packet with the given header and
/// data: the optional header length plus the data length.
fn pes_packet_length(header: &PesHeader, data_len: usize) -> u16 {
    let optional = 3
        + 5 * usize::from(header.pts.is_some())
        + 5 * usize::from(header.dts.is_some())
        + 6 * usize::from(header.escr.is_some());
    (optional + data_len) as u16
}

// --- PSI generators --------------------------------------------------

fn single_program_pat(program_map_pid: Pid) -> Pat {
    Pat {
        transport_stream_id: 0,
        version_number: VersionNumber::from_u8(0).expect("0 is a valid version"),
        table: vec![ProgramAssociation {
            program_num: 0,
            program_map_pid,
        }],
    }
}

fn sample_pat(ctx: &mut TestCaseContext) -> Pat {
    let n_entries =
        noprop::sample_with_boundaries(ctx, &[0usize, 8], noprop::Ratio::one_nth(4), |ctx| {
            noprop::sample_usize_in(ctx, 0..=8)
        });
    let mut table = Vec::with_capacity(n_entries);
    for _ in 0..n_entries {
        table.push(ProgramAssociation {
            program_num: noprop::sample_u16(ctx),
            program_map_pid: sample_pid(ctx),
        });
    }
    Pat {
        transport_stream_id: noprop::sample_u16(ctx),
        version_number: sample_version_number(ctx),
        table,
    }
}

/// Draws 0..=max_count descriptors, consuming at most `cap` bytes
/// (including each descriptor's 2-byte header) and never going below zero.
fn sample_descriptors(
    ctx: &mut TestCaseContext,
    cap: &mut usize,
    max_count: usize,
) -> Vec<Descriptor> {
    let mut descriptors = Vec::new();
    loop {
        if descriptors.len() >= max_count || *cap < 4 {
            break;
        }
        if !descriptors.is_empty() && !noprop::sample_ratio(ctx, noprop::Ratio::new(2, 3)) {
            break;
        }
        let len = noprop::sample_usize_in(ctx, 0..=(*cap - 2).min(255));
        let data = noprop::sample_bytes_vec(ctx, len);
        descriptors.push(Descriptor {
            tag: noprop::sample_u8(ctx),
            data,
        });
        *cap -= 2 + len;
    }
    descriptors
}

/// Draws a PMT whose PSI table always fits in one TS packet payload
/// (max 184 bytes) and whose ES list covers exactly `es_pids`.
///
/// The 5 bytes of every ES entry header are reserved up front, so the ES
/// list is registered even when the budget is exhausted by descriptors.
fn sample_pmt_with_es(ctx: &mut TestCaseContext, es_pids: &[Pid]) -> Pmt {
    let reserved = 5 * es_pids.len();
    let budget = noprop::sample_with_boundaries(
        ctx,
        &[reserved, 100, 167],
        noprop::Ratio::one_nth(4),
        |ctx| noprop::sample_usize_in(ctx, reserved..=167),
    );
    let mut shared = budget - reserved;
    let pcr_pid = if shared >= 4 && noprop::sample_ratio(ctx, noprop::Ratio::new(3, 4)) {
        shared -= 4;
        // 0x1FFF (no PCR) is written as all-ones, so the PID range of
        // `sample_pid` never collides with the no-PCR encoding.
        Some(sample_pid(ctx))
    } else {
        None
    };
    let program_info = sample_descriptors(ctx, &mut shared, 4);
    let mut es_info = Vec::new();
    for &pid in es_pids {
        let mut cap = shared;
        let descriptors = sample_descriptors(ctx, &mut cap, 4);
        es_info.push(EsInfo {
            stream_type: sample_stream_type(ctx),
            elementary_pid: pid,
            descriptors,
        });
        shared = cap;
    }
    Pmt {
        program_num: noprop::sample_u16(ctx),
        pcr_pid,
        version_number: sample_version_number(ctx),
        program_info,
        es_info,
    }
}

// --- TS packet generators --------------------------------------------

fn packet_with(pid: u16, payload: TsPayload) -> TsPacket {
    TsPacket {
        header: TsHeader {
            transport_error_indicator: false,
            transport_priority: false,
            pid: Pid::new(pid).expect("valid pid"),
            transport_scrambling_control: TransportScramblingControl::NotScrambled,
            continuity_counter: ContinuityCounter::from_u8(0).expect("0 is a valid counter"),
        },
        adaptation_field: None,
        payload: Some(payload),
    }
}

fn packet_with_cc(pid: u16, payload: TsPayload, cc: &mut u8) -> TsPacket {
    let mut packet = packet_with(pid, payload);
    packet.header.continuity_counter =
        ContinuityCounter::from_u8(*cc).expect("counter stays within 0..=15");
    *cc = (*cc + 1) & 0x0F;
    packet
}

/// Draws an adaptation field and consumes exactly its wire size from
/// `budget`. The base two bytes (adaptation field length and flags) are
/// consumed even when no optional field is present.
fn sample_adaptation_field(ctx: &mut TestCaseContext, budget: &mut usize) -> AdaptationField {
    *budget -= 2; // adaptation_field_length + flags
    let mut af = AdaptationField {
        discontinuity_indicator: noprop::sample_bool(ctx),
        random_access_indicator: noprop::sample_bool(ctx),
        es_priority_indicator: noprop::sample_bool(ctx),
        pcr: None,
        opcr: None,
        splice_countdown: None,
        transport_private_data: Vec::new(),
        extension: None,
    };
    if *budget >= 6 && noprop::sample_ratio(ctx, noprop::Ratio::new(1, 4)) {
        af.pcr = Some(sample_clock_reference(ctx));
        *budget -= 6;
    }
    if *budget >= 6 && noprop::sample_ratio(ctx, noprop::Ratio::new(1, 4)) {
        af.opcr = Some(sample_clock_reference(ctx));
        *budget -= 6;
    }
    if *budget >= 1 && noprop::sample_ratio(ctx, noprop::Ratio::new(1, 4)) {
        af.splice_countdown = Some(noprop::sample_i8(ctx));
        *budget -= 1;
    }
    if *budget >= 2 && noprop::sample_ratio(ctx, noprop::Ratio::new(1, 4)) {
        let len = noprop::sample_usize_in(ctx, 0..=(*budget - 1).min(255));
        af.transport_private_data = noprop::sample_bytes_vec(ctx, len);
        *budget -= 1 + len;
    }
    if *budget >= 7 && noprop::sample_ratio(ctx, noprop::Ratio::new(3, 4)) {
        *budget -= 2; // adaptation extension length + flags
        let mut extension = AdaptationExtensionField {
            legal_time_window: None,
            piecewise_rate: None,
            seamless_splice: None,
        };
        match noprop::sample_weighted_index(ctx, &[2, 3, 5]) {
            0 => {
                extension.legal_time_window = Some(
                    LegalTimeWindow::new(
                        noprop::sample_bool(ctx),
                        noprop::sample_usize_in(ctx, 0..=usize::from(LegalTimeWindow::MAX_OFFSET))
                            as u16,
                    )
                    .expect("offset within max"),
                );
                *budget -= 2;
            }
            1 => {
                extension.piecewise_rate = Some(
                    PiecewiseRate::new(
                        noprop::sample_usize_in(ctx, 0..=PiecewiseRate::MAX as usize) as u32,
                    )
                    .expect("rate within max"),
                );
                *budget -= 3;
            }
            _ => {
                extension.seamless_splice = Some(
                    SeamlessSplice::new(
                        noprop::sample_usize_in(
                            ctx,
                            0..=usize::from(SeamlessSplice::MAX_SPLICE_TYPE),
                        ) as u8,
                        sample_timestamp(ctx),
                    )
                    .expect("splice type within max"),
                );
                *budget -= 5;
            }
        }
        if extension.legal_time_window.is_none()
            && *budget >= 2
            && noprop::sample_ratio(ctx, noprop::Ratio::new(1, 2))
        {
            extension.legal_time_window = Some(
                LegalTimeWindow::new(
                    noprop::sample_bool(ctx),
                    noprop::sample_usize_in(ctx, 0..=usize::from(LegalTimeWindow::MAX_OFFSET))
                        as u16,
                )
                .expect("offset within max"),
            );
            *budget -= 2;
        }
        if extension.piecewise_rate.is_none()
            && *budget >= 3
            && noprop::sample_ratio(ctx, noprop::Ratio::new(1, 2))
        {
            extension.piecewise_rate = Some(
                PiecewiseRate::new(
                    noprop::sample_usize_in(ctx, 0..=PiecewiseRate::MAX as usize) as u32,
                )
                .expect("rate within max"),
            );
            *budget -= 3;
        }
        if extension.seamless_splice.is_none()
            && *budget >= 5
            && noprop::sample_ratio(ctx, noprop::Ratio::new(1, 2))
        {
            extension.seamless_splice = Some(
                SeamlessSplice::new(
                    noprop::sample_usize_in(ctx, 0..=usize::from(SeamlessSplice::MAX_SPLICE_TYPE))
                        as u8,
                    sample_timestamp(ctx),
                )
                .expect("splice type within max"),
            );
            *budget -= 5;
        }
        af.extension = Some(extension);
    }
    af
}

// --- Reassembly helpers ----------------------------------------------

struct LogicalPes {
    header: PesHeader,
    data: Vec<u8>,
    n_ts_packets: usize,
}

/// Generates a TS stream of PAT + PMT + `n_logical` PES packets split into
/// arbitrary chunks, with occasional null packets interleaved. Returns the
/// written bytes and the expected logical PES packets.
fn generate_reassembly_stream(ctx: &mut TestCaseContext) -> (Vec<u8>, Vec<LogicalPes>) {
    let pids = sample_pids(ctx, 2);
    let n_logical =
        noprop::sample_with_boundaries(ctx, &[1usize, 8], noprop::Ratio::one_nth(4), |ctx| {
            noprop::sample_usize_in(ctx, 1..=8)
        });
    let mut writer = TsPacketWriter::new(Vec::new());
    writer
        .write_ts_packet(&packet_with(0, TsPayload::Pat(single_program_pat(pids[0]))))
        .expect("PAT write");
    let pmt = sample_pmt_with_es(ctx, &[pids[1]]);
    writer
        .write_ts_packet(&packet_with(pids[0].as_u16(), TsPayload::Pmt(pmt)))
        .expect("PMT write");
    let mut cc = 0u8;
    let mut logical = Vec::new();
    for _ in 0..n_logical {
        let header = sample_pes_header(ctx);
        let start_capacity = 184 - pes_header_bytes(&header);
        let data_len = noprop::sample_with_boundaries(
            ctx,
            &[0usize, 1, MAX_PES_DATA],
            noprop::Ratio::one_nth(4),
            |ctx| noprop::sample_usize_in(ctx, 0..=MAX_PES_DATA),
        );
        let data = noprop::sample_bytes_vec(ctx, data_len);
        let chunk0 = if data.is_empty() {
            0
        } else {
            noprop::sample_with_boundaries(
                ctx,
                &[1usize, start_capacity],
                noprop::Ratio::one_nth(4),
                |ctx| noprop::sample_usize_in(ctx, 1..=start_capacity),
            )
            .min(data.len())
        };
        let start = Pes {
            header: header.clone(),
            pes_packet_len: pes_packet_length(&header, data.len()),
            data: Bytes::new(&data[..chunk0]).expect("chunk0 fits in a TS payload"),
        };
        writer
            .write_ts_packet(&packet_with_cc(
                pids[1].as_u16(),
                TsPayload::PesStart(start),
                &mut cc,
            ))
            .expect("PES start write");
        let mut n_ts_packets = 1;
        let mut offset = chunk0;
        while offset < data.len() {
            let remaining = data.len() - offset;
            let chunk = noprop::sample_with_boundaries(
                ctx,
                &[1usize, 184],
                noprop::Ratio::one_nth(4),
                |ctx| noprop::sample_usize_in(ctx, 1..=184),
            )
            .min(remaining);
            let continuation = Bytes::new(&data[offset..offset + chunk])
                .expect("continuation fits in a TS payload");
            writer
                .write_ts_packet(&packet_with_cc(
                    pids[1].as_u16(),
                    TsPayload::PesContinuation(continuation),
                    &mut cc,
                ))
                .expect("PES continuation write");
            offset += chunk;
            n_ts_packets += 1;
            if noprop::sample_ratio(ctx, noprop::Ratio::new(1, 5)) {
                writer
                    .write_ts_packet(&packet_with(0x1FFF, TsPayload::Null(Null)))
                    .expect("null write");
            }
        }
        logical.push(LogicalPes {
            header,
            data,
            n_ts_packets,
        });
    }
    (writer.into_stream(), logical)
}

fn verify_reassembly(bytes: &[u8], logical: &[LogicalPes]) -> noprop::TestResult {
    let mut reader = PesPacketReader::new(TsPacketReader::new(bytes));
    let mut received = Vec::new();
    while let Some(packet) = reader.read_pes_packet()? {
        received.push(packet);
    }
    assert_eq!(
        received.len(),
        logical.len(),
        "reader returned {} of {} generated PES packets",
        received.len(),
        logical.len(),
    );
    for (i, (expected, actual)) in logical.iter().zip(&received).enumerate() {
        assert_eq!(
            actual.header, expected.header,
            "PES header mismatch at packet {i}"
        );
        assert_eq!(
            actual.data, expected.data,
            "PES data mismatch at packet {i}"
        );
    }
    Ok(())
}

// --- Full TS packet round-trip --------------------------------------

#[test]
fn ts_packet_roundtrip_matches_write() -> noprop::TestResult {
    let saw_pcr = Cell::new(false);
    let saw_opcr = Cell::new(false);
    let saw_splice = Cell::new(false);
    let saw_private_data = Cell::new(false);
    let saw_extension = Cell::new(false);
    run(|ctx| {
        let af_budget = noprop::sample_with_boundaries(
            ctx,
            &[2usize, 50, 183],
            noprop::Ratio::one_nth(4),
            |ctx| noprop::sample_usize_in(ctx, 2..=183),
        );
        let mut budget = af_budget;
        let adaptation_field = sample_adaptation_field(ctx, &mut budget);
        let af_ext = af_budget - budget;
        let max_payload = 184 - af_ext;
        let payload_len = noprop::sample_with_boundaries(
            ctx,
            &[0usize, max_payload],
            noprop::Ratio::one_nth(4),
            |ctx| noprop::sample_usize_in(ctx, 0..=max_payload),
        );
        let packet = TsPacket {
            header: TsHeader {
                transport_error_indicator: noprop::sample_bool(ctx),
                transport_priority: noprop::sample_bool(ctx),
                pid: sample_raw_pid(ctx),
                transport_scrambling_control: noprop::sample_choice(
                    ctx,
                    &[
                        TransportScramblingControl::NotScrambled,
                        TransportScramblingControl::ScrambledWithEvenKey,
                        TransportScramblingControl::ScrambledWithOddKey,
                    ],
                ),
                continuity_counter: ContinuityCounter::from_u8(
                    noprop::sample_usize_in(ctx, 0..=15) as u8,
                )
                .expect("counter stays within 0..=15"),
            },
            adaptation_field: Some(adaptation_field.clone()),
            payload: Some(TsPayload::Raw(
                Bytes::new(&noprop::sample_bytes_vec(ctx, payload_len))
                    .expect("payload fits in Bytes::MAX_SIZE"),
            )),
        };
        let af = packet
            .adaptation_field
            .as_ref()
            .expect("adaptation field present");
        saw_pcr.set(saw_pcr.get() || af.pcr.is_some());
        saw_opcr.set(saw_opcr.get() || af.opcr.is_some());
        saw_splice.set(saw_splice.get() || af.splice_countdown.is_some());
        saw_private_data.set(saw_private_data.get() || !af.transport_private_data.is_empty());
        saw_extension.set(saw_extension.get() || af.extension.is_some());

        let mut writer = TsPacketWriter::new(Vec::new());
        writer.write_ts_packet(&packet)?;
        let bytes = writer.into_stream();
        let mut reader = TsPacketReader::new(&bytes[..]);
        let read_back = reader.read_ts_packet()?.expect("one packet written");
        assert_eq!(
            read_back, packet,
            "full packet round-trip must preserve header, adaptation field, and payload"
        );
        assert!(reader.read_ts_packet()?.is_none(), "no further packets");
        Ok(())
    })?;
    assert!(saw_pcr.get(), "no case wrote a PCR");
    assert!(saw_opcr.get(), "no case wrote an OPCR");
    assert!(saw_splice.get(), "no case wrote a splice countdown");
    assert!(
        saw_private_data.get(),
        "no case wrote transport private data"
    );
    assert!(saw_extension.get(), "no case wrote an adaptation extension");
    Ok(())
}

// --- PSI round-trips ------------------------------------------------

#[test]
fn pat_roundtrip_matches_write() -> noprop::TestResult {
    let saw_empty = Cell::new(false);
    let saw_full = Cell::new(false);
    run(|ctx| {
        let pat = sample_pat(ctx);
        saw_empty.set(saw_empty.get() || pat.table.is_empty());
        saw_full.set(saw_full.get() || pat.table.len() == 8);
        let packet = packet_with(0, TsPayload::Pat(pat));
        let mut writer = TsPacketWriter::new(Vec::new());
        writer.write_ts_packet(&packet)?;
        let bytes = writer.into_stream();
        let mut reader = TsPacketReader::new(&bytes[..]);
        let read_back = reader.read_ts_packet()?.expect("one packet written");
        assert_eq!(
            read_back.header, packet.header,
            "PAT header must round-trip"
        );
        assert_eq!(
            read_back.payload, packet.payload,
            "PAT payload must round-trip"
        );
        assert!(reader.read_ts_packet()?.is_none(), "no further packets");
        Ok(())
    })?;
    assert!(saw_empty.get(), "no case generated an empty PAT table");
    assert!(saw_full.get(), "no case generated an 8-entry PAT table");
    Ok(())
}

#[test]
fn pmt_roundtrip_matches_write() -> noprop::TestResult {
    let saw_pcr_pid = Cell::new(false);
    let saw_descriptor = Cell::new(false);
    run(|ctx| {
        let n_es =
            noprop::sample_with_boundaries(ctx, &[1usize, 3], noprop::Ratio::one_nth(4), |ctx| {
                noprop::sample_usize_in(ctx, 1..=3)
            });
        let pids = sample_pids(ctx, n_es + 1);
        let pmt = sample_pmt_with_es(ctx, &pids[1..]);
        saw_pcr_pid.set(saw_pcr_pid.get() || pmt.pcr_pid.is_some());
        saw_descriptor.set(
            saw_descriptor.get()
                || pmt.program_info.iter().any(|d| !d.data.is_empty())
                || pmt
                    .es_info
                    .iter()
                    .any(|e| e.descriptors.iter().any(|d| !d.data.is_empty())),
        );
        let pat_packet = packet_with(0, TsPayload::Pat(single_program_pat(pids[0])));
        let pmt_packet = packet_with(pids[0].as_u16(), TsPayload::Pmt(pmt));
        let mut writer = TsPacketWriter::new(Vec::new());
        writer.write_ts_packet(&pat_packet)?;
        writer.write_ts_packet(&pmt_packet)?;
        let bytes = writer.into_stream();
        let mut reader = TsPacketReader::new(&bytes[..]);
        let read_pat = reader.read_ts_packet()?.expect("PAT packet");
        let read_pmt = reader.read_ts_packet()?.expect("PMT packet");
        assert_eq!(
            read_pat.header, pat_packet.header,
            "PAT header must round-trip"
        );
        assert_eq!(
            read_pat.payload, pat_packet.payload,
            "PAT payload must round-trip"
        );
        assert_eq!(
            read_pmt.header, pmt_packet.header,
            "PMT header must round-trip"
        );
        assert_eq!(
            read_pmt.payload, pmt_packet.payload,
            "PMT payload must round-trip"
        );
        assert!(reader.read_ts_packet()?.is_none(), "no further packets");
        Ok(())
    })?;
    assert!(saw_pcr_pid.get(), "no case wrote a PCR PID");
    assert!(saw_descriptor.get(), "no case wrote a descriptor with data");
    Ok(())
}

#[test]
fn pes_start_roundtrip_matches_write() -> noprop::TestResult {
    let saw_dts = Cell::new(false);
    let saw_escr = Cell::new(false);
    run(|ctx| {
        let pids = sample_pids(ctx, 2);
        let pes = sample_pes(ctx);
        saw_dts.set(saw_dts.get() || pes.header.dts.is_some());
        saw_escr.set(saw_escr.get() || pes.header.escr.is_some());
        let pat_packet = packet_with(0, TsPayload::Pat(single_program_pat(pids[0])));
        let pmt = sample_pmt_with_es(ctx, &[pids[1]]);
        let pmt_packet = packet_with(pids[0].as_u16(), TsPayload::Pmt(pmt));
        let pes_packet = packet_with(pids[1].as_u16(), TsPayload::PesStart(pes));
        let mut writer = TsPacketWriter::new(Vec::new());
        writer.write_ts_packet(&pat_packet)?;
        writer.write_ts_packet(&pmt_packet)?;
        writer.write_ts_packet(&pes_packet)?;
        let bytes = writer.into_stream();
        let mut reader = TsPacketReader::new(&bytes[..]);
        reader.read_ts_packet()?.expect("PAT packet");
        reader.read_ts_packet()?.expect("PMT packet");
        let read_pes = reader.read_ts_packet()?.expect("PES packet");
        assert_eq!(
            read_pes.header, pes_packet.header,
            "PES header must round-trip"
        );
        assert_eq!(
            read_pes.payload, pes_packet.payload,
            "PES payload must round-trip"
        );
        assert!(reader.read_ts_packet()?.is_none(), "no further packets");
        Ok(())
    })?;
    assert!(saw_dts.get(), "no case wrote a PES header with DTS");
    assert!(saw_escr.get(), "no case wrote a PES header with ESCR");
    Ok(())
}

fn sample_pes(ctx: &mut TestCaseContext) -> Pes {
    let header = sample_pes_header(ctx);
    let max_data = 184 - pes_header_bytes(&header);
    let data_len = noprop::sample_with_boundaries(
        ctx,
        &[0usize, max_data],
        noprop::Ratio::one_nth(4),
        |ctx| noprop::sample_usize_in(ctx, 0..=max_data),
    );
    // `PES_packet_length` is opaque to the TS-level round-trip, so any
    // value works; the 0 boundary exercises the unbounded encoding.
    let pes_packet_len = noprop::sample_with_boundaries(
        ctx,
        &[0usize, u16::MAX as usize],
        noprop::Ratio::one_nth(4),
        |ctx| noprop::sample_u16(ctx) as usize,
    ) as u16;
    Pes {
        header,
        pes_packet_len,
        data: Bytes::new(&noprop::sample_bytes_vec(ctx, data_len))
            .expect("data fits in Bytes::MAX_SIZE"),
    }
}

// --- Stateful PES reassembly -----------------------------------------

#[test]
fn pes_reassembly_matches_model() -> noprop::TestResult {
    let saw_split = Cell::new(false);
    let saw_empty = Cell::new(false);
    run(|ctx| {
        let (bytes, logical) = generate_reassembly_stream(ctx);
        for packet in &logical {
            saw_split.set(saw_split.get() || packet.n_ts_packets > 1);
            saw_empty.set(saw_empty.get() || packet.data.is_empty());
        }
        verify_reassembly(&bytes, &logical)?;
        Ok(())
    })?;
    assert!(
        saw_split.get(),
        "no case split a PES packet across multiple TS packets"
    );
    assert!(saw_empty.get(), "no case generated an empty PES packet");
    Ok(())
}

// --- Error handling ---------------------------------------------------

#[test]
fn unregistered_pid_rejected() -> noprop::TestResult {
    run(|ctx| {
        let pids = sample_pids(ctx, 3);
        let pat_packet = packet_with(0, TsPayload::Pat(single_program_pat(pids[0])));
        let pmt = sample_pmt_with_es(ctx, &[pids[1]]);
        let pmt_packet = packet_with(pids[0].as_u16(), TsPayload::Pmt(pmt));
        let pes_packet = packet_with(
            pids[2].as_u16(),
            TsPayload::PesStart(Pes {
                header: sample_pes_header(ctx),
                pes_packet_len: 0,
                data: Bytes::new(&[]).expect("empty data fits"),
            }),
        );
        let mut writer = TsPacketWriter::new(Vec::new());
        writer.write_ts_packet(&pat_packet)?;
        writer.write_ts_packet(&pmt_packet)?;
        writer.write_ts_packet(&pes_packet)?;
        let bytes = writer.into_stream();
        let mut reader = TsPacketReader::new(&bytes[..]);
        reader.read_ts_packet()?.expect("PAT packet");
        reader.read_ts_packet()?.expect("PMT packet");
        assert!(
            reader.read_ts_packet().is_err(),
            "a PES packet on an unregistered PID must be rejected"
        );
        Ok(())
    })
}

#[test]
fn truncated_stream_rejected() -> noprop::TestResult {
    run(|ctx| {
        let total = noprop::sample_usize_in(ctx, 1..=187);
        let mut bytes = Vec::with_capacity(total);
        bytes.push(if noprop::sample_ratio(ctx, noprop::Ratio::new(3, 4)) {
            0x47
        } else {
            noprop::sample_u8(ctx)
        });
        bytes.extend(noprop::sample_bytes_vec(ctx, total - 1));
        let mut reader = TsPacketReader::new(&bytes[..]);
        assert!(
            reader.read_ts_packet().is_err(),
            "a truncated TS packet must be rejected"
        );
        Ok(())
    })
}

#[test]
fn pes_packet_len_shorter_than_header_rejected() -> noprop::TestResult {
    run(|ctx| {
        let pids = sample_pids(ctx, 2);
        let header = sample_pes_header(ctx);
        let optional = 3
            + 5 * usize::from(header.pts.is_some())
            + 5 * usize::from(header.dts.is_some())
            + 6 * usize::from(header.escr.is_some());
        // `PES_packet_length` = 0 means unbounded, so a rejection needs a
        // length in 1..optional.
        let pes_packet_len = noprop::sample_usize_in(ctx, 1..optional) as u16;
        let pat_packet = packet_with(0, TsPayload::Pat(single_program_pat(pids[0])));
        let pmt = sample_pmt_with_es(ctx, &[pids[1]]);
        let pmt_packet = packet_with(pids[0].as_u16(), TsPayload::Pmt(pmt));
        let pes_packet = packet_with(
            pids[1].as_u16(),
            TsPayload::PesStart(Pes {
                header,
                pes_packet_len,
                data: Bytes::new(&[]).expect("empty data fits"),
            }),
        );
        let mut writer = TsPacketWriter::new(Vec::new());
        writer.write_ts_packet(&pat_packet)?;
        writer.write_ts_packet(&pmt_packet)?;
        writer.write_ts_packet(&pes_packet)?;
        let bytes = writer.into_stream();
        let mut reader = PesPacketReader::new(TsPacketReader::new(&bytes[..]));
        assert!(
            reader.read_pes_packet().is_err(),
            "a PES packet whose length is shorter than its optional header must be rejected"
        );
        Ok(())
    })
}

#[test]
fn dts_without_pts_rejected() -> noprop::TestResult {
    run(|ctx| {
        let mut header = sample_pes_header(ctx);
        header.pts = None;
        header.dts = Some(sample_timestamp(ctx));
        let packet = packet_with(
            sample_pid(ctx).as_u16(),
            TsPayload::PesStart(Pes {
                header,
                pes_packet_len: 0,
                data: Bytes::new(&[]).expect("empty data fits"),
            }),
        );
        let mut writer = TsPacketWriter::new(Vec::new());
        assert!(
            writer.write_ts_packet(&packet).is_err(),
            "writing DTS without PTS must be rejected"
        );
        Ok(())
    })
}

// --- Feedback-guided reassembly --------------------------------------

#[test]
fn feedback_guided_pes_reassembly() -> noprop::TestResult {
    run_feedback(64, |ctx| {
        let (bytes, logical) = generate_reassembly_stream(ctx);
        // Total PES data length band: 0 / 1..=64 / 65..=1024 / 1025..=
        // (multiple TS packets each).
        let total_data: usize = logical.iter().map(|p| p.data.len()).sum();
        let data_band = match total_data {
            0 => 0u64,
            1..=64 => 1,
            65..=1024 => 2,
            _ => 3,
        };
        ctx.bucket("pes_total_data_band", data_band);
        // TS packet count band: a packet fully inside its start TS packet
        // is the cheap corner; every further TS packet costs a write.
        let n_ts_packets: usize = logical.iter().map(|p| p.n_ts_packets).sum();
        let packets_band = match n_ts_packets {
            1 => 0u64,
            2..=8 => 1,
            9..=32 => 2,
            _ => 3,
        };
        ctx.bucket("ts_packets_band", packets_band);
        verify_reassembly(&bytes, &logical)
    })
}
