#![forbid(unsafe_code)]

use theos_core::logging;

fn main() {
    logging::init();

    tracing::info!("Hello, world!");
}
