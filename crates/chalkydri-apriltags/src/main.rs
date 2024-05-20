use image::EncodableLayout;

fn main() {
    //chalkydri_apriltags::simd::sobel_shi_tomasi(1, 1, 74, 89);
    let mut img = image::open("test.png").unwrap();
    img = img.blur(2.0);
    let mut img = img.to_rgb8();
    let mut img = img.to_vec();
    let img = img.as_mut_slice();
    chalkydri_apriltags::myalgo::myalgo(img);
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
