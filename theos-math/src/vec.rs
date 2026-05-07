use bytemuck::{Pod, Zeroable};
use core::ops::{Add, AddAssign, Div, DivAssign, Mul, MulAssign, Sub, SubAssign};

// ==========================================
// 1. DATA STRUCTURES (Already completed)
// ==========================================

/// A 2-component floating-point vector with 16-byte alignment.
///
/// Padded to 16 bytes (`repr(C, align(16))`) so it can be loaded into a
/// SIMD register without unaligned penalty and uploaded directly to GPU
/// buffers via `bytemuck`.
#[repr(C, align(16))]
#[derive(Clone, Copy, Debug, Default, PartialEq, Pod, Zeroable)]
pub struct Vec2 {
    pub x: f32,
    pub y: f32,
    _padding: [f32; 2],
}

/// A 3-component floating-point vector with 16-byte alignment.
///
/// Padded to 16 bytes so it shares the same SIMD lane width as [`Vec4`],
/// enabling the same `f32x4` intrinsics for dot and arithmetic operations
/// without a separate code path.
#[repr(C, align(16))]
#[derive(Clone, Copy, Debug, Default, PartialEq, Pod, Zeroable)]
pub struct Vec3 {
    pub x: f32,
    pub y: f32,
    pub z: f32,
    _padding: f32,
}

/// A 4-component floating-point vector with 16-byte alignment.
///
/// Naturally fills one `f32x4` SIMD register with no padding. Used as the
/// column type for [`Mat4`](crate::mat::Mat4) and for homogeneous coordinates
/// where `w = 1` denotes a point and `w = 0` denotes a direction.
#[repr(C, align(16))]
#[derive(Clone, Copy, Debug, Default, PartialEq, Pod, Zeroable)]
pub struct Vec4 {
    pub x: f32,
    pub y: f32,
    pub z: f32,
    pub w: f32,
}

// ==========================================
// 2. CONSTRUCTORS & CORE CAPABILITIES
// ==========================================

impl Vec2 {
    /// Creates a new `Vec2` from x and y components.
    #[inline]
    pub const fn new(x: f32, y: f32) -> Self {
        Self {
            x,
            y,
            _padding: [0.0, 0.0],
        }
    }

    /// Returns the dot (scalar) product of `self` and `rhs`.
    ///
    /// Uses a SIMD `f32x4` multiply-reduce when the `simd` feature is enabled.
    #[inline]
    pub fn dot(self, rhs: Vec2) -> f32 {
        #[cfg(feature = "simd")]
        {
            (wide::f32x4::from([rhs.x, rhs.y, 0.0, 0.0])
                * wide::f32x4::from([self.x, self.y, 0.0, 0.0]))
            .reduce_add()
        }
        #[cfg(not(feature = "simd"))]
        {
            (self.x * rhs.x) + (self.y * rhs.y)
        }
    }

    /// Returns the Euclidean length (magnitude) of the vector.
    #[inline]
    pub fn length(self) -> f32 {
        self.x.hypot(self.y)
    }

    /// Returns a copy of this vector scaled to unit length.
    ///
    /// Returns a vector with `NaN` components if `self` has zero length.
    #[inline]
    pub fn normalize(self) -> Self {
        self / self.length()
    }
}

impl Vec3 {
    /// Creates a new `Vec3` from x, y, and z components.
    #[inline]
    pub const fn new(x: f32, y: f32, z: f32) -> Self {
        Self {
            x,
            y,
            z,
            _padding: 0.0,
        }
    }

    /// Returns the dot (scalar) product of `self` and `rhs`.
    ///
    /// Uses a SIMD `f32x4` multiply-reduce when the `simd` feature is enabled.
    #[inline]
    pub fn dot(self, rhs: Self) -> f32 {
        #[cfg(feature = "simd")]
        {
            (wide::f32x4::from([rhs.x, rhs.y, rhs.z, 0.0])
                * wide::f32x4::from([self.x, self.y, self.z, 0.0]))
            .reduce_add()
        }
        #[cfg(not(feature = "simd"))]
        {
            (self.x * rhs.x) + (self.y * rhs.y) + (self.z * rhs.z)
        }
    }

    /// Returns the cross product of `self` and `rhs`.
    ///
    /// The result is perpendicular to both input vectors. Anti-commutative:
    /// `a.cross(b) == -b.cross(a)`. Returns the zero vector when the inputs
    /// are parallel or either is the zero vector.
    #[inline]
    pub fn cross(self, rhs: Self) -> Self {
        Vec3::new(
            self.y * rhs.z - self.z * rhs.y,
            self.z * rhs.x - self.x * rhs.z,
            self.x * rhs.y - self.y * rhs.x,
        )
    }

    /// Returns the Euclidean length (magnitude) of the vector.
    ///
    /// Uses `dot(self, self).sqrt()` — equivalent to `sqrt(x² + y² + z²)`.
    #[inline]
    pub fn length(self) -> f32 {
        self.dot(self).sqrt()
    }

    /// Returns a copy of this vector scaled to unit length.
    ///
    /// Returns a vector with `NaN` components if `self` has zero length.
    #[inline]
    pub fn normalize(self) -> Self {
        self / self.length()
    }
}

impl Vec4 {
    /// Creates a new `Vec4` from x, y, z, and w components.
    #[inline]
    pub const fn new(x: f32, y: f32, z: f32, w: f32) -> Self {
        Self { x, y, z, w }
    }

    /// Returns the dot (scalar) product of `self` and `rhs`, including the `w` component.
    ///
    /// Uses a SIMD `f32x4` multiply-reduce when the `simd` feature is enabled.
    #[inline]
    pub fn dot(self, rhs: Self) -> f32 {
        #[cfg(feature = "simd")]
        {
            (wide::f32x4::from([rhs.x, rhs.y, rhs.z, rhs.w])
                * wide::f32x4::from([self.x, self.y, self.z, self.w]))
            .reduce_add()
        }
        #[cfg(not(feature = "simd"))]
        {
            (self.x * rhs.x) + (self.y * rhs.y) + (self.z * rhs.z) + (self.w * rhs.w)
        }
    }

    /// Returns the Euclidean length (magnitude) of the vector.
    ///
    /// Uses `dot(self, self).sqrt()` — equivalent to `sqrt(x² + y² + z² + w²)`.
    #[inline]
    pub fn length(self) -> f32 {
        self.dot(self).sqrt()
    }

    /// Returns a copy of this vector scaled to unit length.
    ///
    /// Returns a vector with `NaN` components if `self` has zero length.
    #[inline]
    pub fn normalize(self) -> Self {
        self / self.length()
    }
}

// ==========================================
// 3. TRAIT-BASED SWIZZLING
// ==========================================

/// Swizzle operations that extract sub-vectors from a [`Vec4`].
pub trait Vec4Swizzle {
    /// Extracts the x, y, and z components as a [`Vec3`].
    fn xyz(self) -> Vec3;
    /// Extracts the x and y components as a [`Vec2`].
    fn xy(self) -> Vec2;
    /// Extracts the z and w components as a [`Vec2`].
    fn zw(self) -> Vec2;
}

impl Vec4Swizzle for Vec4 {
    #[inline]
    fn xyz(self) -> Vec3 {
        Vec3::new(self.x, self.y, self.z)
    }

    #[inline]
    fn xy(self) -> Vec2 {
        Vec2::new(self.x, self.y)
    }

    #[inline]
    fn zw(self) -> Vec2 {
        Vec2::new(self.z, self.w)
    }
}

/// Swizzle operations that extract sub-vectors from a [`Vec3`].
pub trait Vec3Swizzle {
    /// Extracts the x and y components as a [`Vec2`].
    fn xy(self) -> Vec2;
    /// Extracts the x and z components as a [`Vec2`].
    fn xz(self) -> Vec2;
    /// Extracts the y and z components as a [`Vec2`].
    fn yz(self) -> Vec2;
}

impl Vec3Swizzle for Vec3 {
    #[inline]
    fn xy(self) -> Vec2 {
        Vec2::new(self.x, self.y)
    }

    #[inline]
    fn xz(self) -> Vec2 {
        Vec2::new(self.x, self.z)
    }

    #[inline]
    fn yz(self) -> Vec2 {
        Vec2::new(self.y, self.z)
    }
}

// ==========================================
// 4. OPERATOR OVERLOADING (Standard Ops)
// ==========================================

// Example structure for Add. You will need to duplicate this pattern
// for Sub, Mul, Div, and for all three Vector types.
impl Add for Vec2 {
    type Output = Vec2;

    #[inline]
    fn add(self, rhs: Self) -> Vec2 {
        #[cfg(feature = "simd")]
        {
            (wide::f32x4::from([self.x, self.y, 0.0, 0.0])
                + wide::f32x4::from([rhs.x, rhs.y, 0.0, 0.0]))
            .into()
        }
        #[cfg(not(feature = "simd"))]
        {
            Self::new(self.x + rhs.x, self.y + rhs.y)
        }
    }
}

impl Add for Vec3 {
    type Output = Self;

    #[inline]
    fn add(self, rhs: Self) -> Self::Output {
        #[cfg(feature = "simd")]
        {
            (wide::f32x4::from([self.x, self.y, self.z, 0.0])
                + wide::f32x4::from([rhs.x, rhs.y, rhs.z, 0.0]))
            .into()
        }
        #[cfg(not(feature = "simd"))]
        {
            Self::new(self.x + rhs.x, self.y + rhs.y, self.z + rhs.z)
        }
    }
}

impl Add for Vec4 {
    type Output = Self;

    #[inline]
    fn add(self, rhs: Self) -> Self::Output {
        #[cfg(feature = "simd")]
        {
            (wide::f32x4::from([self.x, self.y, self.z, self.w])
                + wide::f32x4::from([rhs.x, rhs.y, rhs.z, rhs.w]))
            .into()
        }
        #[cfg(not(feature = "simd"))]
        {
            Self::new(
                self.x + rhs.x,
                self.y + rhs.y,
                self.z + rhs.z,
                self.w + rhs.w,
            )
        }
    }
}

impl AddAssign for Vec2 {
    fn add_assign(&mut self, rhs: Self) {
        *self = *self + rhs;
    }
}

impl AddAssign for Vec3 {
    fn add_assign(&mut self, rhs: Self) {
        *self = *self + rhs
    }
}

impl AddAssign for Vec4 {
    fn add_assign(&mut self, rhs: Self) {
        *self = *self + rhs;
    }
}

impl Sub for Vec2 {
    type Output = Vec2;

    #[inline]
    fn sub(self, rhs: Self) -> Self::Output {
        #[cfg(feature = "simd")]
        {
            (wide::f32x4::from([self.x, self.y, 0.0, 0.0])
                - wide::f32x4::from([rhs.x, rhs.y, 0.0, 0.0]))
            .into()
        }
        #[cfg(not(feature = "simd"))]
        {
            Self::new(self.x - rhs.x, self.y - rhs.y)
        }
    }
}

impl Sub for Vec3 {
    type Output = Self;

    #[inline]
    fn sub(self, rhs: Self) -> Self::Output {
        #[cfg(feature = "simd")]
        {
            (wide::f32x4::from([self.x, self.y, self.z, 0.0])
                - wide::f32x4::from([rhs.x, rhs.y, rhs.z, 0.0]))
            .into()
        }
        #[cfg(not(feature = "simd"))]
        {
            Self::new(self.x - rhs.x, self.y - rhs.y, self.z - rhs.z)
        }
    }
}

impl Sub for Vec4 {
    type Output = Self;

    #[inline]
    fn sub(self, rhs: Self) -> Self::Output {
        #[cfg(feature = "simd")]
        {
            (wide::f32x4::from([self.x, self.y, self.z, self.w])
                - wide::f32x4::from([rhs.x, rhs.y, rhs.z, rhs.w]))
            .into()
        }
        #[cfg(not(feature = "simd"))]
        {
            Self::new(
                self.x - rhs.x,
                self.y - rhs.y,
                self.z - rhs.z,
                self.w - rhs.w,
            )
        }
    }
}

impl SubAssign for Vec2 {
    fn sub_assign(&mut self, rhs: Self) {
        *self = *self - rhs;
    }
}

impl SubAssign for Vec3 {
    fn sub_assign(&mut self, rhs: Self) {
        *self = *self - rhs
    }
}

impl SubAssign for Vec4 {
    fn sub_assign(&mut self, rhs: Self) {
        *self = *self - rhs
    }
}

impl Mul for Vec2 {
    type Output = Vec2;

    fn mul(self, rhs: Vec2) -> Self::Output {
        #[cfg(feature = "simd")]
        {
            (wide::f32x4::from([self.x, self.y, 1.0, 1.0])
                * wide::f32x4::from([rhs.x, rhs.y, 1.0, 1.0]))
            .into()
        }
        #[cfg(not(feature = "simd"))]
        {
            Self::new(self.x * rhs.x, self.y * rhs.y)
        }
    }
}

impl Mul for Vec3 {
    type Output = Self;

    fn mul(self, rhs: Self) -> Self::Output {
        #[cfg(feature = "simd")]
        {
            let rhs_wide = wide::f32x4::from([rhs.x, rhs.y, rhs.z, 1.0]);
            let self_wide = wide::f32x4::from([self.x, self.y, self.z, 1.0]);
            (self_wide * rhs_wide).into()
        }
        #[cfg(not(feature = "simd"))]
        {
            Self::new(self.x * rhs.x, self.y * rhs.y, self.z * rhs.z)
        }
    }
}

impl Mul for Vec4 {
    type Output = Self;

    fn mul(self, rhs: Self) -> Self::Output {
        #[cfg(feature = "simd")]
        {
            (wide::f32x4::from([self.x, self.y, self.z, self.w])
                * wide::f32x4::from([rhs.x, rhs.y, rhs.z, rhs.w]))
            .into()
        }
        #[cfg(not(feature = "simd"))]
        {
            Self::new(
                self.x * rhs.x,
                self.y * rhs.y,
                self.z * rhs.z,
                self.w * rhs.w,
            )
        }
    }
}

impl MulAssign for Vec2 {
    fn mul_assign(&mut self, rhs: Self) {
        *self = *self * rhs
    }
}

impl MulAssign for Vec3 {
    fn mul_assign(&mut self, rhs: Self) {
        *self = *self * rhs;
    }
}

impl MulAssign for Vec4 {
    fn mul_assign(&mut self, rhs: Self) {
        *self = *self * rhs
    }
}

impl Div for Vec2 {
    type Output = Vec2;

    fn div(self, rhs: Vec2) -> Self::Output {
        #[cfg(feature = "simd")]
        {
            (wide::f32x4::from([self.x, self.y, 1.0, 1.0])
                / wide::f32x4::from([rhs.x, rhs.y, 1.0, 1.0]))
            .into()
        }
        #[cfg(not(feature = "simd"))]
        {
            Self::new(self.x / rhs.x, self.y / rhs.y)
        }
    }
}

impl Div for Vec3 {
    type Output = Self;

    fn div(self, rhs: Self) -> Self::Output {
        #[cfg(feature = "simd")]
        {
            let rhs_wide = wide::f32x4::from([rhs.x, rhs.y, rhs.z, 1.0]);
            let self_wide = wide::f32x4::from([self.x, self.y, self.z, 1.0]);
            (self_wide / rhs_wide).into()
        }
        #[cfg(not(feature = "simd"))]
        {
            Self::new(self.x / rhs.x, self.y / rhs.y, self.z / rhs.z)
        }
    }
}

impl Div for Vec4 {
    type Output = Self;

    fn div(self, rhs: Self) -> Self::Output {
        #[cfg(feature = "simd")]
        {
            (wide::f32x4::from([self.x, self.y, self.z, self.w])
                / wide::f32x4::from([rhs.x, rhs.y, rhs.z, rhs.w]))
            .into()
        }
        #[cfg(not(feature = "simd"))]
        {
            Self::new(
                self.x / rhs.x,
                self.y / rhs.y,
                self.z / rhs.z,
                self.w / rhs.w,
            )
        }
    }
}

impl DivAssign for Vec2 {
    fn div_assign(&mut self, rhs: Self) {
        *self = *self / rhs
    }
}

impl DivAssign for Vec3 {
    fn div_assign(&mut self, rhs: Self) {
        *self = *self / rhs
    }
}

impl DivAssign for Vec4 {
    fn div_assign(&mut self, rhs: Self) {
        *self = *self / rhs
    }
}

// ==========================================
// SCALAR DOT Operations
// ==========================================

impl Mul<f32> for Vec2 {
    type Output = Self;

    #[inline]
    fn mul(self, rhs: f32) -> Self::Output {
        Self::new(self.x * rhs, self.y * rhs)
    }
}

impl Mul<f32> for Vec3 {
    type Output = Self;

    #[inline]
    fn mul(self, rhs: f32) -> Self::Output {
        // Architectural Note: Just like the cross product, doing scalar math
        // on a 3-component struct in an AoS layout is often faster in standard
        // scalar instructions than paying the SIMD load/extract overhead.
        // You can safely just use the standard fallback here!
        Self::new(self.x * rhs, self.y * rhs, self.z * rhs)
    }
}

impl Mul<Vec3> for f32 {
    type Output = Vec3;

    fn mul(self, rhs: Vec3) -> Vec3 {
        rhs * self
    }
}

impl Mul<f32> for Vec4 {
    type Output = Self;

    #[inline]
    fn mul(self, rhs: f32) -> Self::Output {
        #[cfg(feature = "simd")]
        {
            (wide::f32x4::from([self.x, self.y, self.z, self.w]) * wide::f32x4::splat(rhs)).into()
        }
        #[cfg(not(feature = "simd"))]
        {
            Self::new(self.x * rhs, self.y * rhs, self.z * rhs, self.w * rhs)
        }
    }
}

impl MulAssign<f32> for Vec2 {
    fn mul_assign(&mut self, rhs: f32) {
        *self = *self * rhs
    }
}

impl MulAssign<f32> for Vec3 {
    fn mul_assign(&mut self, rhs: f32) {
        *self = *self * rhs
    }
}

impl MulAssign<f32> for Vec4 {
    fn mul_assign(&mut self, rhs: f32) {
        *self = *self * rhs
    }
}

impl Div<f32> for Vec2 {
    type Output = Self;

    fn div(self, rhs: f32) -> Self::Output {
        // Architectural Note: Just like the cross product, doing scalar math
        // on a 3-component struct in an AoS layout is often faster in standard
        // scalar instructions than paying the SIMD load/extract overhead.
        // You can safely just use the standard fallback here!
        Self::new(self.x / rhs, self.y / rhs)
    }
}

impl Div<f32> for Vec3 {
    type Output = Self;

    fn div(self, rhs: f32) -> Self::Output {
        // Architectural Note: Just like the cross product, doing scalar math
        // on a 3-component struct in an AoS layout is often faster in standard
        // scalar instructions than paying the SIMD load/extract overhead.
        // You can safely just use the standard fallback here!
        Self::new(self.x / rhs, self.y / rhs, self.z / rhs)
    }
}

impl Div<f32> for Vec4 {
    type Output = Self;

    fn div(self, rhs: f32) -> Self::Output {
        #[cfg(feature = "simd")]
        {
            (wide::f32x4::from([self.x, self.y, self.z, self.w]) / wide::f32x4::splat(rhs)).into()
        }
        #[cfg(not(feature = "simd"))]
        {
            Self::new(self.x / rhs, self.y / rhs, self.z / rhs, self.w / rhs)
        }
    }
}

impl DivAssign<f32> for Vec2 {
    fn div_assign(&mut self, rhs: f32) {
        *self = *self / rhs
    }
}

impl DivAssign<f32> for Vec3 {
    fn div_assign(&mut self, rhs: f32) {
        *self = *self / rhs
    }
}

impl DivAssign<f32> for Vec4 {
    fn div_assign(&mut self, rhs: f32) {
        *self = *self / rhs
    }
}

// ==========================================
// 5. GLAM INTEROPERABILITY
// ==========================================

impl From<glam::Vec2> for Vec2 {
    /// Converts a `glam::Vec2` to a `Vec2`, copying x and y directly.
    #[inline]
    fn from(v: glam::Vec2) -> Self {
        Self::new(v.x, v.y)
    }
}

impl From<Vec2> for glam::Vec2 {
    /// Converts a `Vec2` to a `glam::Vec2`, dropping the internal padding bytes.
    #[inline]
    fn from(v: Vec2) -> Self {
        glam::Vec2::new(v.x, v.y)
    }
}

impl From<glam::Vec3> for Vec3 {
    /// Converts a `glam::Vec3` (12 bytes, no padding) to a `Vec3` (16 bytes).
    ///
    /// The extra padding byte in `Vec3` is zeroed by [`Vec3::new`].
    #[inline]
    fn from(v: glam::Vec3) -> Self {
        Self::new(v.x, v.y, v.z)
    }
}

impl From<Vec3> for glam::Vec3 {
    /// Converts a `Vec3` to a `glam::Vec3`, dropping the internal padding byte.
    #[inline]
    fn from(v: Vec3) -> Self {
        glam::Vec3::new(v.x, v.y, v.z)
    }
}

impl From<glam::Vec3A> for Vec3 {
    /// Converts a `glam::Vec3A` (16-byte SIMD-aligned) to a `Vec3`.
    ///
    /// Both types share the same 16-byte alignment, so this is a direct component copy.
    #[inline]
    fn from(v: glam::Vec3A) -> Self {
        Self::new(v.x, v.y, v.z)
    }
}

impl From<Vec3> for glam::Vec3A {
    /// Converts a `Vec3` to a `glam::Vec3A` (16-byte SIMD-aligned).
    ///
    /// Both types share the same 16-byte alignment, so this is a direct component copy.
    #[inline]
    fn from(v: Vec3) -> Self {
        glam::Vec3A::new(v.x, v.y, v.z)
    }
}

impl From<glam::Vec4> for Vec4 {
    /// Converts a `glam::Vec4` to a `Vec4`, copying all four components.
    #[inline]
    fn from(v: glam::Vec4) -> Self {
        Self::new(v.x, v.y, v.z, v.w)
    }
}

impl From<Vec4> for glam::Vec4 {
    /// Converts a `Vec4` to a `glam::Vec4`, copying all four components.
    #[inline]
    fn from(v: Vec4) -> Self {
        glam::Vec4::new(v.x, v.y, v.z, v.w)
    }
}

// ==========================================
// 6. MINT INTEROPERABILITY (Bridge Pattern)
// ==========================================
#[cfg(feature = "simd")]
impl From<wide::f32x4> for Vec2 {
    #[inline]
    fn from(w: wide::f32x4) -> Self {
        let w = w.to_array();
        Self::new(w[0], w[1])
    }
}

#[cfg(feature = "simd")]
impl From<wide::f32x4> for Vec3 {
    #[inline]
    fn from(w: wide::f32x4) -> Self {
        let w = w.to_array();
        Self::new(w[0], w[1], w[2])
    }
}

#[cfg(feature = "simd")]
impl From<wide::f32x4> for Vec4 {
    #[inline]
    fn from(w: wide::f32x4) -> Self {
        let w = w.to_array();
        Self::new(w[0], w[1], w[2], w[3])
    }
}

#[cfg(feature = "simd")]
impl From<Vec4> for wide::f32x4 {
    #[inline]
    fn from(w: Vec4) -> Self {
        wide::f32x4::from([w.x, w.y, w.z, w.w])
    }
}

impl From<mint::Vector2<f32>> for Vec2 {
    #[inline]
    fn from(v: mint::Vector2<f32>) -> Self {
        Self::new(v.x, v.y)
    }
}

impl From<mint::Vector3<f32>> for Vec3 {
    #[inline]
    fn from(v: mint::Vector3<f32>) -> Self {
        Self::new(v.x, v.y, v.z)
    }
}

impl From<mint::Vector4<f32>> for Vec4 {
    #[inline]
    fn from(v: mint::Vector4<f32>) -> Self {
        Self::new(v.x, v.y, v.z, v.w)
    }
}

impl From<Vec2> for mint::Vector2<f32> {
    #[inline]
    fn from(v: Vec2) -> Self {
        mint::Vector2 { x: v.x, y: v.y }
    }
}

impl From<Vec3> for mint::Vector3<f32> {
    #[inline]
    fn from(v: Vec3) -> Self {
        mint::Vector3 {
            x: v.x,
            y: v.y,
            z: v.z,
        }
    }
}

impl From<Vec4> for mint::Vector4<f32> {
    #[inline]
    fn from(v: Vec4) -> Self {
        mint::Vector4 {
            x: v.x,
            y: v.y,
            z: v.z,
            w: v.w,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // ==========================================
    // FLOATING POINT COMPARISON MACRO
    // ==========================================
    // In game engines, floating point math can have tiny precision drifts.
    // We use an epsilon-based assertion instead of standard assert_eq!.
    macro_rules! assert_vec_eq {
        ($v1:expr, $v2:expr) => {
            assert!(($v1.x - $v2.x).abs() < 1e-5);
            assert!(($v1.y - $v2.y).abs() < 1e-5);
        };
        ($v1:expr, $v2:expr, 3) => {
            assert!(($v1.x - $v2.x).abs() < 1e-5);
            assert!(($v1.y - $v2.y).abs() < 1e-5);
            assert!(($v1.z - $v2.z).abs() < 1e-5);
        };
        ($v1:expr, $v2:expr, 4) => {
            assert!(($v1.x - $v2.x).abs() < 1e-5);
            assert!(($v1.y - $v2.y).abs() < 1e-5);
            assert!(($v1.z - $v2.z).abs() < 1e-5);
            assert!(($v1.w - $v2.w).abs() < 1e-5);
        };
    }

    // ==========================================
    // VEC2 TESTS
    // ==========================================
    #[test]
    fn test_vec2_constructors_and_dot() {
        let v1 = Vec2::new(2.0, 3.0);
        let v2 = Vec2::new(4.0, 5.0);

        assert_eq!(v1.x, 2.0);
        assert_eq!(v1.y, 3.0);
        assert_eq!(v1._padding, [0.0, 0.0]); // Ensure padding is safe

        assert!((v1.dot(v2) - 23.0).abs() < 1e-5); // (2*4) + (3*5) = 23
    }

    #[test]
    fn test_vec2_operations() {
        let mut v1 = Vec2::new(10.0, 20.0);
        let v2 = Vec2::new(2.0, 4.0);

        // Vector Math
        assert_vec_eq!(v1 + v2, Vec2::new(12.0, 24.0));
        assert_vec_eq!(v1 - v2, Vec2::new(8.0, 16.0));
        assert_vec_eq!(v1 * v2, Vec2::new(20.0, 80.0));
        assert_vec_eq!(v1 / v2, Vec2::new(5.0, 5.0));

        // Scalar Math
        assert_vec_eq!(v1 * 2.0, Vec2::new(20.0, 40.0));
        assert_vec_eq!(v1 / 2.0, Vec2::new(5.0, 10.0));

        // Assignments
        v1 += v2;
        assert_vec_eq!(v1, Vec2::new(12.0, 24.0));
        v1 -= v2;
        assert_vec_eq!(v1, Vec2::new(10.0, 20.0));
        v1 *= v2;
        assert_vec_eq!(v1, Vec2::new(20.0, 80.0));
        v1 /= v2;
        assert_vec_eq!(v1, Vec2::new(10.0, 20.0));
        v1 *= 2.0;
        assert_vec_eq!(v1, Vec2::new(20.0, 40.0));
        v1 /= 2.0;
        assert_vec_eq!(v1, Vec2::new(10.0, 20.0));
    }

    // ==========================================
    // VEC3 TESTS
    // ==========================================
    #[test]
    fn test_vec3_constructors_dot_and_cross() {
        let v1 = Vec3::new(1.0, 2.0, 3.0);
        let v2 = Vec3::new(4.0, 5.0, 6.0);

        assert_eq!(v1._padding, 0.0);

        assert!((v1.dot(v2) - 32.0).abs() < 1e-5); // 4 + 10 + 18 = 32

        // Cross Product: X axis x Y axis = Z axis
        let x_axis = Vec3::new(1.0, 0.0, 0.0);
        let y_axis = Vec3::new(0.0, 1.0, 0.0);
        assert_vec_eq!(x_axis.cross(y_axis), Vec3::new(0.0, 0.0, 1.0), 3);
    }

    #[test]
    fn test_vec3_swizzle() {
        let v = Vec3::new(1.0, 2.0, 3.0);
        assert_vec_eq!(v.xy(), Vec2::new(1.0, 2.0));
        assert_vec_eq!(v.xz(), Vec2::new(1.0, 3.0));
        assert_vec_eq!(v.yz(), Vec2::new(2.0, 3.0));
    }

    #[test]
    fn test_vec3_operations() {
        let mut v1 = Vec3::new(10.0, 20.0, 30.0);
        let v2 = Vec3::new(2.0, 4.0, 5.0);

        assert_vec_eq!(v1 + v2, Vec3::new(12.0, 24.0, 35.0), 3);
        assert_vec_eq!(v1 - v2, Vec3::new(8.0, 16.0, 25.0), 3);
        assert_vec_eq!(v1 * v2, Vec3::new(20.0, 80.0, 150.0), 3);
        assert_vec_eq!(v1 / v2, Vec3::new(5.0, 5.0, 6.0), 3);
        assert_vec_eq!(v1 * 2.0, Vec3::new(20.0, 40.0, 60.0), 3);
        assert_vec_eq!(v1 / 2.0, Vec3::new(5.0, 10.0, 15.0), 3);

        v1 += v2;
        assert_vec_eq!(v1, Vec3::new(12.0, 24.0, 35.0), 3);
        v1 -= v2;
        assert_vec_eq!(v1, Vec3::new(10.0, 20.0, 30.0), 3);
        v1 *= v2;
        assert_vec_eq!(v1, Vec3::new(20.0, 80.0, 150.0), 3);
        v1 /= v2;
        assert_vec_eq!(v1, Vec3::new(10.0, 20.0, 30.0), 3);
        v1 *= 2.0;
        assert_vec_eq!(v1, Vec3::new(20.0, 40.0, 60.0), 3);
        v1 /= 2.0;
        assert_vec_eq!(v1, Vec3::new(10.0, 20.0, 30.0), 3);
    }

    // ==========================================
    // VEC4 TESTS
    // ==========================================
    #[test]
    fn test_vec4_constructors_and_dot() {
        let v1 = Vec4::new(1.0, 2.0, 3.0, 4.0);
        let v2 = Vec4::new(5.0, 6.0, 7.0, 8.0);

        assert!((v1.dot(v2) - 70.0).abs() < 1e-5); // 5 + 12 + 21 + 32 = 70
    }

    #[test]
    fn test_vec4_swizzle() {
        let v = Vec4::new(1.0, 2.0, 3.0, 4.0);
        assert_vec_eq!(v.xyz(), Vec3::new(1.0, 2.0, 3.0), 3);
        assert_vec_eq!(v.xy(), Vec2::new(1.0, 2.0));
        assert_vec_eq!(v.zw(), Vec2::new(3.0, 4.0));
    }

    #[test]
    fn test_vec4_operations() {
        let mut v1 = Vec4::new(10.0, 20.0, 30.0, 40.0);
        let v2 = Vec4::new(2.0, 4.0, 5.0, 8.0);

        assert_vec_eq!(v1 + v2, Vec4::new(12.0, 24.0, 35.0, 48.0), 4);
        assert_vec_eq!(v1 - v2, Vec4::new(8.0, 16.0, 25.0, 32.0), 4);
        assert_vec_eq!(v1 * v2, Vec4::new(20.0, 80.0, 150.0, 320.0), 4);
        assert_vec_eq!(v1 / v2, Vec4::new(5.0, 5.0, 6.0, 5.0), 4);
        assert_vec_eq!(v1 * 2.0, Vec4::new(20.0, 40.0, 60.0, 80.0), 4);
        assert_vec_eq!(v1 / 2.0, Vec4::new(5.0, 10.0, 15.0, 20.0), 4);

        v1 += v2;
        assert_vec_eq!(v1, Vec4::new(12.0, 24.0, 35.0, 48.0), 4);
        v1 -= v2;
        assert_vec_eq!(v1, Vec4::new(10.0, 20.0, 30.0, 40.0), 4);
        v1 *= v2;
        assert_vec_eq!(v1, Vec4::new(20.0, 80.0, 150.0, 320.0), 4);
        v1 /= v2;
        assert_vec_eq!(v1, Vec4::new(10.0, 20.0, 30.0, 40.0), 4);
        v1 *= 2.0;
        assert_vec_eq!(v1, Vec4::new(20.0, 40.0, 60.0, 80.0), 4);
        v1 /= 2.0;
        assert_vec_eq!(v1, Vec4::new(10.0, 20.0, 30.0, 40.0), 4);
    }

    // ==========================================
    // INTEROPERABILITY TESTS
    // ==========================================
    #[test]
    fn test_mint_interop() {
        // Vec2
        let m2 = mint::Vector2 { x: 1.0, y: 2.0 };
        let v2: Vec2 = m2.into();
        assert_vec_eq!(v2, Vec2::new(1.0, 2.0));
        let m2_back: mint::Vector2<f32> = v2.into();
        assert_eq!(m2_back.x, 1.0);

        // Vec3
        let m3 = mint::Vector3 {
            x: 1.0,
            y: 2.0,
            z: 3.0,
        };
        let v3: Vec3 = m3.into();
        assert_vec_eq!(v3, Vec3::new(1.0, 2.0, 3.0), 3);
        let m3_back: mint::Vector3<f32> = v3.into();
        assert_eq!(m3_back.z, 3.0);

        // Vec4
        let m4 = mint::Vector4 {
            x: 1.0,
            y: 2.0,
            z: 3.0,
            w: 4.0,
        };
        let v4: Vec4 = m4.into();
        assert_vec_eq!(v4, Vec4::new(1.0, 2.0, 3.0, 4.0), 4);
        let m4_back: mint::Vector4<f32> = v4.into();
        assert_eq!(m4_back.w, 4.0);
    }

    #[cfg(feature = "simd")]
    #[test]
    fn test_simd_interop() {
        let f = wide::f32x4::from([1.0, 2.0, 3.0, 4.0]);

        let v2: Vec2 = f.into();
        assert_vec_eq!(v2, Vec2::new(1.0, 2.0));

        let v3: Vec3 = f.into();
        assert_vec_eq!(v3, Vec3::new(1.0, 2.0, 3.0), 3);

        let v4: Vec4 = f.into();
        assert_vec_eq!(v4, Vec4::new(1.0, 2.0, 3.0, 4.0), 4);
    }

    // ==========================================
    // GLAM INTEROP TESTS
    // ==========================================

    #[test]
    fn test_glam_vec2_roundtrip() {
        let original = Vec2::new(1.0, 2.0);
        let g: glam::Vec2 = original.into();
        let back: Vec2 = g.into();
        assert_vec_eq!(original, back);
        assert_eq!(g.x, 1.0);
        assert_eq!(g.y, 2.0);
    }

    #[test]
    fn test_glam_vec3_roundtrip() {
        let original = Vec3::new(1.0, 2.0, 3.0);
        let g: glam::Vec3 = original.into();
        let back: Vec3 = g.into();
        assert_vec_eq!(original, back, 3);
        // Padding byte is not exposed by glam — only x, y, z survive the roundtrip.
        assert_eq!(g.x, 1.0);
        assert_eq!(g.y, 2.0);
        assert_eq!(g.z, 3.0);
    }

    #[test]
    fn test_glam_vec3a_roundtrip() {
        let original = Vec3::new(4.0, 5.0, 6.0);
        let g: glam::Vec3A = original.into();
        let back: Vec3 = g.into();
        assert_vec_eq!(original, back, 3);
    }

    #[test]
    fn test_glam_vec3_and_vec3a_produce_same_components() {
        let v = Vec3::new(7.0, 8.0, 9.0);
        let g3: glam::Vec3 = v.into();
        let g3a: glam::Vec3A = v.into();
        assert_eq!(g3.x, g3a.x);
        assert_eq!(g3.y, g3a.y);
        assert_eq!(g3.z, g3a.z);
    }

    #[test]
    fn test_glam_vec4_roundtrip() {
        let original = Vec4::new(1.0, 2.0, 3.0, 4.0);
        let g: glam::Vec4 = original.into();
        let back: Vec4 = g.into();
        assert_vec_eq!(original, back, 4);
        assert_eq!(g.w, 4.0);
    }

    // ==========================================
    // EDGE CASES
    // ==========================================

    #[test]
    fn test_vec2_dot_orthogonal_is_zero() {
        let x = Vec2::new(1.0, 0.0);
        let y = Vec2::new(0.0, 1.0);
        assert!(x.dot(y).abs() < 1e-5);
    }

    #[test]
    fn test_vec2_dot_antiparallel_is_negative() {
        let v = Vec2::new(3.0, 0.0);
        let neg = Vec2::new(-3.0, 0.0);
        assert!(v.dot(neg) < 0.0);
    }

    #[test]
    fn test_vec3_dot_orthogonal_axes_are_zero() {
        let x = Vec3::new(1.0, 0.0, 0.0);
        let y = Vec3::new(0.0, 1.0, 0.0);
        let z = Vec3::new(0.0, 0.0, 1.0);
        assert!(x.dot(y).abs() < 1e-5);
        assert!(x.dot(z).abs() < 1e-5);
        assert!(y.dot(z).abs() < 1e-5);
    }

    #[test]
    fn test_vec3_cross_parallel_is_zero() {
        let v = Vec3::new(1.0, 2.0, 3.0);
        let parallel = Vec3::new(2.0, 4.0, 6.0);
        let result = v.cross(parallel);
        assert!(result.x.abs() < 1e-5);
        assert!(result.y.abs() < 1e-5);
        assert!(result.z.abs() < 1e-5);
    }

    #[test]
    fn test_vec3_cross_self_is_zero() {
        let v = Vec3::new(3.0, 7.0, -2.0);
        let result = v.cross(v);
        assert!(result.x.abs() < 1e-5);
        assert!(result.y.abs() < 1e-5);
        assert!(result.z.abs() < 1e-5);
    }

    #[test]
    fn test_vec3_cross_is_anti_commutative() {
        let a = Vec3::new(1.0, 2.0, 3.0);
        let b = Vec3::new(4.0, 5.0, 6.0);
        let ab = a.cross(b);
        let ba = b.cross(a);
        // a×b == -(b×a)
        assert!((ab.x + ba.x).abs() < 1e-5);
        assert!((ab.y + ba.y).abs() < 1e-5);
        assert!((ab.z + ba.z).abs() < 1e-5);
    }

    #[test]
    fn test_vec4_dot_orthogonal_is_zero() {
        let a = Vec4::new(1.0, 0.0, 0.0, 0.0);
        let b = Vec4::new(0.0, 1.0, 0.0, 0.0);
        assert!(a.dot(b).abs() < 1e-5);
    }

    #[test]
    fn test_vec4_dot_with_w_component() {
        // Dot includes the w component — a homogeneous point (w=1) dotted with a
        // direction (w=0) should ignore w, while two points accumulate it.
        let point = Vec4::new(1.0, 0.0, 0.0, 1.0);
        let dir = Vec4::new(1.0, 0.0, 0.0, 0.0);
        assert!((point.dot(dir) - 1.0).abs() < 1e-5);

        let point2 = Vec4::new(0.0, 0.0, 0.0, 1.0);
        assert!((point.dot(point2) - 1.0).abs() < 1e-5);
    }
}
