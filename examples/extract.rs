extern crate clap;
extern crate mpeg2ts;
#[macro_use]
extern crate trackable;

use std::io::Write;
use clap::{App, Arg};
use mpeg2ts::es::EsFrameReader;
use mpeg2ts::packet::PacketReader;
use trackable::error::Failure;

fn main() {
    let matches = App::new("extract")
        .arg(
            Arg::with_name("STREAM_ID")
                .long("stream-id")
                .takes_value(true),
        )
        .get_matches();
    let mut stream_id: Option<u8> = matches
        .value_of("STREAM_ID")
        .map(|s| track_try_unwrap!(s.parse().map_err(Failure::from_error)));
    let mut reader = EsFrameReader::new(PacketReader::new(std::io::stdin()));
    while let Some(frame) = track_try_unwrap!(reader.read_es_frame()) {
        if stream_id.is_none() {
            stream_id = Some(frame.stream_id.as_u8());
        }
        if stream_id != Some(frame.stream_id.as_u8()) {
            continue;
        }
        track_try_unwrap!(
            std::io::stdout()
                .write_all(&frame.data)
                .map_err(Failure::from_error)
        );
    }
}
