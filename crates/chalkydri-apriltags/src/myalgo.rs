use ril::{Line, Rgb};
use std::{
    alloc::{alloc, dealloc, Layout},
    ops::RangeBounds,
    sync::atomic::{AtomicUsize, Ordering},
    time::Instant,
};

use rayon::iter::{ParallelBridge, ParallelIterator};

/// Calculate buffer index for an x and y, given an image width
///
/// # Safety
/// `y` should be within the vertical bounds.
#[inline(always)]
const unsafe fn px(x: usize, y: usize, width: usize) -> usize {
    y.unchecked_mul(width).unchecked_add(x)
}

/// Convert a 24-bit RGB (color) value to a 8-bit luma/brightness (grayscale) value
#[inline(always)]
fn grayscale(data: &[u8]) -> u8 {
    if let &[r, g, b] = data {
        // Somebody else's ideal RGB conversion values:
        // (r as f32).mul_add(0.3, (g as f32).mul_add(0.59, (b as f32) * 0.11)) as u8

        // My "works I guess" RGB conversion values:
        // (r as f32).mul_add(0.2, (g as f32).mul_add(0.69, (b as f32) * 0.11)) as u8

        // An equal mix of R, G, and B is good here, because black is the absence of light.
        (r as f32).mul_add(0.33, (g as f32).mul_add(0.33, (b as f32) * 0.33)) as u8
    } else {
        panic!();
    }
}

/// Turns p1, p2, p3... into an approximate angle
#[rustfmt::skip]
#[inline(always)]
fn fast_angle(p: u8) -> f32 {
    match p {
        1  =>   0.0,
        2  =>  22.5,
        3  =>  45.0,
        4  =>  67.5,
        5  =>  90.0,
        6  => 112.5,
        7  => 135.0,
        8  => 157.5,
        9  => 180.0,
        10 => 202.5,
        11 => 225.0,
        12 => 247.5,
        13 => 270.0,
        14 => 292.5,
        15 => 315.0,
        16 => 337.5,
        _ => panic!("invalid FAST point")
    }
}

struct DetectorBufs {
    buf: *mut bool,
    points: *mut (usize, usize),
}
unsafe impl Send for DetectorBufs {}
unsafe impl Sync for DetectorBufs {}

/// AprilTag detector
pub struct Detector {
    bufs: DetectorBufs,
    points_len: AtomicUsize,
    lines: Vec<(usize, usize, usize, usize)>,
    width: usize,
    height: usize,
}
impl Detector {
    // Initialize a new detector for the specified dimensions
    pub fn new(width: usize, height: usize) -> Self {
        unsafe {
            let buf: *mut bool = alloc(Layout::array::<bool>(width * height).unwrap()).cast();
            let points: *mut (usize, usize) =
                alloc(Layout::array::<(usize, usize)>(width * height).unwrap()).cast();
            let points_len = AtomicUsize::new(0);

            Self {
                bufs: DetectorBufs { buf, points },
                points_len,
                lines: Vec::new(),
                width,
                height,
            }
        }
    }

    /// Calculate otsu value
    pub fn calc_otsu(&mut self, buf: &[u8]) {
        let end = buf.len() - 1;
        let mut i = 0usize;
        let mut hist = [0usize; 256];
        let st = Instant::now();

        while i < end {
            unsafe {
                let pix = hist.get_unchecked_mut(*buf.get_unchecked(i) as usize);
                *pix = (*pix).unchecked_add(1);
                i = i.unchecked_add(3);
            }
        }

        println!("{:?} {i}", st.elapsed());
    }

    /// Process an RGB frame
    ///
    /// FAST needs a 3x3 circle around each pixel, so we only process pixels within a 3x3 pixel
    /// padding.
    pub fn process_frame(&mut self, input: &[u8]) {
        // Check that the input is RGB
        assert_eq!(input.len(), self.width * self.height * 3);

        unsafe {
            self.thresh(input);
        }
        self.points_len.store(0, Ordering::SeqCst);

        //for x in 3..=self.width - 3 {
        //    for y in 3..=self.height - 3 {
        //        unsafe {
        //            self.process_pixel(x, y);
        //        }
        //    }
        //}

        (3..=self.width - 3).par_bridge().for_each(|x| {
            for y in 3..=self.height - 3 {
                unsafe {
                    self.process_pixel(x, y);
                }
            }
        });

        dbg!(self.points_len.load(Ordering::SeqCst));

        self.find_quads();
        self.draw();
    }

    /// Threshold an input RGB buffer
    ///
    /// # Safety
    ///
    /// `input` is treated as an RGB buffer, even if it isn't.
    /// The caller should check that `input` is an RGB buffer.
    #[inline(always)]
    unsafe fn thresh(&self, input: &[u8]) {
        for i in 0..self.width * self.height {
            // Red, green, and blue are each represent with 1 byte
            let gray = grayscale(input.get_unchecked((i * 3)..(i * 3) + 3));

            // 60 is a "kinda works" value because I haven't implemented the algorithm
            *self.bufs.buf.add(i) = gray < 60;
        }
    }

    /// Process a pixel
    ///
    /// This should have as little overhead as possible, as it must be run hundreds of thousands of
    /// times for each frame.
    ///
    /// # Safety
    ///
    /// (`x`, `y`) is assumed to be a valid pixel coord.
    /// The caller should make sure of this.
    #[inline(always)]
    unsafe fn process_pixel(&self, x: usize, y: usize) {
        let width = self.width;
        let buf = self.bufs.buf;

        // Get binary value of pixel at (x,y)
        let p = *buf.add(px(x, y, width));

        if p {
            let (up_left, up_right, down_left, down_right) = (
                *buf.add(px(x - 1, y - 1, width)),
                *buf.add(px(x + 1, y - 1, width)),
                *buf.add(px(x - 1, y + 1, width)),
                *buf.add(px(x + 1, y + 1, width)),
            );

            let clean = up_left ^ up_right ^ down_left ^ down_right;

            if clean {
                // Furthest top right
                let p3 = *buf.add(px(x + 3, y - 3, width));
                // Furthest bottom right
                let p7 = *buf.add(px(x + 3, y + 3, width));
                // Furthest bottom left
                let p11 = *buf.add(px(x - 3, y + 3, width));
                // Furthest top left
                let p15 = *buf.add(px(x - 3, y - 3, width));

                if p3 ^ p7 ^ p11 ^ p15 {
                    // Furthest top center
                    let p1 = *buf.add(px(x, y - 3, width));
                    // Furthest middle right
                    let p5 = *buf.add(px(x + 3, y, width));
                    // Furthest bottom center
                    let p9 = *buf.add(px(x, y + 3, width));
                    // Furthest middle left
                    let p13 = *buf.add(px(x - 3, y, width));

                    if (p1 && p5 && p3 && !p7 && !p11 && !p15)
                        || (p5 && p9 && !p3 && p7 && !p11 && !p15)
                        || (p9 && p13 && !p3 && !p7 && p11 && !p15)
                        || (p13 && p1 && !p3 && !p7 && !p11 && p15)
                    {
                        *self
                            .bufs
                            .points
                            .add(self.points_len.fetch_add(1, Ordering::SeqCst)) = (x, y);
                    }
                }
            }
        }
    }

    /// Find quadrilaterals
    fn find_quads(&mut self) {
        let points = unsafe {
            core::slice::from_raw_parts(
                self.bufs.points as *const _,
                self.points_len.load(Ordering::SeqCst),
            )
        };

        for &(x1, y1) in points {
            for &(x2, y2) in points.into_iter().filter(|(x2, y2)| x1 != *x2 && y1 != *y2) {
                if (x1 as i32 - 200..=x1 as i32 + 200).contains(&(x2 as i32))
                    ^ (y1 as i32 - 200..=y1 as i32 + 200).contains(&(y2 as i32))
                {
                    let distance = libm::sqrtf(
                        (x2 as f32 - x1 as f32).powf(2.0) + (y2 as f32 - y1 as f32).powf(2.0),
                    );
                    if distance > 0.0 {
                        //lines.push(distance as u32);
                        if !self.lines.contains(&(x2, y2, x1, y1)) {
                            self.lines.push((x1, y1, x2, y2).clone());
                        }
                        //println!("({x1}, {y1}) ({x2}, {y2}) => {distance}");
                    }
                }
            }
        }

        self.lines.sort();

        //println!("{:#?} {}", self.lines, self.lines.len());
    }

    fn draw(&self) {
        let mut img = ril::Image::new(self.width as u32, self.height as u32, Rgb::black());
        for (x1, y1, x2, y2) in self.lines.clone() {
            img.draw(&Line::new(
                (x1 as u32, y1 as u32),
                (x2 as u32, y2 as u32),
                Rgb::white(),
            ));
        }
        img.save_inferred("lines.png").unwrap();
    }
}
impl Drop for Detector {
    fn drop(&mut self) {
        unsafe {
            dealloc(
                self.bufs.buf as *mut _,
                Layout::array::<bool>(self.width * self.height).unwrap(),
            );
            dealloc(
                self.bufs.points as *mut _,
                Layout::array::<(usize, usize)>(self.width * self.height).unwrap(),
            );
        }
    }
}
