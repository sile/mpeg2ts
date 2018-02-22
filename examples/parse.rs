// extern crate clap;
// extern crate mpeg2ts;
// #[macro_use]
// extern crate trackable;

// use clap::{App, Arg};
// use mpeg2ts::es::EsFrameReader;
// use mpeg2ts::packet::PacketReader;

fn main() {
    // let matches = App::new("pares")
    //     .arg(
    //         Arg::with_name("TYPE")
    //             .long("type")
    //             .takes_value(true)
    //             .possible_values(&["es", "packet"])
    //             .default_value("packet"),
    //     )
    //     .get_matches();
    // match matches.value_of("TYPE").unwrap() {
    //     "packet" => {
    //         let mut reader = PacketReader::new(std::io::stdin());
    //         while let Some(packet) = track_try_unwrap!(reader.read_packet()) {
    //             println!("{:?}", packet);
    //         }
    //     }
    //     "es" => {
    //         let mut reader = EsFrameReader::new(PacketReader::new(std::io::stdin()));
    //         while let Some(frame) = track_try_unwrap!(reader.read_es_frame()) {
    //             println!(
    //                 "stream_id={:?}, pts={:?}, dts={:?}, data.len={}",
    //                 frame.stream_id,
    //                 frame.pts,
    //                 frame.dts,
    //                 frame.data.len()
    //             );
    //         }
    //     }
    //     _ => unreachable!(),
    // }
}
