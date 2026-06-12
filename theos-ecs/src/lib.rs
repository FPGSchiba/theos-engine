#![forbid(unsafe_code)]

/// In-memory [`WorldAccess`](theos_shared::WorldAccess) stub for tests and G2
/// seam development. Gated behind the non-default `testkit` feature.
#[cfg(feature = "testkit")]
pub mod testkit;

pub fn add(left: u64, right: u64) -> u64 {
    left + right
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn it_works() {
        let result = add(2, 2);
        assert_eq!(result, 4);
    }
}
