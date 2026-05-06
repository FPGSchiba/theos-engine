use crate::vec::{Vec3, Vec4};
use bytemuck::{Pod, Zeroable};
use core::ops::Mul;

#[repr(C, align(16))]
#[derive(Clone, Copy, Debug, Default, PartialEq, Pod, Zeroable)]
pub struct Mat4 {
    pub x_axis: Vec4,
    pub y_axis: Vec4,
    pub z_axis: Vec4,
    pub w_axis: Vec4,
}

impl Mat4 {
    pub const fn new(x_axis: Vec4, y_axis: Vec4, z_axis: Vec4, w_axis: Vec4) -> Self {
        Self {
            x_axis,
            y_axis,
            z_axis,
            w_axis,
        }
    }

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

            Mat4::new(res_col0.into(), res_col1.into(), res_col2.into(), res_col3.into())
        }
        #[cfg(not(feature = "simd"))]
        {
            let res_col0 = (self.x_axis * rhs.x_axis.x) + (self.y_axis * rhs.x_axis.y) + (self.z_axis * rhs.x_axis.z) + (self.w_axis * rhs.x_axis.w);
            let res_col1 = (self.x_axis * rhs.y_axis.x) + (self.y_axis * rhs.y_axis.y) + (self.z_axis * rhs.y_axis.z) + (self.w_axis * rhs.y_axis.w);
            let res_col2 = (self.x_axis * rhs.z_axis.x) + (self.y_axis * rhs.z_axis.y) + (self.z_axis * rhs.z_axis.z) + (self.w_axis * rhs.z_axis.w);
            let res_col3 = (self.x_axis * rhs.w_axis.x) + (self.y_axis * rhs.w_axis.y) + (self.z_axis * rhs.w_axis.z) + (self.w_axis * rhs.w_axis.w);
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
            (self.x_axis * rhs.x) + (self.y_axis * rhs.y) + (self.z_axis * rhs.z) + (self.w_axis * rhs.w)
        }
    }
}

#[repr(C, align(16))]
#[derive(Clone, Copy, Debug, Default, PartialEq, Pod, Zeroable)]
pub struct Mat3 {
    pub x_axis: Vec3,
    pub y_axis: Vec3,
    pub z_axis: Vec3,
}

impl Mat3 {
    pub const fn new(x_axis: Vec3, y_axis: Vec3, z_axis: Vec3) -> Self {
        Self {
            x_axis,
            y_axis,
            z_axis,
        }
    }

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
        let res_col0 = (self.x_axis * rhs.x_axis.x) + (self.y_axis * rhs.x_axis.y) + (self.z_axis * rhs.x_axis.z);
        let res_col1 = (self.x_axis * rhs.y_axis.x) + (self.y_axis * rhs.y_axis.y) + (self.z_axis * rhs.y_axis.z);
        let res_col2 = (self.x_axis * rhs.z_axis.x) + (self.y_axis * rhs.z_axis.y) + (self.z_axis * rhs.z_axis.z);
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