use criterion::{Criterion, black_box, criterion_group, criterion_main};
// Replace `theos_math` if your crate name in Cargo.toml is different (e.g., uses a dash).
use theos_math::{Mat3, Mat4, Vec3, Vec4};

fn bench_mat4_mul(c: &mut Criterion) {
    let mut group = c.benchmark_group("Mat4");

    // We use somewhat arbitrary data to prevent the compiler from optimizing
    // a bunch of 0.0s or 1.0s away into a no-op.
    let m1 = Mat4::new(
        Vec4::new(1.0, 2.0, 3.0, 4.0),
        Vec4::new(5.0, 6.0, 7.0, 8.0),
        Vec4::new(9.0, 10.0, 11.0, 12.0),
        Vec4::new(13.0, 14.0, 15.0, 16.0),
    );
    let m2 = Mat4::new(
        Vec4::new(16.0, 15.0, 14.0, 13.0),
        Vec4::new(12.0, 11.0, 10.0, 9.0),
        Vec4::new(8.0, 7.0, 6.0, 5.0),
        Vec4::new(4.0, 3.0, 2.0, 1.0),
    );

    group.bench_function("Multiply", |b| {
        b.iter(|| {
            // black_box tells the Rust compiler: "Do not optimize this math away,
            // even if you know the answer at compile time!"
            black_box(black_box(m1) * black_box(m2))
        });
    });
    group.finish();
}

fn bench_mat3_mul(c: &mut Criterion) {
    let mut group = c.benchmark_group("Mat3");

    let m1 = Mat3::new(
        Vec3::new(1.0, 2.0, 3.0),
        Vec3::new(4.0, 5.0, 6.0),
        Vec3::new(7.0, 8.0, 9.0),
    );
    let m2 = Mat3::new(
        Vec3::new(9.0, 8.0, 7.0),
        Vec3::new(6.0, 5.0, 4.0),
        Vec3::new(3.0, 2.0, 1.0),
    );

    group.bench_function("Multiply", |b| {
        b.iter(|| black_box(black_box(m1) * black_box(m2)));
    });
    group.finish();
}

fn bench_vec3_dot(c: &mut Criterion) {
    let mut group = c.benchmark_group("Vec3");
    let a = Vec3::new(1.0, 2.0, 3.0);
    let b = Vec3::new(4.0, 5.0, 6.0);
    group.bench_function("Dot", |bench| {
        bench.iter(|| black_box(black_box(a).dot(black_box(b))))
    });
    group.finish();
}

fn bench_vec3_cross(c: &mut Criterion) {
    let mut group = c.benchmark_group("Vec3");
    let a = Vec3::new(1.0, 2.0, 3.0);
    let b = Vec3::new(4.0, 5.0, 6.0);
    group.bench_function("Cross", |bench| {
        bench.iter(|| black_box(black_box(a).cross(black_box(b))))
    });
    group.finish();
}

// Register and run the benchmarks
criterion_group!(
    benches,
    bench_mat4_mul,
    bench_mat3_mul,
    bench_vec3_dot,
    bench_vec3_cross
);
criterion_main!(benches);
