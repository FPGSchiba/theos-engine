use crate::vec::{Vec3, Vec4};
use bytemuck::{Pod, Zeroable};
use core::ops::Mul;

/// A 4×4 column-major floating-point matrix.
///
/// Stored as four [`Vec4`] columns (`x_axis`…`w_axis`). The `repr(C, align(16))`
/// layout guarantees each column sits on a 16-byte boundary, matching the
/// `float4` alignment required by GPU constant buffers and SIMD load instructions.
///
/// Multiplication is column-major: `self * rhs` computes each result column as
/// `self` applied to the corresponding column of `rhs`.
#[repr(C, align(16))]
#[derive(Clone, Copy, Debug, Default, PartialEq, Pod, Zeroable)]
pub struct Mat4 {
    pub x_axis: Vec4,
    pub y_axis: Vec4,
    pub z_axis: Vec4,
    pub w_axis: Vec4,
}

impl Mat4 {
    /// Creates a `Mat4` from four column vectors.
    pub const fn new(x_axis: Vec4, y_axis: Vec4, z_axis: Vec4, w_axis: Vec4) -> Self {
        Self {
            x_axis,
            y_axis,
            z_axis,
            w_axis,
        }
    }

    /// The identity matrix: ones on the diagonal, zeros elsewhere.
    pub const IDENTITY: Self = Self {
        x_axis: Vec4::new(1.0, 0.0, 0.0, 0.0),
        y_axis: Vec4::new(0.0, 1.0, 0.0, 0.0),
        z_axis: Vec4::new(0.0, 0.0, 1.0, 0.0),
        w_axis: Vec4::new(0.0, 0.0, 0.0, 1.0),
    };
}

impl Mul for Mat4 {
    type Output = Self;

    #[inline]
    fn mul(self, rhs: Self) -> Self::Output {
        #[cfg(feature = "simd")]
        {
            // 1. Load Matrix A's (self) columns into SIMD registers ONCE
            let a_col0: wide::f32x4 = self.x_axis.into();
            let a_col1: wide::f32x4 = self.y_axis.into();
            let a_col2: wide::f32x4 = self.z_axis.into();
            let a_col3: wide::f32x4 = self.w_axis.into();

            // 2. Compute Result Column 0 using Matrix B's (rhs) first column (x_axis)
            let b0_x = wide::f32x4::splat(rhs.x_axis.x);
            let b0_y = wide::f32x4::splat(rhs.x_axis.y);
            let b0_z = wide::f32x4::splat(rhs.x_axis.z);
            let b0_w = wide::f32x4::splat(rhs.x_axis.w);

            let res_col0 = (a_col0 * b0_x) + (a_col1 * b0_y) + (a_col2 * b0_z) + (a_col3 * b0_w);

            let b1_x = wide::f32x4::splat(rhs.y_axis.x);
            let b1_y = wide::f32x4::splat(rhs.y_axis.y);
            let b1_z = wide::f32x4::splat(rhs.y_axis.z);
            let b1_w = wide::f32x4::splat(rhs.y_axis.w);

            let res_col1 = (a_col0 * b1_x) + (a_col1 * b1_y) + (a_col2 * b1_z) + (a_col3 * b1_w);

            let b2_x = wide::f32x4::splat(rhs.z_axis.x);
            let b2_y = wide::f32x4::splat(rhs.z_axis.y);
            let b2_z = wide::f32x4::splat(rhs.z_axis.z);
            let b2_w = wide::f32x4::splat(rhs.z_axis.w);

            let res_col2 = (a_col0 * b2_x) + (a_col1 * b2_y) + (a_col2 * b2_z) + (a_col3 * b2_w);

            let b3_x = wide::f32x4::splat(rhs.w_axis.x);
            let b3_y = wide::f32x4::splat(rhs.w_axis.y);
            let b3_z = wide::f32x4::splat(rhs.w_axis.z);
            let b3_w = wide::f32x4::splat(rhs.w_axis.w);

            let res_col3 = (a_col0 * b3_x) + (a_col1 * b3_y) + (a_col2 * b3_z) + (a_col3 * b3_w);

            Mat4::new(
                res_col0.into(),
                res_col1.into(),
                res_col2.into(),
                res_col3.into(),
            )
        }
        #[cfg(not(feature = "simd"))]
        {
            let res_col0 = (self.x_axis * rhs.x_axis.x)
                + (self.y_axis * rhs.x_axis.y)
                + (self.z_axis * rhs.x_axis.z)
                + (self.w_axis * rhs.x_axis.w);
            let res_col1 = (self.x_axis * rhs.y_axis.x)
                + (self.y_axis * rhs.y_axis.y)
                + (self.z_axis * rhs.y_axis.z)
                + (self.w_axis * rhs.y_axis.w);
            let res_col2 = (self.x_axis * rhs.z_axis.x)
                + (self.y_axis * rhs.z_axis.y)
                + (self.z_axis * rhs.z_axis.z)
                + (self.w_axis * rhs.z_axis.w);
            let res_col3 = (self.x_axis * rhs.w_axis.x)
                + (self.y_axis * rhs.w_axis.y)
                + (self.z_axis * rhs.w_axis.z)
                + (self.w_axis * rhs.w_axis.w);
            Mat4::new(res_col0, res_col1, res_col2, res_col3)
        }
    }
}

impl Mul<Vec4> for Mat4 {
    type Output = Vec4;

    #[inline]
    fn mul(self, rhs: Vec4) -> Self::Output {
        #[cfg(feature = "simd")]
        {
            // 1. Load Matrix A's (self) columns into SIMD registers ONCE
            let a_col0: wide::f32x4 = self.x_axis.into();
            let a_col1: wide::f32x4 = self.y_axis.into();
            let a_col2: wide::f32x4 = self.z_axis.into();
            let a_col3: wide::f32x4 = self.w_axis.into();

            // 2. Splat the scalar values from your `rhs` vector
            let b_x = wide::f32x4::splat(rhs.x);
            let b_y = wide::f32x4::splat(rhs.y);
            let b_z = wide::f32x4::splat(rhs.z);
            let b_w = wide::f32x4::splat(rhs.w);

            // 3. Multiply and add them together
            let res = (a_col0 * b_x) + (a_col1 * b_y) + (a_col2 * b_z) + (a_col3 * b_w);

            // 4. Return the result converted back to a Vec4
            res.into()
        }
        #[cfg(not(feature = "simd"))]
        {
            (self.x_axis * rhs.x)
                + (self.y_axis * rhs.y)
                + (self.z_axis * rhs.z)
                + (self.w_axis * rhs.w)
        }
    }
}

/// A 3×3 column-major floating-point matrix.
///
/// Stored as three [`Vec3`] columns. Each `Vec3` is 16-byte aligned (padded),
/// so the full `Mat3` is 48 bytes with no internal holes. Commonly used for
/// rotation and normal transforms where the translation column of a `Mat4`
/// is not needed.
#[repr(C, align(16))]
#[derive(Clone, Copy, Debug, Default, PartialEq, Pod, Zeroable)]
pub struct Mat3 {
    pub x_axis: Vec3,
    pub y_axis: Vec3,
    pub z_axis: Vec3,
}

impl Mat3 {
    /// Creates a `Mat3` from three column vectors.
    pub const fn new(x_axis: Vec3, y_axis: Vec3, z_axis: Vec3) -> Self {
        Self {
            x_axis,
            y_axis,
            z_axis,
        }
    }

    /// The identity matrix: ones on the diagonal, zeros elsewhere.
    pub const IDENTITY: Self = Self {
        x_axis: Vec3::new(1.0, 0.0, 0.0),
        y_axis: Vec3::new(0.0, 1.0, 0.0),
        z_axis: Vec3::new(0.0, 0.0, 1.0),
    };
}

impl Mul for Mat3 {
    type Output = Self;

    #[inline]
    fn mul(self, rhs: Self) -> Self::Output {
        // No SIMD cfg flag needed here!
        // We will just use the Vec3 operator traits you already wrote.
        let res_col0 = (self.x_axis * rhs.x_axis.x)
            + (self.y_axis * rhs.x_axis.y)
            + (self.z_axis * rhs.x_axis.z);
        let res_col1 = (self.x_axis * rhs.y_axis.x)
            + (self.y_axis * rhs.y_axis.y)
            + (self.z_axis * rhs.y_axis.z);
        let res_col2 = (self.x_axis * rhs.z_axis.x)
            + (self.y_axis * rhs.z_axis.y)
            + (self.z_axis * rhs.z_axis.z);
        Self::new(res_col0, res_col1, res_col2)
    }
}

impl Mul<Vec3> for Mat3 {
    type Output = Vec3;

    #[inline]
    fn mul(self, rhs: Vec3) -> Self::Output {
        (self.x_axis * rhs.x) + (self.y_axis * rhs.y) + (self.z_axis * rhs.z)
    }
}

// ==========================================
// GLAM INTEROPERABILITY
// ==========================================

impl From<glam::Mat4> for Mat4 {
    /// Converts a `glam::Mat4` to a `Mat4`, copying all four column vectors.
    ///
    /// Both types use column-major layout with `Vec4` columns, so this is a
    /// direct field-by-field copy.
    #[inline]
    fn from(m: glam::Mat4) -> Self {
        Self::new(
            m.x_axis.into(),
            m.y_axis.into(),
            m.z_axis.into(),
            m.w_axis.into(),
        )
    }
}

impl From<Mat4> for glam::Mat4 {
    /// Converts a `Mat4` to a `glam::Mat4`, copying all four column vectors.
    #[inline]
    fn from(m: Mat4) -> Self {
        glam::Mat4::from_cols(
            m.x_axis.into(),
            m.y_axis.into(),
            m.z_axis.into(),
            m.w_axis.into(),
        )
    }
}

impl From<glam::Mat3> for Mat3 {
    /// Converts a `glam::Mat3` (12-byte `Vec3` columns) to a `Mat3`.
    ///
    /// Each `glam::Vec3` column is widened to a padded 16-byte `Vec3` during
    /// conversion — see [`Vec3::from<glam::Vec3>`].
    #[inline]
    fn from(m: glam::Mat3) -> Self {
        Self::new(m.x_axis.into(), m.y_axis.into(), m.z_axis.into())
    }
}

impl From<Mat3> for glam::Mat3 {
    /// Converts a `Mat3` to a `glam::Mat3`, dropping the padding byte from each column.
    #[inline]
    fn from(m: Mat3) -> Self {
        glam::Mat3::from_cols(m.x_axis.into(), m.y_axis.into(), m.z_axis.into())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::vec::{Vec3, Vec4};

    // Column-major epsilon comparisons — Mat4 and Mat3 store axes as columns.
    macro_rules! assert_vec4_eq {
        ($a:expr, $b:expr) => {
            assert!(($a.x - $b.x).abs() < 1e-5, "x: {} != {}", $a.x, $b.x);
            assert!(($a.y - $b.y).abs() < 1e-5, "y: {} != {}", $a.y, $b.y);
            assert!(($a.z - $b.z).abs() < 1e-5, "z: {} != {}", $a.z, $b.z);
            assert!(($a.w - $b.w).abs() < 1e-5, "w: {} != {}", $a.w, $b.w);
        };
    }

    macro_rules! assert_vec3_eq {
        ($a:expr, $b:expr) => {
            assert!(($a.x - $b.x).abs() < 1e-5, "x: {} != {}", $a.x, $b.x);
            assert!(($a.y - $b.y).abs() < 1e-5, "y: {} != {}", $a.y, $b.y);
            assert!(($a.z - $b.z).abs() < 1e-5, "z: {} != {}", $a.z, $b.z);
        };
    }

    macro_rules! assert_mat4_eq {
        ($a:expr, $b:expr) => {
            assert_vec4_eq!($a.x_axis, $b.x_axis);
            assert_vec4_eq!($a.y_axis, $b.y_axis);
            assert_vec4_eq!($a.z_axis, $b.z_axis);
            assert_vec4_eq!($a.w_axis, $b.w_axis);
        };
    }

    macro_rules! assert_mat3_eq {
        ($a:expr, $b:expr) => {
            assert_vec3_eq!($a.x_axis, $b.x_axis);
            assert_vec3_eq!($a.y_axis, $b.y_axis);
            assert_vec3_eq!($a.z_axis, $b.z_axis);
        };
    }

    // ==========================================
    // GLAM INTEROP TESTS
    // ==========================================

    #[test]
    fn test_glam_mat4_roundtrip() {
        let original = Mat4::new(
            Vec4::new(1.0, 2.0, 3.0, 4.0),
            Vec4::new(5.0, 6.0, 7.0, 8.0),
            Vec4::new(9.0, 10.0, 11.0, 12.0),
            Vec4::new(13.0, 14.0, 15.0, 16.0),
        );
        let g: glam::Mat4 = original.into();
        let back: Mat4 = g.into();
        assert_mat4_eq!(original, back);
    }

    #[test]
    fn test_glam_mat4_identity_roundtrip() {
        let g: glam::Mat4 = Mat4::IDENTITY.into();
        let back: Mat4 = g.into();
        assert_mat4_eq!(back, Mat4::IDENTITY);
        // Sanity: glam's identity should match ours column-for-column.
        assert_eq!(g, glam::Mat4::IDENTITY);
    }

    #[test]
    fn test_glam_mat3_roundtrip() {
        let original = Mat3::new(
            Vec3::new(1.0, 2.0, 3.0),
            Vec3::new(4.0, 5.0, 6.0),
            Vec3::new(7.0, 8.0, 9.0),
        );
        let g: glam::Mat3 = original.into();
        let back: Mat3 = g.into();
        assert_mat3_eq!(original, back);
    }

    #[test]
    fn test_glam_mat3_identity_roundtrip() {
        let g: glam::Mat3 = Mat3::IDENTITY.into();
        let back: Mat3 = g.into();
        assert_mat3_eq!(back, Mat3::IDENTITY);
        assert_eq!(g, glam::Mat3::IDENTITY);
    }

    // ==========================================
    // MAT4 TESTS
    // ==========================================

    #[test]
    fn test_mat4_identity_columns_are_unit_axes() {
        let id = Mat4::IDENTITY;
        assert_vec4_eq!(id.x_axis, Vec4::new(1.0, 0.0, 0.0, 0.0));
        assert_vec4_eq!(id.y_axis, Vec4::new(0.0, 1.0, 0.0, 0.0));
        assert_vec4_eq!(id.z_axis, Vec4::new(0.0, 0.0, 1.0, 0.0));
        assert_vec4_eq!(id.w_axis, Vec4::new(0.0, 0.0, 0.0, 1.0));
    }

    #[test]
    fn test_mat4_mul_by_identity_is_noop() {
        let m = Mat4::new(
            Vec4::new(1.0, 2.0, 3.0, 4.0),
            Vec4::new(5.0, 6.0, 7.0, 8.0),
            Vec4::new(9.0, 10.0, 11.0, 12.0),
            Vec4::new(13.0, 14.0, 15.0, 16.0),
        );
        assert_mat4_eq!(m * Mat4::IDENTITY, m);
        assert_mat4_eq!(Mat4::IDENTITY * m, m);
    }

    #[test]
    fn test_mat4_mul_known_result() {
        // Embed a 2×2 problem in the top-left; identity in the rest.
        // A (cols): col0=(1,2,0,0)  col1=(3,4,0,0)  col2=e3  col3=e4
        // B (cols): col0=(5,6,0,0)  col1=(7,8,0,0)  col2=e3  col3=e4
        //
        // As row-major 2×2: A = [[1,3],[2,4]]  B = [[5,7],[6,8]]
        // A*B = [[1*5+3*6, 1*7+3*8],[2*5+4*6, 2*7+4*8]] = [[23,31],[34,46]]
        // So result col0 = (23,34,0,0)  col1 = (31,46,0,0)
        let a = Mat4::new(
            Vec4::new(1.0, 2.0, 0.0, 0.0),
            Vec4::new(3.0, 4.0, 0.0, 0.0),
            Vec4::new(0.0, 0.0, 1.0, 0.0),
            Vec4::new(0.0, 0.0, 0.0, 1.0),
        );
        let b = Mat4::new(
            Vec4::new(5.0, 6.0, 0.0, 0.0),
            Vec4::new(7.0, 8.0, 0.0, 0.0),
            Vec4::new(0.0, 0.0, 1.0, 0.0),
            Vec4::new(0.0, 0.0, 0.0, 1.0),
        );
        let result = a * b;
        assert_vec4_eq!(result.x_axis, Vec4::new(23.0, 34.0, 0.0, 0.0));
        assert_vec4_eq!(result.y_axis, Vec4::new(31.0, 46.0, 0.0, 0.0));
        assert_vec4_eq!(result.z_axis, Vec4::new(0.0, 0.0, 1.0, 0.0));
        assert_vec4_eq!(result.w_axis, Vec4::new(0.0, 0.0, 0.0, 1.0));
    }

    #[test]
    fn test_mat4_mul_vec4_identity_is_noop() {
        let v = Vec4::new(1.0, 2.0, 3.0, 4.0);
        assert_vec4_eq!(Mat4::IDENTITY * v, v);
    }

    #[test]
    fn test_mat4_mul_vec4_scaling() {
        // Diagonal scaling matrix: scale x by 2, y by 3, z by 4, w unchanged.
        let scale = Mat4::new(
            Vec4::new(2.0, 0.0, 0.0, 0.0),
            Vec4::new(0.0, 3.0, 0.0, 0.0),
            Vec4::new(0.0, 0.0, 4.0, 0.0),
            Vec4::new(0.0, 0.0, 0.0, 1.0),
        );
        let result = scale * Vec4::new(1.0, 1.0, 1.0, 1.0);
        assert_vec4_eq!(result, Vec4::new(2.0, 3.0, 4.0, 1.0));
    }

    #[test]
    fn test_mat4_mul_is_not_commutative() {
        // For any two non-commuting matrices A and B, A*B ≠ B*A.
        let a = Mat4::new(
            Vec4::new(1.0, 2.0, 0.0, 0.0),
            Vec4::new(3.0, 4.0, 0.0, 0.0),
            Vec4::new(0.0, 0.0, 1.0, 0.0),
            Vec4::new(0.0, 0.0, 0.0, 1.0),
        );
        let b = Mat4::new(
            Vec4::new(5.0, 6.0, 0.0, 0.0),
            Vec4::new(7.0, 8.0, 0.0, 0.0),
            Vec4::new(0.0, 0.0, 1.0, 0.0),
            Vec4::new(0.0, 0.0, 0.0, 1.0),
        );
        assert_ne!(a * b, b * a);
    }

    // ==========================================
    // MAT3 TESTS
    // ==========================================

    #[test]
    fn test_mat3_identity_columns_are_unit_axes() {
        let id = Mat3::IDENTITY;
        assert_vec3_eq!(id.x_axis, Vec3::new(1.0, 0.0, 0.0));
        assert_vec3_eq!(id.y_axis, Vec3::new(0.0, 1.0, 0.0));
        assert_vec3_eq!(id.z_axis, Vec3::new(0.0, 0.0, 1.0));
    }

    #[test]
    fn test_mat3_mul_by_identity_is_noop() {
        let m = Mat3::new(
            Vec3::new(1.0, 2.0, 3.0),
            Vec3::new(4.0, 5.0, 6.0),
            Vec3::new(7.0, 8.0, 9.0),
        );
        assert_mat3_eq!(m * Mat3::IDENTITY, m);
        assert_mat3_eq!(Mat3::IDENTITY * m, m);
    }

    #[test]
    fn test_mat3_mul_known_result() {
        // Same 2×2 embed strategy as Mat4 test — keeps manual verification tractable.
        // A = [[1,3,0],[2,4,0],[0,0,1]]  B = [[5,7,0],[6,8,0],[0,0,1]]
        // A*B col0 = (23,34,0)   col1 = (31,46,0)   col2 = (0,0,1)
        let a = Mat3::new(
            Vec3::new(1.0, 2.0, 0.0),
            Vec3::new(3.0, 4.0, 0.0),
            Vec3::new(0.0, 0.0, 1.0),
        );
        let b = Mat3::new(
            Vec3::new(5.0, 6.0, 0.0),
            Vec3::new(7.0, 8.0, 0.0),
            Vec3::new(0.0, 0.0, 1.0),
        );
        let result = a * b;
        assert_vec3_eq!(result.x_axis, Vec3::new(23.0, 34.0, 0.0));
        assert_vec3_eq!(result.y_axis, Vec3::new(31.0, 46.0, 0.0));
        assert_vec3_eq!(result.z_axis, Vec3::new(0.0, 0.0, 1.0));
    }

    #[test]
    fn test_mat3_mul_vec3_identity_is_noop() {
        let v = Vec3::new(1.0, 2.0, 3.0);
        assert_vec3_eq!(Mat3::IDENTITY * v, v);
    }

    #[test]
    fn test_mat3_mul_vec3_scaling() {
        let scale = Mat3::new(
            Vec3::new(2.0, 0.0, 0.0),
            Vec3::new(0.0, 3.0, 0.0),
            Vec3::new(0.0, 0.0, 4.0),
        );
        let result = scale * Vec3::new(1.0, 1.0, 1.0);
        assert_vec3_eq!(result, Vec3::new(2.0, 3.0, 4.0));
    }

    #[test]
    fn test_mat3_mul_is_not_commutative() {
        let a = Mat3::new(
            Vec3::new(1.0, 2.0, 0.0),
            Vec3::new(3.0, 4.0, 0.0),
            Vec3::new(0.0, 0.0, 1.0),
        );
        let b = Mat3::new(
            Vec3::new(5.0, 6.0, 0.0),
            Vec3::new(7.0, 8.0, 0.0),
            Vec3::new(0.0, 0.0, 1.0),
        );
        assert_ne!(a * b, b * a);
    }
}
