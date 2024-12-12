#![feature(
    portable_simd,
    alloc_layout_extra,
    slice_as_chunks,
    sync_unsafe_cell,
    array_chunks
)]
#![warn(clippy::infinite_loop)]

#[cfg(feature = "multi-thread")]
extern crate rayon;

//mod decode;
// mod pose_estimation;
pub mod utils;

use cam_geom::IntrinsicParametersPerspective;
// use pose_estimation::pose_estimation;
use ril::{Line, Rgb};
// TODO: ideally we'd use alloc here and only pull in libstd for sync::atomic when the multi-thread feature is enabled
use std::{
    alloc::{alloc_zeroed, dealloc, Layout},
    sync::atomic::{AtomicUsize, Ordering},
};

#[cfg(feature = "multi-thread")]
use rayon::iter::{ParallelBridge, ParallelIterator};

use crate::utils::*;

/// Raw buffers used by a [`detector`](Detector)
///
/// We need a separate struct for this so the compiler will treat them as thread-safe.
/// Interacting with raw buffers is typically lower overhead, but unsafe.
struct DetectorBufs {
    /// The thresholded image buffer
    buf: *mut Color,
    /// Detected corners
    points: *mut (usize, usize),
}
unsafe impl Send for DetectorBufs {}
unsafe impl Sync for DetectorBufs {}

/// An AprilTag detector
///
/// This is the main entrypoint.
pub struct Detector {
    /// Raw buffers used by the detector
    bufs: DetectorBufs,
    valid_tags: &'static [usize],
    points_len: AtomicUsize,
    /// Checked edges (x1, y1, x2, y2)
    lines: Vec<(usize, usize, usize, usize)>,
    /// Width of input frames
    width: usize,
    /// Height of input frames
    height: usize,
}
impl Detector {
    /// Initialize a new detector for the specified dimensions
    ///
    /// `valid_tags` is required for optimization and error resistance.
    pub fn new(
        width: usize,
        height: usize,
        valid_tags: &'static [usize],
        //intrinsics: IntrinsicParametersPerspective<f32>,
    ) -> Self {
        unsafe {
            // Allocate raw buffers
            let buf: *mut Color =
                alloc_zeroed(Layout::array::<Color>(width * height).unwrap()).cast();
            let points: *mut (usize, usize) =
                alloc_zeroed(Layout::array::<(usize, usize)>(width * height).unwrap()).cast();
            let points_len = AtomicUsize::new(0);

            Self {
                bufs: DetectorBufs { buf, points },
                valid_tags,
                points_len,
                lines: Vec::new(),
                width,
                height,
            }
        }
    }

    /// Calculate otsu value
    ///
    /// [Otsu's method](https://en.wikipedia.org/wiki/Otsu%27s_method) is an adaptive thresholding
    /// algorithm. In English: it turns a grayscale image into binary (foreground/background,
    /// black/white).
    ///
    /// We should investigate combining the variations for unbalanced images and triclass
    /// thresholding.
    pub fn calc_otsu(&mut self, input: &[u8]) {
        let mut i = 0usize;
        // Histogram
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
        // Reset points_len to 0
        self.points_len.store(0, Ordering::SeqCst);
        // Clear the lines Vec
        self.lines.clear();

        self.detect_corners();

        self.check_edges();

        //pose_estimation(intrinsics);
    }

    /// Run corner detection
    #[inline(always)]
    pub fn detect_corners(&mut self) {
        #[cfg(not(feature = "multi-thread"))]
        for x in 3..=self.width - 3 {
            for y in 3..=self.height - 3 {
                unsafe {
                    self.process_pixel(x, y);
                }
            }
        }

        #[cfg(feature = "multi-thread")]
        (3..=self.width - 3).par_bridge().for_each(|x| {
            for y in 3..=self.height - 3 {
                unsafe {
                    self.process_pixel(x, y);
                }
            }
        });
    }

    /// Threshold an input RGB buffer
    ///
    /// TODO: This needs to use [Self::calc_otsu].
    ///
    /// # Safety
    /// `input` is treated as an RGB buffer, even if it isn't.
    /// The caller should check that `input` is an RGB buffer.
    #[inline(always)]
    pub unsafe fn thresh(&self, input: &[u8]) {
        // This is mainly memory-bound, so multi-threading probably isn't worth it.
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
    /// The caller must make sure of this.
    #[inline(always)]
    unsafe fn process_pixel(&self, x: usize, y: usize) {
        // Pull out frame width and frame buffer for cleaner looking code
        // TODO: is this optimized down into a noop?
        let width = self.width;
        let buf = self.bufs.buf;

        // Get binary value of pixel at (x,y)
        let p = *buf.add(px(x, y, width));

        if p.is_black() {
            // Get pixels that are diagonal neighbors of p
            let (up_left, up_right, down_left, down_right) = (
                *buf.add(px(x - 1, y - 1, width)),
                *buf.add(px(x + 1, y - 1, width)),
                *buf.add(px(x - 1, y + 1, width)),
                *buf.add(px(x + 1, y + 1, width)),
            );

            // Only one can be black
            // The carrot is Rust's exclusive or (XOR) operation
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

                    // Add p to the corner buffer
                    *self
                        .bufs
                        .points
                        .add(self.points_len.fetch_add(1, Ordering::SeqCst)) = (x, y);
                }
            }
        }
    }

    /// Check a single edge (imaginary line between two corners)
    ///
    /// See [Self::check_edges].
    ///
    /// # Safety
    /// (`x1`, `y1`) and (`x2`, `y2`) are assumed to be a valid pixel coords.
    /// The caller must make sure of this.
    unsafe fn check_edge(&mut self, x1: usize, y1: usize, x2: usize, y2: usize) {
        // idk how to describe this one
        const CHECK_OFFSET: usize = 5;
        let width = self.width;
        let buf = self.bufs.buf;

        // calculate & store midpoint
        let midpoint_x = (x1 + x2) / 2;
        let midpoint_y = (y1 + y2) / 2;

        // Figure out if edge is closer to horizontal/vertical
        let (xdiff, ydiff) = (x1.max(x2) - x1.min(x2), y1.max(y2) - y1.min(y2));
        let is_vertical_line = x1 == x2 || xdiff < ydiff;
        let is_horizontal_line = y1 == y2 || ydiff < xdiff;

        // Calculate and store the coords for the midway points
        let (mw1x, mw1y) = ((midpoint_x + x1) / 2, (midpoint_y + y1) / 2);
        let (mw2x, mw2y) = ((midpoint_x + x2) / 2, (midpoint_y + y2) / 2);

        if is_vertical_line {
            // edge is closer to a vertical line instead of a diagonal
            let mw1right = *buf.add(px(mw1x + CHECK_OFFSET, mw1y, width));
            let mw2right = *buf.add(px(mw2x + CHECK_OFFSET, mw2y, width));

            let mw1left = *buf.add(px(mw1x - CHECK_OFFSET, mw1y, width));
            let mw2left = *buf.add(px(mw2x - CHECK_OFFSET, mw2y, width));

            // Check that all of the checking points are valid
            if mw1left.is_good() && mw2left.is_good() && mw1right.is_good() && mw2right.is_good() {
                // Check that only one side of the edge is black (the other should be white)
                if (mw1left.is_black() ^ mw2right.is_black())
                    && (mw2left.is_black() ^ mw1right.is_black())
                    && (mw1left == mw2left)
                {
                    // midway one has black pixels on both sides
                    self.lines.push((x1, y1, x2, y2));
                }
            }
        }

        if is_horizontal_line {
            // edge is closer to a horizontal line instead of a diagonal

            // XXX: Checking midway 1 then midway 2 *might* have marginally better performance,
            // but likely not worth it for the more complex code.

            // create the point to the right of the two midways
            let mw1top = *buf.add(px(mw1x, mw1y - CHECK_OFFSET, width));
            let mw2top = *buf.add(px(mw2x, mw2y - CHECK_OFFSET, width));

            // create the point ot the left of the two midways
            let mw1bottom = *buf.add(px(mw1x, mw1y + CHECK_OFFSET, width));
            let mw2bottom = *buf.add(px(mw2x, mw2y + CHECK_OFFSET, width));

            // check if the midways are black pixels,
            // and if the pixels to the right and left of these midways are black pixels as well.

            if mw1top.is_good() && mw2top.is_good() && mw1bottom.is_good() && mw2bottom.is_good() {
                if (mw1top.is_black() ^ mw2bottom.is_black())
                    && (mw2top.is_black() ^ mw1bottom.is_black())
                    && (mw1top == mw2top)
                {
                    // midway one has black pixels on both sides
                    self.lines.push((x1, y1, x2, y2));
                }
            }
        }
    }

    /// Perform edge checking on all detected corners
    #[inline(always)]
    pub fn check_edges(&mut self) {
        // Turn the raw buffer into a Rust slice
        let points = unsafe {
            core::slice::from_raw_parts(
                self.bufs.points as *const _,
                self.points_len.load(Ordering::SeqCst),
            )
        };

        // Iterate over every detected corner
        // TODO: this might benefit from multi-threading
        for &(x1, y1) in points.iter() {
            // Iterate over every detected corner in reverse, checking for edges
            for &(x2, y2) in points.iter().rev() {
                unsafe {
                    self.check_edge(x1, y1, x2, y2);
                }
            }
        }
    }

    pub fn draw(&self) {
        let mut img = ril::Image::new(self.width as u32, self.height as u32, Rgb::black());
        for (x1, y1, x2, y2) in self.lines.clone() {
            img.draw(
                &ril::draw::Ellipse::circle(x1 as u32, y1 as u32, 2)
                    .with_fill(Rgb::from_hex("ffa500").unwrap()),
            );
            img.draw(
                &ril::draw::Ellipse::circle(x2 as u32, y2 as u32, 2)
                    .with_fill(Rgb::from_hex("ff0000").unwrap()),
            );
            img.draw(&Line::new(
                (x1 as u32, y1 as u32),
                (x2 as u32, y2 as u32),
                Rgb::white(),
            ));
        }
        img.save(ril::ImageFormat::Png, "lines.png").unwrap();
    }
}
impl Clone for Detector {
    fn clone(&self) -> Self {
        Self::new(self.width, self.height, &[])
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
