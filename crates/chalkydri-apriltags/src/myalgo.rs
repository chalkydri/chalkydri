//!
//!
//!

use std::{
    alloc::{alloc, Layout},
    cell::{SyncUnsafeCell, UnsafeCell},
    simd::{f32x4, f32x64, mask64x8, mask8x8, prelude::SimdFloat, u16x64},
    sync::Arc,
    time::Instant,
};

use image::{GenericImage, GrayImage, Luma};
use rayon::prelude::*;

#[rustfmt::skip]
const COEFFS: f32x64 = f32x64::from_array([
    0.3, 0.59, 0.11, 0.0,
    0.3, 0.59, 0.11, 0.0,
    0.3, 0.59, 0.11, 0.0,
    0.3, 0.59, 0.11, 0.0,
    0.3, 0.59, 0.11, 0.0,
    0.3, 0.59, 0.11, 0.0,
    0.3, 0.59, 0.11, 0.0,
    0.3, 0.59, 0.11, 0.0,
    0.3, 0.59, 0.11, 0.0,
    0.3, 0.59, 0.11, 0.0,
    0.3, 0.59, 0.11, 0.0,
    0.3, 0.59, 0.11, 0.0,
    0.3, 0.59, 0.11, 0.0,
    0.3, 0.59, 0.11, 0.0,
    0.3, 0.59, 0.11, 0.0,
    0.3, 0.59, 0.11, 0.0,
]);

#[inline(always)]
const fn px(x: usize, y: usize, width: usize) -> usize {
    y * width + x
}

#[inline(always)]
fn grayscale(data: &[u8]) -> u8 {
    if let &[r, g, b] = data {
        //(r as f32).mul_add(0.3, (g as f32).mul_add(0.59, (b as f32) * 0.11)) as u8
        (r as f32).mul_add(0.33, (g as f32).mul_add(0.33, (b as f32) * 0.33)) as u8
        //(r as f32).mul_add(0.2, (g as f32).mul_add(0.69, (b as f32) * 0.11)) as u8
    } else {
        panic!();
    }
}

struct Buffer {
    inner: UnsafeCell<*mut bool>,
}
unsafe impl Send for Buffer {}
unsafe impl Sync for Buffer {}

pub fn myalgo(buf: &mut [u8]) {
    let mut img = GrayImage::new(1080, 720);
    let mut img2 = GrayImage::new(1080, 720);
    let st = Instant::now();
    let end = buf.len() - 1;
    let mut i = 0usize;
    //let mut hist = [0usize; 256];

    let width = 1080;
    let height = 720;

    let graybuf: *mut bool =
        unsafe { alloc(Layout::array::<bool>(width * height).unwrap()).cast() };

    let st = Instant::now();

    /*
    for &[r, g, b] in buf.array_chunks::<3>() {
        //let grayscale = ((r as f32 * 0.3) + (g as f32 * 0.59) + (b as f32 * 0.11)) as u8;
        let grayscale = (r as f32).mul_add(0.3, (g as f32).mul_add(0.59, (b as f32) * 0.11)) as u8;
        unsafe {
            let pix = hist.get_unchecked_mut(grayscale as usize);
            //let pix = hist.get_unchecked_mut(r as usize);
            *pix = (*pix).unchecked_add(1);
            i = i.unchecked_add(3);
        }
    }
    */

    //for &[r, g, b] in buf.array_chunks::<3>() {

    unsafe {
        for i in 0..width * height {
            let gray = grayscale(buf.get_unchecked((i * 3)..(i * 3) + 3));
            img2.put_pixel(
                (i % 1080).try_into().unwrap(),
                (i / 1080).try_into().unwrap(),
                Luma([gray]),
            );
            //let grayscale = ((r as f32 * 0.3) + (g as f32 * 0.59) + (b as f32 * 0.11)) as u8;
            //let grayscale = (r as f32).mul_add(0.3, (g as f32).mul_add(0.59, (b as f32) * 0.11)) as u8;
            //let pix = hist.get_unchecked_mut(grayscale as usize);
            //let pix = hist.get_unchecked_mut(r as usize);
            //*pix = (*pix).unchecked_add(1);

            *graybuf.add(i) = gray < 60;
        }
    }

    println!("{:?} {i} {{hist:?}}", st.elapsed());

    let st = Instant::now();

    unsafe {
        //(3..=width - 3).par_bridge().for_each(|x| {
        for x in 3..=width - 3 {
            //(3..=height - 3).par_bridge().for_each(|y| {
            for y in 3..=height - 3 {
                let mut ct = 0u8;

                /*
                let p1 = px(x, y - 3, width) * 3;
                let p5 = px(x + 3, y, width) * 3;
                let p9 = px(x, y + 3, width) * 3;
                let p13 = px(x - 3, y, width) * 3;
                let p1 = grayscale(buf.get_unchecked(p1..p1 + 3));
                let p5 = grayscale(buf.get_unchecked(p5..p5 + 3));
                let p9 = grayscale(buf.get_unchecked(p9..p9 + 3));
                let p13 = grayscale(buf.get_unchecked(p13..p13 + 3));
                */

                let p = *graybuf.add(px(x, y, width));
                if p {
                    let p3 = *graybuf.add(px(x + 3, y - 3, width));
                    let p7 = *graybuf.add(px(x + 3, y + 3, width));
                    let p11 = *graybuf.add(px(x - 3, y + 3, width));
                    let p15 = *graybuf.add(px(x - 3, y - 3, width));

                    /*
                    if (!p1 && p5 && p9 && p13)
                        || (p1 && !p5 && p9 && p13)
                        || (p1 && p5 && !p9 && p13)
                        || (p1 && p5 && p9 && !p13)
                    */
                    if (p3 && !p7 && !p11 && !p15)
                        || (!p3 && p7 && !p11 && !p15)
                        || (!p3 && !p7 && p11 && !p15)
                        || (!p3 && !p7 && !p11 && p15)
                    {
                        //if p3 {
                        let p2 = *graybuf.add(px(x + 1, y - 3, width));
                        //
                        let p1 = *graybuf.add(px(x, y - 3, width));
                        let p5 = *graybuf.add(px(x + 3, y, width));
                        let p9 = *graybuf.add(px(x, y + 3, width));
                        let p13 = *graybuf.add(px(x - 3, y, width));

                        if (p1 && p5 && p3 && !p7 && !p11 && !p15)
                            || (p5 && p9 && !p3 && p7 && !p11 && !p15)
                            || (p9 && p13 && !p3 && !p7 && p11 && !p15)
                            || (p13 && p1 && !p3 && !p7 && !p11 && p15)
                        {
                            /*
                            for p in [p1, p5, p9, p13] {
                                if p {
                                    ct += 1;
                                }
                            }

                            if ct == 3 {
                            */
                            img.put_pixel(x as u32, y as u32, Luma([255]));
                            //*graybuf.add(px(x, y, width)) = true;
                            println!(">>>> {x}, {y} ({p1} {p5} {p9} {p13})");
                            //}
                        }
                    }
                }
            }
        }
    }

    println!("{:?} {i}", st.elapsed());

    img.save("outout.png").unwrap();
    img2.save("outgray.png").unwrap();
}
