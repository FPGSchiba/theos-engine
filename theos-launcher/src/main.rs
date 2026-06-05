#![forbid(unsafe_code)]

use theos_core::{get_engine_version, logging, TheosKernel};

fn main() {
    logging::init();
    let version = get_engine_version();
    // AC2: Required output string
    tracing::info!("Theos Engine v{} - Kernel starting...", version);

    let mut kernel = TheosKernel::default();
    kernel.set_target_hz(None);
    kernel.init().unwrap();

    if std::env::var("THEOS_DRY_RUN").is_ok() {
        kernel.request_shutdown().unwrap();
    }

    kernel.run().unwrap();
}
