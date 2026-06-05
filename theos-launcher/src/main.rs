#![forbid(unsafe_code)]

fn main() {
    let mut kernel = theos_core::TheosKernel::default();
    kernel.init().unwrap();
    kernel.run().unwrap();
    kernel.set_target_hz(None);
}
