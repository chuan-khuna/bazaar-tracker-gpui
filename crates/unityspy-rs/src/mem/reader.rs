//! The one door every Mono structure is read through, so tests can use byte buffers instead of a live game.

/// Reads raw bytes from some address space: a live process, or a buffer in a test.
pub trait Memory {
    /// Fill `buffer` with the bytes at `address`, or say why that failed.
    fn read(&self, address: u64, buffer: &mut [u8]) -> std::io::Result<()>;
}
