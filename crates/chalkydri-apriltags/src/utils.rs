#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Color {
    Black,
    White,
    Other,
}
impl Color {
    #[inline(always)]
    pub fn is_black(&self) -> bool {
        *self == Color::Black
    }
    #[inline(always)]
    pub fn is_white(&self) -> bool {
        *self == Color::White
    }
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

#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Orientation {
    Collinear,
    Clockwise,
    Counterclockwise,
}

/// Calculate the orientation
#[inline(always)]
pub fn orientation(
    (px, py): (usize, usize),
    (qx, qy): (usize, usize),
    (rx, ry): (usize, usize),
) -> Orientation {
    match ((qy as i32 - py as i32) * (rx as i32 - qx as i32))
        - ((qx as i32 - px as i32) * (ry as i32 - qy as i32))
    {
        //unsafe {
        //match ((qy as i16).unchecked_sub(py as i16) * (rx as i16).unchecked_sub(qx as i16)) - ((qx as i16).unchecked_sub(px as i16) * (ry as i16).unchecked_sub(qy as i16)) {
        0 => Orientation::Collinear,
        i => {
            if i > 0 {
                Orientation::Clockwise
            } else {
                Orientation::Counterclockwise
            }
        } //}
    }
}

/// My gift wrapping implementation
pub struct PresentWrapper {}
impl PresentWrapper {
    // IDEA: I can take advantage of triangles for the early termination feature.
    // After drawing two lines, I can find the hypotenuse using Pythag theorem.
    // Then check that the angles are ok.
    // Also makes it fairly trivial to guess the last point and what would be acceptable edges.
    // 3 is a magic number.

    pub fn find_convex_hull(points: &[(usize, usize)]) -> Vec<(usize, usize)> {
        let mut hull = Vec::new();

        // Find leftmost point
        let mut l = 0;

        //for (i, (x, _)) in points.into_iter().enumerate() {
        //    if *x < points[l].0 {
        //        l = i;
        //    }
        //}
        for i in 0..points.len() {
            if points[i].0 < points[l].0 {
                l = i;
            }
        }

        let mut p = l;
        let mut q: usize;

        while p != l || hull.is_empty() {
            hull.push(points[p]);

            q = (p + 1) % points.len();

            //for (i, point) in points.into_iter().enumerate() {
            //    if orientation(points[p], *point, points[q]) == Orientation::Counterclockwise {
            //        q = i;
            //    }
            //}
            for i in 0..points.len() {
                if orientation(points[p], points[i], points[q]) == Orientation::Counterclockwise {
                    q = i;
                }
            }

            p = q;
        }

        hull
    }
}
