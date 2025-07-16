use std::time::{Duration, Instant};

use image::EncodableLayout;

fn main() {
    //chalkydri_apriltags::simd::sobel_shi_tomasi(1, 1, 74, 89);
    let mut img = image::open("test.png").unwrap();
    //img = img.blur(0.1);
    let mut imgg = img.to_rgb8();
    let mut img = imgg.to_vec();
    let img = img.as_mut_slice();

    let st = Instant::now();
    let mut det =
        chalkydri_apriltags::Detector::new(imgg.width() as usize, imgg.height() as usize, &[]);
    println!("{:?}", st.elapsed());

    let mut total = Duration::ZERO;
    for _ in 0..1_000 {
        let st = Instant::now();
        det.process_frame(img);
        det.draw();

        //let conn_comp = Instant::now();
        det.connected_components();
        //dbg!(conn_comp.elapsed());
        break;
        total += st.elapsed();
    }
    println!("{:?}", total / 1_000);

    drop(det);

    //chalkydri_apriltags::myalgo::myalgo(img);
    //chalkydri_apriltags::otsu::otsu(img);
    /*
    let mut ins = wgpu::Instance::new(wgpu::InstanceDescriptor {
        ..Default::default()
    });

    let adap = futures::executor::block_on(ins.request_adapter(&wgpu::RequestAdapterOptions {
        power_preference: wgpu::PowerPreference::HighPerformance,
        ..Default::default()
    }))
    .unwrap();

    if let Ok((dev, queue)) =
        futures::executor::block_on(adap.request_device(&DeviceDescriptor::default(), None))
    {
        chalkydri_apriltags::det(dev, queue);
    }
    */
}
