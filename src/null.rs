use std::io::Read;

use Result;

#[derive(Debug)]
pub struct Null;
impl Null {
    pub const PID: u16 = 0x1FFF;

    pub fn read_from<R: Read>(mut reader: R) -> Result<Self> {
        let mut buf = [0; 188];
        while 0 != track_io!(reader.read(&mut buf))? {}
        Ok(Null)
    }
}
