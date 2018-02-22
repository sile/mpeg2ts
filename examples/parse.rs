extern crate clap;
extern crate mpeg2ts;
#[macro_use]
extern crate trackable;

use clap::{App, Arg};
use mpeg2ts::ts::{ReadTsPacket, TsPacketReader};

fn main() {
    let matches = App::new("parse")
        .arg(
            Arg::with_name("OUTPUT_TYPE")
                .long("output-type")
                .takes_value(true)
                .possible_values(&["ts", "pes", "es"])
                .default_value("ts"),
        )
        .get_matches();
    match matches.value_of("OUTPUT_TYPE").unwrap() {
        "ts" => {
            let mut reader = TsPacketReader::new(std::io::stdin());
            while let Some(packet) = track_try_unwrap!(reader.read_ts_packet()) {
                println!("{:?}", packet);
            }
        }
        _ => unreachable!(),
    }
}
