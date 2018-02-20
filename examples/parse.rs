extern crate mpeg2ts;
#[macro_use]
extern crate trackable;

use mpeg2ts::packet::PacketReader;

fn main() {
    let mut reader = PacketReader::new(std::io::stdin());
    while let Some(packet) = track_try_unwrap!(reader.read_packet()) {
        println!("{:?}", packet);
    }
}
