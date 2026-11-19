/// The addressed device did not answer (no RPLY on the Q-bus). The CPU turns
/// this into a trap to vector 4.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct BusError;

/// The direction of a bus cycle's data transfer, for [`Bus::cycle`]. A
/// read-modify-write (DATIO) cycle is reported as its read, then its write.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Cycle {
    Read,
    Write,
}

/// The CPU's view of the system bus.
///
/// Word accesses are always made at even addresses: the VM1 ignores address
/// bit 0 for word cycles, and the CPU clears it before calling the bus.
pub trait Bus {
    fn read_word(&mut self, addr: u16) -> Result<u16, BusError>;
    fn write_word(&mut self, addr: u16, value: u16) -> Result<(), BusError>;
    fn read_byte(&mut self, addr: u16) -> Result<u8, BusError>;
    fn write_byte(&mut self, addr: u16, value: u8) -> Result<(), BusError>;

    /// A bus cycle's data strobe is about to go out: `at` CPU clocks into the
    /// current [`crate::Cpu::step`], addressing `addr`. Returns the clocks
    /// until the device replies (0: at once). Called just before the read or
    /// write it times, so a bus that models time can bring its devices up to
    /// that moment (a tape input sampled at the exact clock of the read, a
    /// memory controller's slots). The default replies at once.
    fn cycle(&mut self, _at: u32, _addr: u16, _kind: Cycle) -> u32 {
        0
    }

    /// The `RESET` instruction asserts INIT: devices return to their power-on
    /// state and withdraw any interrupt requests.
    fn reset_devices(&mut self) {}

    /// Vectored interrupt (VIRQ) acknowledge. Called only when the CPU is about
    /// to take a VIRQ. Return the vector of the highest-priority device that is
    /// requesting, and clear that device's request. Return `None` if no device
    /// is requesting.
    fn acknowledge_interrupt(&mut self) -> Option<u16> {
        None
    }
}
