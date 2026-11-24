//! Optional measurement of video memory in use.

/// Reports how much video memory the process is using, when it can tell.
pub trait MemoryProbe {
    /// Bytes of video memory in use, or `None` when unknown.
    fn vram_bytes(&self) -> Option<u64>;
}

/// A probe that never knows.
#[derive(Clone, Copy, Debug, Default)]
pub struct NoProbe;

impl MemoryProbe for NoProbe {
    fn vram_bytes(&self) -> Option<u64> {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn no_probe_reports_nothing() {
        assert_eq!(NoProbe.vram_bytes(), None);
    }
}
