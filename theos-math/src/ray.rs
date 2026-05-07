use crate::Vec3;
use bytemuck::{Pod, Zeroable};

/// A ray defined by an origin point and a unit-length direction vector.
///
/// Rays are used for intersection tests (picking, shadows, BVH traversal) and
/// are parameterised by a scalar `t` where `t = 0` is the origin and increasing
/// `t` moves along the direction. The direction must be unit-length — [`Ray::new`]
/// panics if this is violated.
#[repr(C)]
#[derive(Clone, Copy, Debug, PartialEq, Pod, Zeroable)]
pub struct Ray {
    /// The point in space where the ray starts.
    pub origin: Vec3,
    /// The unit-length direction the ray travels in.
    pub direction: Vec3,
}

impl Ray {
    /// Creates a ray from an origin point and a direction vector.
    ///
    /// # Panics
    ///
    /// Panics if `direction` is not unit-length within a tolerance of `1e-5`.
    /// Normalize the direction with [`Vec3::normalize`] before passing it in.
    pub fn new(origin: Vec3, direction: Vec3) -> Self {
        if (direction.length() - 1.0).abs() > 1e-5 {
            panic!(
                "Ray direction must be of length 1.0, got: {:?}",
                direction.length()
            );
        }
        Self { origin, direction }
    }

    /// Returns the point along the ray at parameter `t`.
    ///
    /// Computed as `origin + direction * t`. A value of `t = 0` returns the
    /// origin; positive `t` moves forward along the direction.
    pub fn at(self, t: f32) -> Vec3 {
        self.origin + self.direction * t
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Vec3;

    const EPSILON: f32 = 1e-5;

    fn assert_vec3_eq(a: Vec3, b: Vec3) {
        assert!((a.x - b.x).abs() < EPSILON, "x: {} != {}", a.x, b.x);
        assert!((a.y - b.y).abs() < EPSILON, "y: {} != {}", a.y, b.y);
        assert!((a.z - b.z).abs() < EPSILON, "z: {} != {}", a.z, b.z);
    }

    // ==========================================
    // CONSTRUCTION
    // ==========================================

    #[test]
    fn test_new_stores_origin_and_direction() {
        let origin = Vec3::new(1.0, 2.0, 3.0);
        let direction = Vec3::new(0.0, 0.0, 1.0);
        let ray = Ray::new(origin, direction);
        assert_vec3_eq(ray.origin, origin);
        assert_vec3_eq(ray.direction, direction);
    }

    #[test]
    #[should_panic(expected = "Ray direction must be of length 1.0")]
    fn test_new_panics_on_non_unit_direction() {
        Ray::new(Vec3::new(0.0, 0.0, 0.0), Vec3::new(0.0, 0.0, 2.0));
    }

    #[test]
    #[should_panic(expected = "Ray direction must be of length 1.0")]
    fn test_new_panics_on_zero_direction() {
        Ray::new(Vec3::new(0.0, 0.0, 0.0), Vec3::new(0.0, 0.0, 0.0));
    }

    #[test]
    fn test_new_accepts_axis_aligned_directions() {
        // Axis-aligned unit vectors are the most common case — ensure none are
        // rejected by floating point precision issues at the validation boundary.
        Ray::new(Vec3::new(0.0, 0.0, 0.0), Vec3::new(1.0, 0.0, 0.0));
        Ray::new(Vec3::new(0.0, 0.0, 0.0), Vec3::new(0.0, 1.0, 0.0));
        Ray::new(Vec3::new(0.0, 0.0, 0.0), Vec3::new(0.0, 0.0, 1.0));
        Ray::new(Vec3::new(0.0, 0.0, 0.0), Vec3::new(-1.0, 0.0, 0.0));
    }

    // ==========================================
    // AT
    // ==========================================

    #[test]
    fn test_at_zero_returns_origin() {
        let ray = Ray::new(Vec3::new(1.0, 2.0, 3.0), Vec3::new(0.0, 1.0, 0.0));
        assert_vec3_eq(ray.at(0.0), ray.origin);
    }

    #[test]
    fn test_at_one_returns_origin_plus_direction() {
        let ray = Ray::new(Vec3::new(1.0, 0.0, 0.0), Vec3::new(0.0, 1.0, 0.0));
        assert_vec3_eq(ray.at(1.0), Vec3::new(1.0, 1.0, 0.0));
    }

    #[test]
    fn test_at_known_t_value() {
        let ray = Ray::new(Vec3::new(0.0, 0.0, 0.0), Vec3::new(1.0, 0.0, 0.0));
        assert_vec3_eq(ray.at(5.0), Vec3::new(5.0, 0.0, 0.0));
    }

    #[test]
    fn test_at_negative_t_goes_behind_origin() {
        let ray = Ray::new(Vec3::new(0.0, 0.0, 0.0), Vec3::new(1.0, 0.0, 0.0));
        assert_vec3_eq(ray.at(-3.0), Vec3::new(-3.0, 0.0, 0.0));
    }
}
