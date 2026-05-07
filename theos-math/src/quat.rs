use crate::vec::{Vec3, Vec4};
use bytemuck::{Pod, Zeroable};
use core::ops::{Mul, MulAssign};

/// A unit quaternion representing a 3D rotation.
///
/// Stored as four `f32` components `(x, y, z, w)` where `xyz` is the imaginary
/// (vector) part and `w` is the scalar (real) part. A quaternion encodes a rotation
/// as an axis scaled by `sin(angle/2)` and a `w = cos(angle/2)`.
///
/// Quaternions must be kept unit-length (`length == 1`) to represent pure rotations.
/// Repeated multiplications accumulate floating-point drift — call [`Quat::normalize`]
/// periodically to correct it.
#[repr(C, align(16))]
#[derive(Clone, Copy, Debug, Default, PartialEq, Pod, Zeroable)]
pub struct Quat {
    pub x: f32,
    pub y: f32,
    pub z: f32,
    pub w: f32,
}

impl Quat {
    /// Creates a quaternion from its four components.
    ///
    /// The caller is responsible for ensuring the result is unit-length if it
    /// is intended to represent a rotation.
    pub const fn new(x: f32, y: f32, z: f32, w: f32) -> Self {
        Self { x, y, z, w }
    }

    /// The identity quaternion `(0, 0, 0, 1)`, representing zero rotation.
    ///
    /// Multiplying any quaternion by `IDENTITY` leaves it unchanged.
    /// Rotating any vector by `IDENTITY` leaves it unchanged.
    pub const IDENTITY: Self = Self::new(0.0, 0.0, 0.0, 1.0);

    /// Returns the Euclidean length (magnitude) of the quaternion.
    ///
    /// A unit quaternion has `length == 1`. Use [`Quat::normalize`] to correct drift.
    pub fn length(self) -> f32 {
        let v = Vec4::new(self.x, self.y, self.z, self.w);
        v.dot(v).sqrt()
    }

    /// Returns a copy of this quaternion scaled to unit length.
    ///
    /// Returns a quaternion with `NaN` components if `self` has zero length.
    pub fn normalize(self) -> Self {
        let l = self.length();
        Self::new(self.x / l, self.y / l, self.z / l, self.w / l)
    }

    /// Extracts the imaginary (vector) part `(x, y, z)` as a [`Vec3`].
    pub fn xyz(self) -> Vec3 {
        Vec3::new(self.x, self.y, self.z)
    }
}

/// Composes two rotations.
///
/// `self * rhs` applies `rhs` first, then `self`. Quaternion multiplication is
/// not commutative: `p * q ≠ q * p` in general.
impl Mul for Quat {
    type Output = Self;

    fn mul(self, rhs: Self) -> Self::Output {
        let w = self.w * rhs.w - self.x * rhs.x - self.y * rhs.y - self.z * rhs.z;
        let x = self.w * rhs.x + self.x * rhs.w + self.y * rhs.z - self.z * rhs.y;
        let y = self.w * rhs.y - self.x * rhs.z + self.y * rhs.w + self.z * rhs.x;
        let z = self.w * rhs.z + self.x * rhs.y - self.y * rhs.x + self.z * rhs.w;
        Self::new(x, y, z, w)
    }
}

impl MulAssign for Quat {
    fn mul_assign(&mut self, rhs: Self) {
        *self = *self * rhs;
    }
}

/// Rotates `rhs` by this quaternion using the Rodrigues formula.
///
/// `self` must be a unit quaternion. The result is undefined for non-unit inputs.
impl Mul<Vec3> for Quat {
    type Output = Vec3;

    fn mul(self, rhs: Vec3) -> Vec3 {
        2.0 * self.xyz().dot(rhs) * self.xyz()
            + (self.w * self.w - self.xyz().dot(self.xyz())) * rhs
            + 2.0 * self.w * self.xyz().cross(rhs)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::vec::Vec3;

    const EPSILON: f32 = 1e-5;

    fn assert_quat_eq(a: Quat, b: Quat) {
        assert!((a.x - b.x).abs() < EPSILON, "x: {} != {}", a.x, b.x);
        assert!((a.y - b.y).abs() < EPSILON, "y: {} != {}", a.y, b.y);
        assert!((a.z - b.z).abs() < EPSILON, "z: {} != {}", a.z, b.z);
        assert!((a.w - b.w).abs() < EPSILON, "w: {} != {}", a.w, b.w);
    }

    fn assert_vec3_eq(a: Vec3, b: Vec3) {
        assert!((a.x - b.x).abs() < EPSILON, "x: {} != {}", a.x, b.x);
        assert!((a.y - b.y).abs() < EPSILON, "y: {} != {}", a.y, b.y);
        assert!((a.z - b.z).abs() < EPSILON, "z: {} != {}", a.z, b.z);
    }

    // 90° rotation around the Z axis: w = cos(π/4), z = sin(π/4)
    fn rot90_z() -> Quat {
        Quat::new(0.0, 0.0, 2.0_f32.sqrt() * 0.5, 2.0_f32.sqrt() * 0.5)
    }

    // ==========================================
    // CONSTRUCTION & CONSTANTS
    // ==========================================

    #[test]
    fn test_new_stores_components() {
        let q = Quat::new(1.0, 2.0, 3.0, 4.0);
        assert_eq!(q.x, 1.0);
        assert_eq!(q.y, 2.0);
        assert_eq!(q.z, 3.0);
        assert_eq!(q.w, 4.0);
    }

    #[test]
    fn test_identity_components() {
        assert_eq!(Quat::IDENTITY.x, 0.0);
        assert_eq!(Quat::IDENTITY.y, 0.0);
        assert_eq!(Quat::IDENTITY.z, 0.0);
        assert_eq!(Quat::IDENTITY.w, 1.0);
    }

    #[test]
    fn test_xyz_extracts_imaginary_part() {
        let q = Quat::new(1.0, 2.0, 3.0, 4.0);
        assert_vec3_eq(q.xyz(), Vec3::new(1.0, 2.0, 3.0));
    }

    // ==========================================
    // LENGTH & NORMALIZE
    // ==========================================

    #[test]
    fn test_length_of_identity_is_one() {
        assert!((Quat::IDENTITY.length() - 1.0).abs() < EPSILON);
    }

    #[test]
    fn test_length_known_value() {
        // length((1,2,3,4)) = sqrt(1+4+9+16) = sqrt(30)
        let q = Quat::new(1.0, 2.0, 3.0, 4.0);
        assert!((q.length() - 30.0_f32.sqrt()).abs() < EPSILON);
    }

    #[test]
    fn test_normalize_produces_unit_length() {
        let q = Quat::new(1.0, 2.0, 3.0, 4.0).normalize();
        assert!((q.length() - 1.0).abs() < EPSILON);
    }

    #[test]
    fn test_normalize_identity_is_identity() {
        assert_quat_eq(Quat::IDENTITY.normalize(), Quat::IDENTITY);
    }

    // ==========================================
    // QUATERNION MULTIPLICATION
    // ==========================================

    #[test]
    fn test_mul_identity_left() {
        let q = Quat::new(0.5, 0.5, 0.5, 0.5);
        assert_quat_eq(Quat::IDENTITY * q, q);
    }

    #[test]
    fn test_mul_identity_right() {
        let q = Quat::new(0.5, 0.5, 0.5, 0.5);
        assert_quat_eq(q * Quat::IDENTITY, q);
    }

    #[test]
    fn test_mul_identity_with_itself() {
        assert_quat_eq(Quat::IDENTITY * Quat::IDENTITY, Quat::IDENTITY);
    }

    #[test]
    fn test_mul_is_not_commutative() {
        let p = Quat::new(0.5, 0.5, 0.5, 0.5);
        let q = rot90_z();
        assert_ne!(p * q, q * p);
    }

    #[test]
    fn test_mul_assign() {
        let mut q = rot90_z();
        let original = q;
        q *= Quat::IDENTITY;
        assert_quat_eq(q, original);
    }

    #[test]
    fn test_mul_two_90deg_rotations_gives_180deg() {
        // Two 90° rotations around Z should rotate the X axis to -X.
        let q = rot90_z();
        let after_180 = (q * q) * Vec3::new(1.0, 0.0, 0.0);
        assert_vec3_eq(after_180, Vec3::new(-1.0, 0.0, 0.0));
    }

    // ==========================================
    // VECTOR ROTATION
    // ==========================================

    #[test]
    fn test_rotate_by_identity_is_noop() {
        let v = Vec3::new(1.0, 2.0, 3.0);
        assert_vec3_eq(Quat::IDENTITY * v, v);
    }

    #[test]
    fn test_rotate_x_axis_90deg_around_z_gives_y_axis() {
        let rotated = rot90_z() * Vec3::new(1.0, 0.0, 0.0);
        assert_vec3_eq(rotated, Vec3::new(0.0, 1.0, 0.0));
    }

    #[test]
    fn test_rotate_preserves_vector_length() {
        let v = Vec3::new(1.0, 2.0, 3.0);
        let rotated = rot90_z() * v;
        assert!((v.dot(v) - rotated.dot(rotated)).abs() < EPSILON);
    }
}
