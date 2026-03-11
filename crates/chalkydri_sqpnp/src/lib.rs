#![feature(const_heap)]

use nalgebra::{
    Isometry3, Matrix3x4, Point3, Rotation3, SMatrix, SVector, SimdRealField, Translation3, UnitQuaternion
};
use uom::si::{f64::Length, length::{centimeter, meter}};
use std::{f64::consts::PI, ops::AddAssign, sync::LazyLock};

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
static TAG_SIZE: LazyLock<Length> = LazyLock::new(|| {
    Length::new::<centimeter>(16.51)
});
static CORNER_DISTANCE: LazyLock<Length> = LazyLock::new(|| {
    *TAG_SIZE / 2.0
});

#[rustfmt::skip]
static CORNER_POINTS_MAT: LazyLock<[Pnt3; 4]> = LazyLock::new(|| {
    let s: f64 = (*CORNER_DISTANCE).get::<meter>();

    [
        Pnt3::new(0.0, -s, -s),
        Pnt3::new(0.0,  s, -s),
        Pnt3::new(0.0,  s,  s),
        Pnt3::new(0.0, -s,  s),
    ]
});

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
    fwd_in_cam: Vec3,
}

impl Default for SqPnP {
    fn default() -> Self {
        Self::new()
    }
}

impl SqPnP {
    pub fn new() -> Self {
        Self {
            max_iter: 15,
            tol_sq: 1e-16,
            buffer: Vec::with_capacity(32),
            candidates: Vec::with_capacity(6),
            gyro_cos: 0.0,
            gyro_sin: 0.0,
            sign_change_error: 0.0,
            fwd_in_cam: Vec3::new(0.0, 0.0, 1.0),
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

    fn compute_std_devs(&self, pure_geometric_energy: f64, distance: Length, n_tags: usize) -> Vec3 {
        let n_points = (n_tags * 4) as f64;
        let rms_error = (pure_geometric_energy / n_points).sqrt();

        if rms_error > MAX_TRUSTABLE_RMS {
            return Vec3::new(f64::MAX, f64::MAX, f64::MAX);
        }

        let d_ratio = distance.get::<meter>() / TAG_SIZE.get::<meter>();
        let distance_multiplier = 1.0 + (d_ratio * d_ratio);

        let tag_penalty = if n_tags <= 2 { 3.0 } else { 1.0 };

        let base_xy_std = rms_error * distance_multiplier * tag_penalty;
        let xy_std = (base_xy_std / (n_tags as f64).sqrt()) * XY_STD_DEV_SCALAR;
        let xy_std = xy_std.clamp(0.01, f64::MAX);

        let theta_std = {
            let base_theta_std = (rms_error * distance_multiplier * tag_penalty) / TAG_SIZE.get::<meter>();
            let val = (base_theta_std / (n_tags as f64).sqrt()) * THETA_STD_DEV_SCALAR;
            val.clamp(0.01, f64::MAX)
        };

        Vec3::new(xy_std, xy_std, theta_std)
    }

    fn solve(
        &mut self,
        points_isometry: &[Isometry3<f64>],
        points_2d: &[Vec3],
    ) -> Option<Vec<(Rot3, Vec3, f64)>> {
        self.corner_points_from_center(points_isometry);

        if self.buffer.len() < 3 || self.buffer.len() != points_2d.len() {
            return None;
        }

        let centroid: Vec3 =
            self.buffer.iter().fold(Vec3::zeros(), |acc, p| acc + p) / self.buffer.len() as f64;
        let points_3d_local: Vec<Vec3> = self.buffer.iter().map(|p| p - centroid).collect();

        let sys = build_linear_system(&points_3d_local, points_2d);

        self.solve_rotation_candidates(&sys.omega);

        let mut results: Vec<(Rot3, Vec3, f64)> = Vec::new();

        for (r_vec, penalized_energy) in &self.candidates {
            let r_mat = Mat3::from_column_slice(r_vec.as_slice());
            let t_local = -(sys.q_tt_inv * sys.q_rt.tr_mul(&r_vec));
            let t = t_local - r_mat * centroid;

            let all_in_front = self.buffer.iter().all(|p| {
                let p_cam = r_mat * p + t;
                p_cam.z > 0.0
            });

            if all_in_front {
                let rot = Rot3::from_matrix(&r_mat);
                results.push((rot, t, *penalized_energy));
            }
        }

        Some(results)
    }

    pub fn solve_robot_pose(
        &mut self,
        points_isometry: &[Isometry3<f64>],
        points_2d: &[Vec3],
        robot_to_cam: &Isometry3<f64>,
        gyro: f64,
        sign_change_error: f64,
    ) -> Option<(Rot3, Vec3, Vec3)> {
        self.gyro_cos = gyro.cos();
        self.gyro_sin = gyro.sin();

        let candidates = self.solve(points_isometry, points_2d)?;
        let n_tags = points_isometry.len();

        let mut best_pose = None;
        let mut best_score = f64::MAX;
        let mut best_std_devs = Vec3::zeros();

        let expected_fwd = Vec3::new(self.gyro_cos, self.gyro_sin, 0.0);

        for (world_to_cam_rot, world_to_cam_trans, penalized_energy) in candidates {
            let world_to_cam_iso =
                Iso3::from_parts(world_to_cam_trans.into(), world_to_cam_rot.into());
            let world_to_cam_iso_inv = world_to_cam_iso.inverse();
            let world_to_cam_inv_rot_mat = world_to_cam_iso_inv
                .rotation
                .to_rotation_matrix()
                .matrix()
                .clone();

            let world_to_cam_iso_nwu = Iso3::from_parts(
                world_to_cam_iso_inv.translation,
                UnitQuaternion::from_rotation_matrix(&Rot3::from_matrix(&Mat3::from_columns(&[
                    world_to_cam_inv_rot_mat.column(2).into_owned(),
                    -world_to_cam_inv_rot_mat.column(0).into_owned(),
                    -world_to_cam_inv_rot_mat.column(1).into_owned(),
                ]))),
            );

            let world_to_robot_iso = world_to_cam_iso_nwu * robot_to_cam.inverse();
            let world_to_robot_rot = world_to_robot_iso.rotation.to_rotation_matrix();
            let world_to_robot_trans = world_to_robot_iso.translation.vector;

            let est_fwd = world_to_robot_rot * Vec3::new(1.0, 0.0, 0.0);
            let fwd_dot = est_fwd.dot(&expected_fwd);

            if sign_change_error > 0.0 && fwd_dot < 0.707 {
                continue;
            }

            let yaw_error_metric = (1.0 - fwd_dot).max(0.0);
            let penalized_score = penalized_energy + sign_change_error * yaw_error_metric * 1000.0;

            if penalized_score < best_score {
                best_score = penalized_score;
                best_pose = Some((world_to_robot_rot, world_to_robot_trans));
                let distance = world_to_cam_trans.norm();
                best_std_devs = self.compute_std_devs(penalized_energy, Length::new::<meter>(distance), n_tags);
            }
        }

        if let Some((rot, trans)) = best_pose {
            Some((rot, trans, best_std_devs))
        } else {
            None
        }
    }

    pub fn create_solver_camera_transform(
        fwd_m: f64,
        left_m: f64,
        up_m: f64,
        roll_deg: f64,
        pitch_deg: f64,
        yaw_deg: f64,
    ) -> Iso3 {
        let nwu_translation = Translation3::new(fwd_m, left_m, up_m);
        
        let nwu_rotation = UnitQuaternion::from_euler_angles(
            roll_deg.to_radians(),
            pitch_deg.to_radians(),
            yaw_deg.to_radians(),
        );
        
        let robot_pose_of_cam_nwu = Isometry3::from_parts(nwu_translation, nwu_rotation);

        let nwu_to_cv_rot = Rotation3::from_matrix_unchecked(Mat3::new(
            0.0,  0.0,  1.0,
            -1.0,  0.0,  0.0,
            0.0, -1.0,  0.0,
        ));
        
        let nwu_to_cv = Isometry3::from_parts(
            Translation3::identity(), 
            UnitQuaternion::from_rotation_matrix(&nwu_to_cv_rot)
        );

        (robot_pose_of_cam_nwu * nwu_to_cv).inverse()
    }


    fn corner_points_from_center(&mut self, isometry: &[Iso3]) -> () {
        isometry.iter().for_each(|iso: &Iso3| {
            self.buffer
                .extend(CORNER_POINTS_MAT.iter().map(|c| (iso * c).coords));
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

                    let d = &self.fwd_in_cam;

                    let robot_fwd_x =
                        refined_r[0] * d[0] + refined_r[1] * d[1] + refined_r[2] * d[2];
                    let robot_fwd_y =
                        refined_r[3] * d[0] + refined_r[4] * d[1] + refined_r[5] * d[2];

                    let dot = (robot_fwd_x * self.gyro_cos) + (robot_fwd_y * self.gyro_sin);
                    let angle_error = (1.0 - dot).max(0.0);

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
