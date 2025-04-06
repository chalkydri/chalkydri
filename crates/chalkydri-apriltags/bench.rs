use std::time::Instant;

use criterion::Criterion;

#[macro_use]
extern crate criterion;

fn simple_bench(c: &mut Criterion) {
    let mut img = image::open("test.png").unwrap();
    //img = img.blur(2.0);
    let mut img = img.to_rgb8();
    let mut img = img.to_vec();
    let img = img.as_mut_slice();

    let mut det = chalkydri_apriltags::Detector::new(703, 905, &[]);

    c.bench_function("simple bench", |b| {
        b.iter_custom(|iters| {
            let st = Instant::now();
            for _ in 0..iters {
                det.process_frame(img);
            }
            st.elapsed()
        });
    });
}

criterion_group!(benches, simple_bench);
criterion_main!(benches);
