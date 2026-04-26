#![forbid(unsafe_code)]

use theos_shared::VERSION;

pub fn get_engine_version() -> &'static str {
    VERSION
}

