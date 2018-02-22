use {ErrorKind, Result};

/// Packet Identifier.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct Pid(u16);
impl Pid {
    /// Maximum PID value.
    pub const MAX: u16 = (1 << 13) - 1;

    /// PID of the Program Association Table (PAT) packet.
    pub const PAT: Pid = Pid(0);

    /// PID of the null packet.
    pub const NULL: Pid = Pid(0x1FFF);

    /// Makes a new `Pid` instance.
    ///
    /// # Errors
    ///
    /// If `pid` exceeds `Pid::MAX`, it will return an `ErrorKind::InvalidInput` error.
    pub fn new(pid: u16) -> Result<Self> {
        track_assert!(
            pid <= Self::MAX,
            ErrorKind::InvalidInput,
            "Too large PID: {}",
            pid
        );
        Ok(Pid(pid))
    }

    /// Returns the value of the `Pid`.
    pub fn as_u16(&self) -> u16 {
        self.0
    }
}
impl From<u8> for Pid {
    fn from(f: u8) -> Self {
        Pid(u16::from(f))
    }
}
