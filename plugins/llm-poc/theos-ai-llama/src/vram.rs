//! Video memory in use by this process, read through DXGI.
//!
//! `IDXGIAdapter3::QueryVideoMemoryInfo` reports per-process usage on Windows,
//! where NVML's per-process figure reads "not available". Whether it sees CUDA
//! allocations is checked by `vram_probe_reads_more_after_loading_the_generator`.

use theos_ai::MemoryProbe;

/// PCI vendor id of NVIDIA.
pub const NVIDIA_VENDOR_ID: u32 = 0x10DE;

/// Reads the local-segment `CurrentUsage` of this process on the NVIDIA adapter.
pub struct DxgiProbe {
    #[cfg(windows)]
    adapter: windows::Win32::Graphics::Dxgi::IDXGIAdapter3,
}

#[cfg(windows)]
impl DxgiProbe {
    /// The probe for the first NVIDIA adapter, or `None` when there is none.
    pub fn new() -> Option<DxgiProbe> {
        use windows::core::Interface;
        use windows::Win32::Graphics::Dxgi::{CreateDXGIFactory1, IDXGIAdapter3, IDXGIFactory1};

        // SAFETY: CreateDXGIFactory1 has no preconditions and returns an owned interface.
        let factory: IDXGIFactory1 = unsafe { CreateDXGIFactory1() }.ok()?;
        let mut index = 0;
        // SAFETY: `factory` is a live interface; an index past the last adapter
        // returns DXGI_ERROR_NOT_FOUND, which ends the loop.
        while let Ok(adapter) = unsafe { factory.EnumAdapters1(index) } {
            index += 1;
            // SAFETY: `adapter` is a live interface; the description is returned by value.
            let Ok(desc) = (unsafe { adapter.GetDesc1() }) else {
                continue;
            };
            if desc.VendorId != NVIDIA_VENDOR_ID {
                continue;
            }
            if let Ok(adapter) = adapter.cast::<IDXGIAdapter3>() {
                return Some(DxgiProbe { adapter });
            }
        }
        None
    }
}

#[cfg(not(windows))]
impl DxgiProbe {
    /// DXGI exists only on Windows.
    pub fn new() -> Option<DxgiProbe> {
        None
    }
}

impl MemoryProbe for DxgiProbe {
    #[cfg(windows)]
    fn vram_bytes(&self) -> Option<u64> {
        use windows::Win32::Graphics::Dxgi::{
            DXGI_MEMORY_SEGMENT_GROUP_LOCAL, DXGI_QUERY_VIDEO_MEMORY_INFO,
        };

        let mut info = DXGI_QUERY_VIDEO_MEMORY_INFO::default();
        // SAFETY: `self.adapter` is a live interface owned by the probe, and `info`
        // is a writable out-parameter that outlives the call.
        unsafe {
            self.adapter
                .QueryVideoMemoryInfo(0, DXGI_MEMORY_SEGMENT_GROUP_LOCAL, &mut info)
        }
        .ok()?;
        Some(info.CurrentUsage)
    }

    #[cfg(not(windows))]
    fn vram_bytes(&self) -> Option<u64> {
        None
    }
}
