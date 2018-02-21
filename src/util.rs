use std::io::Read;

use {ErrorKind, Result};

pub fn consume_stuffing_bytes<R: Read>(mut reader: R) -> Result<()> {
    let mut buf = [0];
    while 1 == track_io!(reader.read(&mut buf))? {
        track_assert_eq!(buf[0], 0xFF, ErrorKind::InvalidInput);
    }
    Ok(())
}
