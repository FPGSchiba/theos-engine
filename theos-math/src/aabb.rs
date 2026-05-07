use crate::vec::Vec3;

/// An axis-aligned bounding box defined by its minimum and maximum corners.
///
/// All sides are parallel to the coordinate axes, which makes intersection and
/// containment tests cheap — just six scalar comparisons. Commonly used for
/// broad-phase collision detection and BVH ray traversal.
///
/// The invariant `min <= max` on every axis is enforced at construction time.
/// [`Aabb::new`] panics if this is violated.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Aabb {
    /// The corner with the smallest x, y, and z values.
    pub min: Vec3,
    /// The corner with the largest x, y, and z values.
    pub max: Vec3,
}

impl Aabb {
    /// Creates an AABB from explicit min and max corners.
    ///
    /// # Panics
    ///
    /// Panics if `min` is greater than `max` on any axis.
    pub fn new(min: Vec3, max: Vec3) -> Self {
        if min.x > max.x || min.y > max.y || min.z > max.z {
            panic!("Invalid AABB: min {:?} must be <= max {:?}", min, max);
        }
        Self { min, max }
    }

    /// Returns true if `point` lies inside or on the surface of this box.
    pub fn contains_point(&self, point: Vec3) -> bool {
        point.x >= self.min.x
            && point.x <= self.max.x
            && point.y >= self.min.y
            && point.y <= self.max.y
            && point.z >= self.min.z
            && point.z <= self.max.z
    }

    /// Returns true if this box overlaps `other`.
    ///
    /// Boxes that share only a face or edge are considered overlapping.
    pub fn overlaps(&self, other: &Aabb) -> bool {
        self.min.x <= other.max.x
            && self.max.x >= other.min.x
            && self.min.y <= other.max.y
            && self.max.y >= other.min.y
            && self.min.z <= other.max.z
            && self.max.z >= other.min.z
    }

    /// Returns the smallest AABB that contains both `self` and `other`.
    ///
    /// Equivalent to the union of the two boxes. Commutative: `a.merge(&b) == b.merge(&a)`.
    pub fn merge(&self, other: &Aabb) -> Aabb {
        let min = Vec3::new(
            self.min.x.min(other.min.x),
            self.min.y.min(other.min.y),
            self.min.z.min(other.min.z),
        );
        let max = Vec3::new(
            self.max.x.max(other.max.x),
            self.max.y.max(other.max.y),
            self.max.z.max(other.max.z),
        );
        Self { min, max }
    }

    /// Grows this box just enough to include `point`.
    ///
    /// If `point` is already inside the box the result is identical to `self`.
    /// Feed vertices one at a time to build an AABB from scratch.
    pub fn expand_to_contain(&self, point: Vec3) -> Aabb {
        let min = Vec3::new(
            self.min.x.min(point.x),
            self.min.y.min(point.y),
            self.min.z.min(point.z),
        );
        let max = Vec3::new(
            self.max.x.max(point.x),
            self.max.y.max(point.y),
            self.max.z.max(point.z),
        );
        Self { min, max }
    }

    /// Scales the box symmetrically around its center by `scalar`.
    ///
    /// A scalar of `2.0` doubles the extents in every direction while keeping
    /// the center fixed. A scalar of `0.5` halves them.
    pub fn scale(&self, scalar: f32) -> Aabb {
        let center = (self.min + self.max) * 0.5;
        let half_size = (self.max - self.min) * 0.5;
        let new_half_size = half_size * scalar;
        let new_min = center - new_half_size;
        let new_max = center + new_half_size;
        Self {
            min: new_min,
            max: new_max,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::vec::Vec3;

    fn aabb(min: (f32, f32, f32), max: (f32, f32, f32)) -> Aabb {
        Aabb::new(
            Vec3::new(min.0, min.1, min.2),
            Vec3::new(max.0, max.1, max.2),
        )
    }

    fn vec(x: f32, y: f32, z: f32) -> Vec3 {
        Vec3::new(x, y, z)
    }

    fn assert_vec_eq(a: Vec3, b: Vec3) {
        assert!((a.x - b.x).abs() < 1e-5, "x: {} != {}", a.x, b.x);
        assert!((a.y - b.y).abs() < 1e-5, "y: {} != {}", a.y, b.y);
        assert!((a.z - b.z).abs() < 1e-5, "z: {} != {}", a.z, b.z);
    }

    // ==========================================
    // CONSTRUCTION
    // ==========================================

    #[test]
    fn test_new_stores_corners() {
        let b = aabb((1.0, 2.0, 3.0), (4.0, 5.0, 6.0));
        assert_vec_eq(b.min, vec(1.0, 2.0, 3.0));
        assert_vec_eq(b.max, vec(4.0, 5.0, 6.0));
    }

    #[test]
    fn test_new_degenerate_point_box_is_valid() {
        // A box where min == max is a single point — valid.
        let b = aabb((2.0, 2.0, 2.0), (2.0, 2.0, 2.0));
        assert_vec_eq(b.min, b.max);
    }

    #[test]
    #[should_panic(expected = "Invalid AABB")]
    fn test_new_panics_when_x_inverted() {
        aabb((5.0, 0.0, 0.0), (1.0, 1.0, 1.0));
    }

    #[test]
    #[should_panic(expected = "Invalid AABB")]
    fn test_new_panics_when_only_one_axis_inverted() {
        // Only y is inverted — the whole-vector-swap approach would miss this.
        aabb((0.0, 5.0, 0.0), (1.0, 1.0, 1.0));
    }

    // ==========================================
    // CONTAINS POINT
    // ==========================================

    #[test]
    fn test_contains_point_interior() {
        let b = aabb((0.0, 0.0, 0.0), (4.0, 4.0, 4.0));
        assert!(b.contains_point(vec(2.0, 2.0, 2.0)));
    }

    #[test]
    fn test_contains_point_on_surface_is_inside() {
        let b = aabb((0.0, 0.0, 0.0), (4.0, 4.0, 4.0));
        assert!(b.contains_point(vec(0.0, 2.0, 2.0))); // on min face
        assert!(b.contains_point(vec(4.0, 2.0, 2.0))); // on max face
        assert!(b.contains_point(vec(0.0, 0.0, 0.0))); // min corner
        assert!(b.contains_point(vec(4.0, 4.0, 4.0))); // max corner
    }

    #[test]
    fn test_contains_point_outside() {
        let b = aabb((0.0, 0.0, 0.0), (4.0, 4.0, 4.0));
        assert!(!b.contains_point(vec(5.0, 2.0, 2.0))); // beyond max x
        assert!(!b.contains_point(vec(2.0, -1.0, 2.0))); // below min y
        assert!(!b.contains_point(vec(2.0, 2.0, 10.0))); // beyond max z
    }

    // ==========================================
    // OVERLAPS
    // ==========================================

    #[test]
    fn test_overlaps_partial_intersection() {
        let a = aabb((0.0, 0.0, 0.0), (3.0, 3.0, 3.0));
        let b = aabb((2.0, 2.0, 2.0), (5.0, 5.0, 5.0));
        assert!(a.overlaps(&b));
        assert!(b.overlaps(&a)); // symmetric
    }

    #[test]
    fn test_overlaps_touching_faces_counts_as_overlap() {
        let a = aabb((0.0, 0.0, 0.0), (1.0, 1.0, 1.0));
        let b = aabb((1.0, 0.0, 0.0), (2.0, 1.0, 1.0));
        assert!(a.overlaps(&b));
    }

    #[test]
    fn test_overlaps_separated_on_one_axis() {
        let a = aabb((0.0, 0.0, 0.0), (1.0, 1.0, 1.0));
        let b = aabb((2.0, 0.0, 0.0), (3.0, 1.0, 1.0)); // gap on x
        assert!(!a.overlaps(&b));
    }

    #[test]
    fn test_overlaps_one_box_inside_another() {
        let outer = aabb((0.0, 0.0, 0.0), (10.0, 10.0, 10.0));
        let inner = aabb((2.0, 2.0, 2.0), (4.0, 4.0, 4.0));
        assert!(outer.overlaps(&inner));
        assert!(inner.overlaps(&outer));
    }

    #[test]
    fn test_overlaps_self() {
        let b = aabb((0.0, 0.0, 0.0), (1.0, 1.0, 1.0));
        assert!(b.overlaps(&b));
    }

    // ==========================================
    // MERGE
    // ==========================================

    #[test]
    fn test_merge_produces_bounding_box() {
        let a = aabb((0.0, 0.0, 0.0), (2.0, 2.0, 2.0));
        let b = aabb((1.0, 1.0, 1.0), (4.0, 3.0, 5.0));
        let merged = a.merge(&b);
        assert_vec_eq(merged.min, vec(0.0, 0.0, 0.0));
        assert_vec_eq(merged.max, vec(4.0, 3.0, 5.0));
    }

    #[test]
    fn test_merge_with_self_is_noop() {
        let b = aabb((1.0, 2.0, 3.0), (4.0, 5.0, 6.0));
        assert_eq!(b.merge(&b), b);
    }

    #[test]
    fn test_merge_is_symmetric() {
        let a = aabb((0.0, 5.0, 0.0), (3.0, 8.0, 3.0));
        let b = aabb((1.0, 0.0, 1.0), (5.0, 6.0, 5.0));
        assert_eq!(a.merge(&b), b.merge(&a));
    }

    // ==========================================
    // EXPAND TO CONTAIN
    // ==========================================

    #[test]
    fn test_expand_to_contain_point_outside() {
        let b = aabb((0.0, 0.0, 0.0), (1.0, 1.0, 1.0));
        let expanded = b.expand_to_contain(vec(3.0, -1.0, 2.0));
        assert_vec_eq(expanded.min, vec(0.0, -1.0, 0.0));
        assert_vec_eq(expanded.max, vec(3.0, 1.0, 2.0));
    }

    #[test]
    fn test_expand_to_contain_point_inside_is_noop() {
        let b = aabb((0.0, 0.0, 0.0), (4.0, 4.0, 4.0));
        let expanded = b.expand_to_contain(vec(2.0, 2.0, 2.0));
        assert_eq!(expanded, b);
    }

    // ==========================================
    // SCALE
    // ==========================================

    #[test]
    fn test_scale_doubles_extents_symmetrically() {
        let b = aabb((0.0, 0.0, 0.0), (2.0, 2.0, 2.0));
        // Center is (1,1,1), half-extents are (1,1,1). Scaling by 2 -> half=(2,2,2).
        let scaled = b.scale(2.0);
        assert_vec_eq(scaled.min, vec(-1.0, -1.0, -1.0));
        assert_vec_eq(scaled.max, vec(3.0, 3.0, 3.0));
    }

    #[test]
    fn test_scale_by_one_is_noop() {
        let b = aabb((1.0, 2.0, 3.0), (4.0, 5.0, 6.0));
        let scaled = b.scale(1.0);
        assert_vec_eq(scaled.min, b.min);
        assert_vec_eq(scaled.max, b.max);
    }

    #[test]
    fn test_scale_center_does_not_move() {
        let b = aabb((0.0, 0.0, 0.0), (4.0, 4.0, 4.0));
        let scaled = b.scale(3.0);
        let original_center = (b.min + b.max) * 0.5;
        let scaled_center = (scaled.min + scaled.max) * 0.5;
        assert_vec_eq(original_center, scaled_center);
    }
}
