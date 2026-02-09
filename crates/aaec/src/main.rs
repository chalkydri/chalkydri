use nalgebra::Matrix3;
use nalgebra::matrix;

const SOBEL_KERNEL_DX: Matrix3<f64> = matrix![
    -0.125, 0.0, 0.125;
    -0.25, 0.0, 0.25;
    -0.125, 0.0, 0.125;
];
const SOBEL_KERNEL_DY: Matrix3<f64> = matrix![
    -0.125, -0.25, -0.125;
    0.0, 0.0, 0.0;
    0.125, 0.25, 0.125;
];

fn find_gradients() {}

fn main() {}
