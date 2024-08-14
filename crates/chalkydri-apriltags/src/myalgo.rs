use ril::{gradient::RadialGradientFill, Line, RadialGradient, Rgb};
use std::{
    alloc::{alloc, dealloc, Layout},
    ops::RangeBounds,
    simd::usizex4,
    sync::atomic::{AtomicUsize, Ordering},
    time::Instant,
};

use rayon::iter::{ParallelBridge, ParallelIterator};

use crate::utils::PresentWrapper;

#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Color {
    Black,
    White,
    Other,
}
impl Color {
    /// Whether the color is black (like the tag markings)
    #[inline(always)]
    pub fn is_black(&self) -> bool {
        *self == Color::Black
    }
    /// Whether the color is white (like the paper)
    #[inline(always)]
    pub fn is_white(&self) -> bool {
        *self == Color::White
    }
    /// Whether the color is relevant to tag detection
    #[inline(always)]
    pub fn is_good(&self) -> bool {
        *self != Color::Other
    }
}

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
    buf: *mut Color,
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
            // Allocate
            let buf: *mut Color = alloc(Layout::array::<Color>(width * height).unwrap()).cast();
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
    pub fn calc_otsu(&mut self, input: &[u8]) {
        let mut i = 0usize;
        let mut hist = [0usize; 256];

        // Calculate histogram
        for i in 0..self.width * self.height {
            unsafe {
                // Red, green, and blue are each represent with 1 byte
                let gray = grayscale(input.get_unchecked((i * 3)..(i * 3) + 3));

                let pix = hist.get_unchecked_mut(*input.get_unchecked(i) as usize);
                *pix = (*pix).unchecked_add(1);
            }
        }

        let mut sum = 0u32;
        let mut sum_b = 0u32;
        let mut var_max = 0f64;
        let mut thresh = 0u8;

        //for t in 0..256 {
        //    sum += t as u32 * hist[t];

        //    let w_b =

        //println!("{:?} {i}", st.elapsed());
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
        self.lines.clear();

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

        self.find_quads();
        //self.draw();
    }

    /// Threshold an input RGB buffer
    ///
    /// # Safety
    /// `input` is treated as an RGB buffer, even if it isn't.
    /// The caller should check that `input` is an RGB buffer.
    #[inline(always)]
    unsafe fn thresh(&self, input: &[u8]) {
        for i in 0..self.width * self.height {
            // Red, green, and blue are each represent with 1 byte
            let gray = grayscale(input.get_unchecked((i * 3)..(i * 3) + 3));

            // 60 is a "kinda works" value because I haven't implemented the algorithm
            if gray < 60 {
                *self.bufs.buf.add(i) = Color::Black;
            } else if gray > 160 {
                *self.bufs.buf.add(i) = Color::White;
            } else {
                *self.bufs.buf.add(i) = Color::Other;
            }
        }
    }

    /// Process a pixel
    ///
    /// This should have as little overhead as possible, as it must be run hundreds of thousands of
    /// times for each frame.
    ///
    /// # Safety
    /// (`x`, `y`) is assumed to be a valid pixel coord.
    /// The caller should make sure of this.
    #[inline(always)]
    unsafe fn process_pixel(&self, x: usize, y: usize) {
        let width = self.width;
        let buf = self.bufs.buf;

        // Get binary value of pixel at (x,y)
        let p = *buf.add(px(x, y, width));

        if p.is_black() {
            let (up_left, up_right, down_left, down_right) = (
                *buf.add(px(x - 1, y - 1, width)),
                *buf.add(px(x + 1, y - 1, width)),
                *buf.add(px(x - 1, y + 1, width)),
                *buf.add(px(x + 1, y + 1, width)),
            );

            let clean = up_left.is_black()
                ^ up_right.is_black()
                ^ down_left.is_black()
                ^ down_right.is_black();

            if clean {
                // Furthest top right
                let p3 = *buf.add(px(x + 3, y - 3, width));
                // Furthest bottom right
                let p7 = *buf.add(px(x + 3, y + 3, width));
                // Furthest bottom left
                let p11 = *buf.add(px(x - 3, y + 3, width));
                // Furthest top left
                let p15 = *buf.add(px(x - 3, y - 3, width));

                if (p3.is_good() && p7.is_good() && p11.is_good() && p15.is_good())
                    && (p3.is_black() ^ p7.is_black() ^ p11.is_black() ^ p15.is_black())
                {
                    // Furthest top center
                    let p1 = *buf.add(px(x, y - 3, width));
                    // Furthest middle right
                    let p5 = *buf.add(px(x + 3, y, width));
                    // Furthest bottom center
                    let p9 = *buf.add(px(x, y + 3, width));
                    // Furthest middle left
                    let p13 = *buf.add(px(x - 3, y, width));

                    *self
                        .bufs
                        .points
                        .add(self.points_len.fetch_add(1, Ordering::SeqCst)) = (x, y);
                }
            }
        }
    }

    /// Find quadrilaterals
    #[inline(always)]
    fn find_quads(&mut self) {
        let points = unsafe {
            core::slice::from_raw_parts(
                self.bufs.points as *const _,
                self.points_len.load(Ordering::SeqCst),
            )
        };

        let mut hull: Vec<(usize, usize)> = PresentWrapper::find_convex_hull(points);

        for p in hull {
            self.lines.push((p.0, p.1, p.0, p.1));
        }
    }

    fn draw(&self) {
        let mut img = ril::Image::new(self.width as u32, self.height as u32, Rgb::black());
        for (x1, y1, x2, y2) in self.lines.clone() {
            img.draw(&ril::draw::Ellipse::circle(x1 as u32, y1 as u32, 1).with_fill(Rgb::white()));
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
