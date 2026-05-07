use crate::{Ray, Vec3};
use bytemuck::Zeroable;

/// An infinite plane defined by a unit-length normal and a signed distance from the origin.
///
/// The plane equation is `dot(normal, point) = distance`. All points satisfying
/// this equation lie on the plane. Points where the result is greater than
/// `distance` are in front of the plane (the side the normal points toward);
/// points where it is less are behind it.
///
/// The normal is always normalized at construction time.
#[repr(C)]
#[derive(Clone, Copy, Debug, PartialEq, Zeroable)]
pub struct Plane {
    /// The unit-length vector perpendicular to the plane surface.
    pub normal: Vec3,
    /// The signed distance from the world origin to the plane along the normal.
    pub distance: f32,
}

impl Plane {
    /// Creates a plane from a normal vector and a distance from the origin.
    ///
    /// The `normal` is normalized automatically, so passing a non-unit vector
    /// is safe. The `distance` is kept as-is.
    pub fn new(normal: Vec3, distance: f32) -> Self {
        let normal = normal.normalize();
        Self { normal, distance }
    }

    /// Returns the signed distance from `point` to this plane.
    ///
    /// - Positive: the point is in front of the plane (same side as the normal).
    /// - Zero: the point lies on the plane.
    /// - Negative: the point is behind the plane.
    pub fn signed_distance(&self, point: Vec3) -> f32 {
        self.normal.dot(point) - self.distance
    }

    /// Returns the point where `ray` intersects this plane, or `None` if the
    /// ray is parallel to the plane.
    ///
    /// A ray is considered parallel when `dot(normal, ray.direction)` is within
    /// `1e-5` of zero. Rays hitting the plane from behind are handled correctly
    /// and return `Some`.
    pub fn intersects_ray(&self, ray: Ray) -> Option<Vec3> {
        let denom = self.normal.dot(ray.direction);
        if denom.abs() < 1e-5 {
            None
        } else {
            let t = (self.distance - self.normal.dot(ray.origin)) / denom;
            Some(ray.at(t))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Ray, Vec3};

    const EPSILON: f32 = 1e-5;

    fn assert_vec3_eq(a: Vec3, b: Vec3) {
        assert!((a.x - b.x).abs() < EPSILON, "x: {} != {}", a.x, b.x);
        assert!((a.y - b.y).abs() < EPSILON, "y: {} != {}", a.y, b.y);
        assert!((a.z - b.z).abs() < EPSILON, "z: {} != {}", a.z, b.z);
    }

    // The XZ plane (y = 0): normal pointing up, distance 0.
    fn xz_plane() -> Plane {
        Plane::new(Vec3::new(0.0, 1.0, 0.0), 0.0)
    }

    // ==========================================
    // CONSTRUCTION
    // ==========================================

    #[test]
    fn test_new_normalizes_normal() {
        let p = Plane::new(Vec3::new(0.0, 2.0, 0.0), 1.0);
        assert!((p.normal.length() - 1.0).abs() < EPSILON);
        assert_vec3_eq(p.normal, Vec3::new(0.0, 1.0, 0.0));
    }

    #[test]
    fn test_new_stores_distance() {
        let p = Plane::new(Vec3::new(0.0, 1.0, 0.0), 5.0);
        assert_eq!(p.distance, 5.0);
    }

    // ==========================================
    // SIGNED DISTANCE
    // ==========================================

    #[test]
    fn test_signed_distance_point_on_plane_is_zero() {
        let p = xz_plane();
        assert!(p.signed_distance(Vec3::new(3.0, 0.0, -2.0)).abs() < EPSILON);
    }

    #[test]
    fn test_signed_distance_point_in_front_is_positive() {
        let p = xz_plane();
        let d = p.signed_distance(Vec3::new(0.0, 3.0, 0.0));
        assert!(d > 0.0);
        assert!((d - 3.0).abs() < EPSILON);
    }

    #[test]
    fn test_signed_distance_point_behind_is_negative() {
        let p = xz_plane();
        let d = p.signed_distance(Vec3::new(0.0, -2.0, 0.0));
        assert!(d < 0.0);
        assert!((d + 2.0).abs() < EPSILON);
    }

    #[test]
    fn test_signed_distance_offset_plane() {
        // Plane at y = 4.
        let p = Plane::new(Vec3::new(0.0, 1.0, 0.0), 4.0);
        assert!(p.signed_distance(Vec3::new(0.0, 4.0, 0.0)).abs() < EPSILON);
        assert!((p.signed_distance(Vec3::new(0.0, 6.0, 0.0)) - 2.0).abs() < EPSILON);
    }

    // ==========================================
    // RAY INTERSECTION
    // ==========================================

    #[test]
    fn test_intersects_ray_parallel_returns_none() {
        // Ray travelling along X — parallel to the XZ plane.
        let ray = Ray::new(Vec3::new(0.0, 1.0, 0.0), Vec3::new(1.0, 0.0, 0.0));
        assert!(xz_plane().intersects_ray(ray).is_none());
    }

    #[test]
    fn test_intersects_ray_from_above_hits_origin() {
        // Ray from (0,2,0) pointing straight down — should hit (0,0,0).
        let ray = Ray::new(Vec3::new(0.0, 2.0, 0.0), Vec3::new(0.0, -1.0, 0.0));
        let hit = xz_plane()
            .intersects_ray(ray)
            .expect("expected intersection");
        assert_vec3_eq(hit, Vec3::new(0.0, 0.0, 0.0));
    }

    #[test]
    fn test_intersects_ray_from_below_hits_plane() {
        // Ray from below pointing up — hits from behind, still valid.
        let ray = Ray::new(Vec3::new(0.0, -2.0, 0.0), Vec3::new(0.0, 1.0, 0.0));
        let hit = xz_plane()
            .intersects_ray(ray)
            .expect("expected intersection");
        assert_vec3_eq(hit, Vec3::new(0.0, 0.0, 0.0));
    }

    #[test]
    fn test_intersects_ray_offset_origin() {
        // Ray from (3,2,5) pointing straight down — should hit (3,0,5).
        let ray = Ray::new(Vec3::new(3.0, 2.0, 5.0), Vec3::new(0.0, -1.0, 0.0));
        let hit = xz_plane()
            .intersects_ray(ray)
            .expect("expected intersection");
        assert_vec3_eq(hit, Vec3::new(3.0, 0.0, 5.0));
    }
}
