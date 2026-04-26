#![forbid(unsafe_code)]

use theos_core::get_engine_version;

fn main() {
    let version = get_engine_version();
    // AC2: Required output string
    println!("Theos Engine v{} - Kernel starting...", version);

    std::process::exit(0);
}

