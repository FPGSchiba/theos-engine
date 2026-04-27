#![forbid(unsafe_code)]

use theos_core::{get_engine_version, logging};

fn main() {
    logging::init();

    let version = get_engine_version();
    // AC2: Required output string
    tracing::info!("Theos Engine v{} - Kernel starting...", version);
}
