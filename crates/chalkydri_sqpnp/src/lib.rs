#![feature(const_heap)]

use nalgebra::{Isometry3, Matrix3x4, Point3, Rotation3, SMatrix, SVector, SimdRealField};
use std::{f64::consts::PI, ops::AddAssign};

// --- Type Definitions ---
pub type Mat3 = SMatrix<f64, 3, 3>;
pub type Mat9 = SMatrix<f64, 9, 9>;
pub type Vec9 = SVector<f64, 9>;
pub type Mat15 = SMatrix<f64, 15, 15>;
pub type Vec15 = SVector<f64, 15>;
pub type Mat6x9 = SMatrix<f64, 6, 9>;
pub type Vec6 = SVector<f64, 6>;
pub type Vec3 = SVector<f64, 3>;
pub type Mat9x3 = SMatrix<f64, 9, 3>;
pub type Iso3 = Isometry3<f64>;
pub type Pnt3 = Point3<f64>;
pub type Rot3 = Rotation3<f64>;

// Increase these to trust vision LESS. Decrease to trust vision MORE.
const XY_STD_DEV_SCALAR: f64 = 5.0;
const THETA_STD_DEV_SCALAR: f64 = 2.0;
const MAX_TRUSTABLE_RMS: f64 = 0.1;

// At what degree difference do we FULLY pivot the pose to match the gyro?
// With a gradient, you usually want this slightly higher than a hard cutoff.
const MAX_GYRO_DELTA: f64 = 30.0;

// 2026 tag size in meters
const TAG_SIZE: f64 = 0.1651;
const CORNER_DISTANCE: f64 = TAG_SIZE / 2.0;

#[inline(always)]
fn nearest_so3(r_vec: &Vec9) -> Option<Vec9> {
    let m = Mat3::from_column_slice(r_vec.as_slice());
    // SVD to orthogonalize the matrix (make it a true rotation)
    let svd = m.svd(true, true);
    if let Some(u) = svd.u {
        if let Some(vt) = svd.v_t {
            let mut rot = u * vt;
            // Fix chirality (ensure determinant is +1, not -1)
            if rot.determinant() < 0.0 {
                let mut u_prime = u;
                u_prime.column_mut(2).scale_mut(-1.0);
                rot = u_prime * vt;
            }

            return Some(Vec9::from_column_slice(rot.as_slice()));
        }
    }

    None
}

#[inline(always)]
fn constraints_and_jacobian(r_vec: &Vec9) -> (Vec6, Mat6x9) {
    let c1 = r_vec.fixed_view::<3, 1>(0, 0);
    let c2 = r_vec.fixed_view::<3, 1>(3, 0);
    let c3 = r_vec.fixed_view::<3, 1>(6, 0);

    // Orthogonality and Normality constraints for SO(3)
    let h = Vec6::new(
        c1.norm_squared() - 1.0,
        c2.norm_squared() - 1.0,
        c3.norm_squared() - 1.0,
        c1.dot(&c2),
        c1.dot(&c3),
        c2.dot(&c3),
    );

    let mut jac = Mat6x9::zeros();

    // Gradient of constraints
    jac.fixed_view_mut::<1, 3>(0, 0)
        .copy_from(&(2.0 * c1.transpose()));
    jac.fixed_view_mut::<1, 3>(1, 3)
        .copy_from(&(2.0 * c2.transpose()));
    jac.fixed_view_mut::<1, 3>(2, 6)
        .copy_from(&(2.0 * c3.transpose()));

    jac.fixed_view_mut::<1, 3>(3, 0).copy_from(&c2.transpose());
    jac.fixed_view_mut::<1, 3>(3, 3).copy_from(&c1.transpose());
    jac.fixed_view_mut::<1, 3>(4, 0).copy_from(&c3.transpose());
    jac.fixed_view_mut::<1, 3>(4, 6).copy_from(&c1.transpose());
    jac.fixed_view_mut::<1, 3>(5, 3).copy_from(&c3.transpose());
    jac.fixed_view_mut::<1, 3>(5, 6).copy_from(&c2.transpose());

    (h, jac)
}

#[inline(always)]
fn solve_newton(r: &Vec9, omega: &Mat9, h: &Vec6, jac: &Mat6x9) -> Option<Vec9> {
    // SQP (Sequential Quadratic Programming) step using KKT system
    let mut lhs = Mat15::zeros();
    lhs.fixed_view_mut::<9, 9>(0, 0).copy_from(omega);
    lhs.fixed_view_mut::<9, 6>(0, 9).copy_from(&jac.transpose());
    lhs.fixed_view_mut::<6, 9>(9, 0).copy_from(jac);

    let mut rhs = Vec15::zeros();
    let omega_r = omega * r;

    rhs.fixed_view_mut::<9, 1>(0, 0).copy_from(&(-omega_r));
    rhs.fixed_view_mut::<6, 1>(9, 0).copy_from(&(-h));

    match lhs.lu().solve(&rhs) {
        Some(sol) => Some(sol.fixed_view::<9, 1>(0, 0).into_owned()),
        None => None,
    }
}

struct LinearSys {
    omega: Mat9,
    q_tt_inv: Mat3,
    q_rt: Mat9x3,
}

#[inline(always)]
fn build_linear_system(points_3d: &[Vec3], points_2d: &[Vec3]) -> LinearSys {
    let n = points_3d.len();
    assert_eq!(n, points_2d.len());

    let mut q_rr = Mat9::zeros();
    let mut q_rt = Mat9x3::zeros();
    let mut q_tt = Mat3::zeros();

    for (p_3d, p_img) in points_3d.iter().zip(points_2d.iter()) {
        // Build Projection Matrix P = I - (v*v^T)/(v^T*v)
        let sq_norm = p_img.norm_squared();
        let inv_norm = 1.0 / sq_norm;
        let v_vt = p_img * p_img.transpose();
        let p = Mat3::identity() - v_vt.scale(inv_norm);

        q_tt += p;

        let px = p.scale(p_3d.x);
        let py = p.scale(p_3d.y);
        let pz = p.scale(p_3d.z);

        // Accumulate Q_rt
        q_rt.fixed_view_mut::<3, 3>(0, 0).add_assign(&px);
        q_rt.fixed_view_mut::<3, 3>(3, 0).add_assign(&py);
        q_rt.fixed_view_mut::<3, 3>(6, 0).add_assign(&pz);

        // Accumulate Q_rr
        q_rr.fixed_view_mut::<3, 3>(0, 0)
            .add_assign(&px.scale(p_3d.x));
        q_rr.fixed_view_mut::<3, 3>(3, 3)
            .add_assign(&py.scale(p_3d.y));
        q_rr.fixed_view_mut::<3, 3>(6, 6)
            .add_assign(&pz.scale(p_3d.z));

        let pxy = px.scale(p_3d.y);
        q_rr.fixed_view_mut::<3, 3>(0, 3).add_assign(&pxy);
        q_rr.fixed_view_mut::<3, 3>(3, 0).add_assign(&pxy);

        let pxz = px.scale(p_3d.z);
        q_rr.fixed_view_mut::<3, 3>(0, 6).add_assign(&pxz);
        q_rr.fixed_view_mut::<3, 3>(6, 0).add_assign(&pxz);

        let pyz = py.scale(p_3d.z);
        q_rr.fixed_view_mut::<3, 3>(3, 6).add_assign(&pyz);
        q_rr.fixed_view_mut::<3, 3>(6, 3).add_assign(&pyz);
    }

    let q_tt_inv = q_tt.try_inverse().unwrap_or_default();
    let temp = q_rt * q_tt_inv;
    let omega = q_rr - temp * q_rt.transpose();

    LinearSys {
        omega,
        q_tt_inv,
        q_rt,
    }
}

#[derive(Clone, Debug)]
pub struct SqPnP {
    max_iter: usize,
    tol_sq: f64,
    buffer: Vec<Vec3>,
    candidates: Vec<(Vec9, f64)>,
    gyro_cos: f64,
    gyro_sin: f64,
    sign_change_error: f64,
}

impl Default for SqPnP {
    fn default() -> Self {
        Self::new()
    }
}

impl SqPnP {
    pub const fn new() -> Self {
        Self {
            max_iter: 15,
            tol_sq: 1e-16,
            buffer: Vec::with_capacity(32),
            candidates: Vec::with_capacity(6),
            gyro_cos: 0.0,
            gyro_sin: 0.0,
            sign_change_error: 0.0,
        }
    }
    pub const fn max_iter(mut self, max_iter: usize) -> Self {
        self.max_iter = max_iter;
        self
    }
    pub const fn tolerance(mut self, tol: f64) -> Self {
        self.tol_sq = tol * tol;
        self
    }

    /// Computes WPILib-compatible standard deviations (x, y, theta) from pure geometry.
    /// This never returns None, so it won't interrupt your existing working code.
    fn compute_std_devs(&self, pure_geometric_energy: f64, distance: f64, n_tags: usize) -> Vec3 {
        let n_points = (n_tags * 4) as f64;

        let rms_error = (pure_geometric_energy / n_points).sqrt();

        // 1. Rejection threshold
        if rms_error > MAX_TRUSTABLE_RMS {
            return Vec3::new(f64::MAX, f64::MAX, f64::MAX);
        }

        let distance_multiplier = 1.0 + (distance / TAG_SIZE);

        // 2. Apply XY error Scalar
        let base_xy_std = rms_error * distance_multiplier;
        let xy_std = (base_xy_std / (n_tags as f64).sqrt()) * XY_STD_DEV_SCALAR;
        let xy_std = xy_std.clamp(0.01, 10.0);

        // 3. Apply Theta error Scalar
        let theta_std = {
            let base_theta_std = rms_error / TAG_SIZE;
            let val = (base_theta_std * distance_multiplier / (n_tags as f64).sqrt())
                * THETA_STD_DEV_SCALAR;
            val.clamp(0.05, PI)
        };

        Vec3::new(xy_std, xy_std, theta_std)
    }

    /// Solves for the standard Computer Vision pose (World -> Camera).
    /// Returns (Rotation, Translation, Pure Geometric Energy)
    fn solve(
        &mut self,
        points_isometry: &[Isometry3<f64>],
        points_2d: &[Vec3],
    ) -> Option<(Rot3, Vec3, f64)> {
        self.corner_points_from_center(points_isometry);

        if self.buffer.len() < 3 || self.buffer.len() != points_2d.len() {
            return None;
        }

        let centroid: Vec3 =
            self.buffer.iter().fold(Vec3::zeros(), |acc, p| acc + p) / self.buffer.len() as f64;
        let points_3d_local: Vec<Vec3> = self.buffer.iter().map(|p| p - centroid).collect();

        let sys = build_linear_system(&points_3d_local, points_2d);

        self.solve_rotation_candidates(&sys.omega);

        let mut best_result: Option<(Rot3, Vec3, f64)> = None;
        let mut best_score = f64::MAX;

        for (r_vec, penalized_energy) in &self.candidates {
            let r_mat = Mat3::from_column_slice(r_vec.as_slice());
            let t_local = -(sys.q_tt_inv * sys.q_rt.tr_mul(&r_vec));
            let t = t_local - r_mat * centroid;

            let all_in_front = self.buffer.iter().all(|p| {
                let p_cam = r_mat * p + t;
                p_cam.z > 0.0
            });

            if !all_in_front {
                continue;
            }

            if *penalized_energy < best_score {
                best_score = *penalized_energy;

                // Keep the pure geometric energy for std dev calculations
                let pure_geometric_energy = r_vec.dot(&(sys.omega * r_vec));

                let rot = Rot3::from_matrix(&r_mat);
                best_result = Some((rot, t, pure_geometric_energy));
            }
        }

        best_result
    }

    /// Solves for the Robot's Position in the World (Field Frame).
    /// Returns (Robot Rotation, Robot Position, WPILib Std Devs)
    pub fn solve_robot_pose(
        &mut self,
        points_isometry: &[Isometry3<f64>],
        points_2d: &[Vec3],
        gyro: f64,
        sign_change_error: f64,
    ) -> Option<(Rot3, Vec3, Vec3)> {
        self.gyro_cos = gyro.cos();
        self.gyro_sin = gyro.sin();
        self.sign_change_error = sign_change_error;

        let (rot_world_to_cam, trans_world_to_cam, pure_energy) =
            self.solve(points_isometry, points_2d)?;

        let distance = trans_world_to_cam.norm();
        let n_tags = points_isometry.len();

        let std_devs = self.compute_std_devs(pure_energy, distance, n_tags);

        let cam_pos_world = rot_world_to_cam.inverse() * (-trans_world_to_cam);

        let rot_world_to_cam_mat = rot_world_to_cam.matrix();
        let cam_x_in_world = rot_world_to_cam_mat.row(0).transpose(); // Right
        let cam_y_in_world = rot_world_to_cam_mat.row(1).transpose(); // Down
        let cam_z_in_world = rot_world_to_cam_mat.row(2).transpose(); // Forward

        let robot_x = cam_z_in_world;
        let robot_y = -cam_x_in_world;
        let robot_z = -cam_y_in_world;

        let robot_rot_mat = Mat3::from_columns(&[robot_x, robot_y, robot_z]);
        let robot_rot = Rotation3::from_matrix(&robot_rot_mat);

        // ==========================================================
        // === GRADIENT PIVOT IN WORLD-SPACE AROUND THE TAG(S)    ===
        // ==========================================================

        // 1. Find the centroid (center point) of the tags we are looking at
        let tag_centroid = points_isometry
            .iter()
            .fold(Vec3::zeros(), |acc, iso| acc + iso.translation.vector)
            / n_tags as f64;

        // 2. Get the Vision's calculated Yaw
        // (0, 0)
        let vision_fwd_x = robot_rot_mat[0];
        // (1, 0)
        let vision_fwd_y = robot_rot_mat[1];
        let vision_yaw = vision_fwd_y.simd_atan2(vision_fwd_x);

        // 3. Find the normalized difference between the Gyro and the Vision Yaw
        //    (Normalization prevents full 360° wraps from causing false massive errors)
        let mut delta_yaw = (gyro - vision_yaw) % (2.0 * PI);
        if delta_yaw > PI {
            delta_yaw -= 2.0 * PI;
        }
        if delta_yaw < -PI {
            delta_yaw += 2.0 * PI;
        }

        let delta_deg = delta_yaw.abs().to_degrees();

        // 4. Calculate Gradient Weight (0.0 = 100% Vision, 1.0 = 100% Gyro)
        let mut weight = (delta_deg / MAX_GYRO_DELTA).clamp(0.0, 1.0);

        // Smoothstep function makes the transition an "S" curve, avoiding sudden jumps.
        // Tiny errors barely pivot. Large errors ramp up into a full pivot.
        weight = weight * weight * (3.0 - 2.0 * weight);

        // Calculate the actual angle we are going to pivot by
        let applied_delta_yaw = delta_yaw * weight;

        // 5. Create a Z-axis rotation matrix for this blended difference
        let cos_dt = applied_delta_yaw.cos();
        let sin_dt = applied_delta_yaw.sin();
        #[rustfmt::skip]
        let rot_z = Mat3::new(
            cos_dt, -sin_dt, 0.0,
            sin_dt,  cos_dt, 0.0,
               0.0,     0.0, 1.0,
        );
        let rot_z_rot3 = Rotation3::from_matrix(&rot_z);

        // 6. Pivot the camera's position around the tag centroid
        let pos_relative_to_tag = cam_pos_world - tag_centroid;
        let pivoted_cam_pos = tag_centroid + (rot_z * pos_relative_to_tag);

        // 7. Rotate the robot's heading by the gradient amount
        let pivoted_robot_rot = rot_z_rot3 * robot_rot;

        // Return the smoothly pivoted pose!
        Some((pivoted_robot_rot, pivoted_cam_pos, std_devs))
    }

    fn corner_points_from_center(&mut self, isometry: &[Iso3]) -> () {
        const S: f64 = CORNER_DISTANCE;

        #[rustfmt::skip]
        const CORNER_POINTS_MAT: [Vec3; 4] = [
            Vec3::new(0.0, -S, -S),
            Vec3::new(0.0,  S, -S),
            Vec3::new(0.0,  S,  S),
            Vec3::new(0.0, -S,  S),
        ];

        self.buffer.clear();
        isometry.iter().for_each(|iso: &Iso3| {
            self.buffer
                .extend(CORNER_POINTS_MAT.iter().map(|c| iso * c));
        });
    }

    fn solve_rotation_candidates(&mut self, omega: &Mat9) {
        self.candidates.clear();
        let eigen = omega.symmetric_eigen();

        let mut indices = [0usize, 1, 2, 3, 4, 5, 6, 7, 8];
        indices.sort_by(|&a, &b| eigen.eigenvalues[a].total_cmp(&eigen.eigenvalues[b]));

        for &i in indices.iter().take(3) {
            let e = eigen.eigenvectors.column(i);
            for sign in [-1.0, 1.0] {
                let guess = e.scale(sign);
                if let Some(r_start) = nearest_so3(&guess) {
                    let (refined_r, mut energy) = self.optimization(r_start, omega);

                    // (2, 0)
                    let robot_fwd_x = refined_r[2];
                    // (2, 1)
                    let robot_fwd_y = refined_r[5];
                    let dot = (robot_fwd_x * self.gyro_cos) + (robot_fwd_y * self.gyro_sin);
                    let angle_error = (1.0 - dot).max(0.0);

                    // Add penalty to influence sorting ONLY
                    energy += self.sign_change_error * angle_error;

                    self.candidates.push((refined_r, energy));
                }
            }
        }

        self.candidates.sort_by(|a, b| a.1.total_cmp(&b.1));
    }

    fn optimization(&self, start_r: Vec9, omega: &Mat9) -> (Vec9, f64) {
        let mut r = start_r;
        for _ in 0..self.max_iter {
            let (h, jac) = constraints_and_jacobian(&r);
            match solve_newton(&r, omega, &h, &jac) {
                Some(delta_r) => {
                    r += delta_r;
                    if delta_r.norm_squared() < self.tol_sq {
                        break;
                    }
                }
                None => break,
            }
        }
        let energy = r.dot(&(omega * r));
        (r, energy)
    }
}
