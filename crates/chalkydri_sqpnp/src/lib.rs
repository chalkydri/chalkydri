use nalgebra::{Isometry3, Point3, Rotation3, SMatrix, SVector};
use std::{f64, ops::AddAssign};

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

// Standard Computer Vision to Robot (NWU: X-Forward, Y-Left, Z-Up) Rotation Matrix
// Maps: Cam Z(Fwd) -> Rob X(Fwd), Cam X(Right) -> Rob Y(Left), Cam Y(Down) -> Rob Z(Up)
const CAM_TO_ROBOT_ROT: Mat3 = Mat3::new(0.0, 0.0, 1.0, -1.0, 0.0, 0.0, 0.0, -1.0, 0.0);

#[inline(always)]
fn nearest_so3(r_vec: &Vec9) -> Vec9 {
    let m = Mat3::from_column_slice(r_vec.as_slice());
    // SVD to orthogonalize the matrix (make it a true rotation)
    let svd = m.svd(true, true);
    let u = svd.u.unwrap_or_default();
    let vt = svd.v_t.unwrap_or_default();

    let mut rot = u * vt;
    // Fix chirality (ensure determinant is +1, not -1)
    if rot.determinant() < 0.0 {
        let mut u_prime = u;
        u_prime.column_mut(2).scale_mut(-1.0);
        rot = u_prime * vt;
    }
    Vec9::from_column_slice(rot.as_slice())
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

#[derive(Clone, Copy, Debug)]
pub struct SqPnP {
    max_iter: usize,
    tol_sq: f64,
    corner_distance: f64,
}

impl Default for SqPnP {
    fn default() -> Self {
        Self {
            max_iter: 15,
            tol_sq: 1e-16,
            corner_distance: 0.1651f64 / 2.0, //2026 in cm
        }
    }
}

impl SqPnP {
    pub fn new() -> Self {
        Self::default()
    }
    pub const fn max_iter(mut self, max_iter: usize) -> Self {
        self.max_iter = max_iter;
        self
    }
    pub const fn tolerance(mut self, tol: f64) -> Self {
        self.tol_sq = tol * tol;
        self
    }
    pub fn with_tag_side_size(mut self, size: f64) -> Self {
        self.corner_distance = size / 2.0;
        self
    }

    /// Solves for the standard Computer Vision pose (World -> Camera).
    /// Returns (Rotation, Translation) where P_cam = R * P_world + T
    pub fn solve(
    &self,
    points_isometry: &[Isometry3<f64>],
    points_2d: &[Vec3],
    gyro: f64,
    sign_change_error: f64,
    buffer: &mut Vec<Pnt3>,
) -> Option<(Rot3, Vec3)> {
    self.corner_points_from_center(points_isometry, buffer);
    let mut points_3d: Vec<Vec3> = Vec::with_capacity(buffer.len());
    for point in buffer {
        points_3d.push(Vec3::new(point.x, point.y, point.z));
    }

    if points_3d.len() < 3 || points_3d.len() != points_2d.len() {
        return None;
    }

    let n = points_3d.len() as f64;
    let centroid: Vec3 =
        points_3d.iter().copied().fold(Vec3::zeros(), |acc, p| acc + p) / n;
    let points_3d_local: Vec<Vec3> = points_3d.iter().map(|p| p - centroid).collect();

    let sys = build_linear_system(&points_3d_local, points_2d);

    // --- Changed: get ALL candidate rotations instead of just one ---
    let candidates = self.solve_rotation_candidates(&sys.omega, gyro, sign_change_error);

    let mut best_result: Option<(Rot3, Vec3)> = None;
    let mut best_score = f64::MAX;

    for (r_mat, energy) in candidates {
        let r_vec = Vec9::from_column_slice(r_mat.as_slice());
        let t_local = -sys.q_tt_inv * sys.q_rt.transpose() * r_vec;
        // t here is the camera-frame translation: P_cam = R * P_world + t
        let t = t_local - r_mat * centroid;

        // ---- THE KEY FIX: Cheirality check ----
        // Transform every world point into camera coords.
        // If ANY point ends up with z <= 0, this solution has the
        // tag behind the camera — physically impossible, so skip it.
        let all_in_front = points_3d.iter().all(|p| {
            let p_cam = r_mat * p + t;
            p_cam.z > 0.0
        });

        if !all_in_front {
            continue;
        }

        if energy < best_score {
            best_score = energy;
            let rot = Rot3::from_matrix(&r_mat);
            best_result = Some((rot, t));
        }
    }

    best_result
}

    /// Solves for the Robot's Position in the World (Field Frame).
    /// Handles the coordinate system change (CV -> Robot) and the PnP Inversion.
    pub fn solve_robot_pose(
        &self,
        points_isometry: &[Isometry3<f64>],
        points_2d: &[Vec3],
        gyro: f64,
        sign_change_error: f64,
        buffer: &mut Vec<Pnt3>,
    ) -> Option<(Rot3, Vec3)> {
        // 1. Get raw World-to-Camera (CV Frame)
        // Returns T_wc (Translation from World to Cam in Cam coords)
        let (r_wc, t_wc) =
            self.solve(points_isometry, points_2d, gyro, sign_change_error, buffer)?;

        // 2. Invert to get Camera-to-World (CV Frame)
        // Position of Camera in World = -R^T * T
        // NOTE: We apply negation to the vector because Rot3 cannot be negated directly
        let cam_pos_world = r_wc.inverse() * (-t_wc);

        // 3. Compute Robot Rotation
        // R_wc contains the camera axes in World Frame (Rows of R_wc):
        // Row 0 = Cam X (Right) in World
        // Row 1 = Cam Y (Down) in World
        // Row 2 = Cam Z (Forward) in World

        // Robot Frame (NWU): X=Forward, Y=Left, Z=Up
        // Mapping:
        // Robot X (Forward) = Cam Z
        // Robot Y (Left)    = -Cam X
        // Robot Z (Up)      = -Cam Y

        let r_wc_mat = r_wc.matrix();
        let cam_x_in_world = r_wc_mat.row(0).transpose(); // Right
        let cam_y_in_world = r_wc_mat.row(1).transpose(); // Down
        let cam_z_in_world = r_wc_mat.row(2).transpose(); // Forward

        let robot_x = cam_z_in_world;
        let robot_y = -cam_x_in_world;
        let robot_z = -cam_y_in_world;

        let robot_rot_mat = Mat3::from_columns(&[robot_x, robot_y, robot_z]);
        let robot_rot = Rotation3::from_matrix(&robot_rot_mat);

        Some((robot_rot, cam_pos_world))
    }

    fn corner_points_from_center(&self, isometry: &[Iso3], buffer: &mut Vec<Pnt3>) -> () {
        buffer.clear();
        let s = self.corner_distance;
        isometry.iter().for_each(|iso: &Iso3| {
            let corners = [
                Pnt3::new(0.0, -s, -s),
                Pnt3::new(0.0, s, -s),
                Pnt3::new(0.0, s, s),
                Pnt3::new(0.0, -s, s),
            ];

            for c in corners {
                // Apply the tag's field pose (isometry)
                buffer.push(iso * c);
            }
        });
    }

    /// Returns all candidate rotations with their gyro-penalized energies,
/// sorted best-first. Previously `solve_rotation` collapsed this to one.
fn solve_rotation_candidates(
    &self,
    omega: &Mat9,
    gyro: f64,
    sign_change_error: f64,
) -> Vec<(Mat3, f64)> {
    let eigen = omega.symmetric_eigen();

    let mut indices: Vec<usize> = (0..9).collect();
    indices.sort_by(|&a, &b| {
        eigen.eigenvalues[a]
            .partial_cmp(&eigen.eigenvalues[b])
            .unwrap()
    });

    let mut candidates: Vec<(Mat3, f64)> = Vec::with_capacity(6);

    // 3 smallest eigenvectors × 2 signs = 6 candidates
    for &i in indices.iter().take(3) {
        let e = eigen.eigenvectors.column(i);
        for sign in [-1.0, 1.0] {
            let guess = e.scale(sign);
            let r_start = nearest_so3(&guess);
            let (refined_r, mut energy) = self.optimization(r_start, omega);

            let r_mat = Mat3::from_column_slice(refined_r.as_slice());

            // Gyro heading penalty (same as before)
            let robot_fwd_x = r_mat[(2, 0)];
            let robot_fwd_y = r_mat[(2, 1)];
            let dot = (robot_fwd_x * gyro.cos()) + (robot_fwd_y * gyro.sin());
            let angle_error = (1.0 - dot).max(0.0);
            energy += sign_change_error * angle_error;

            candidates.push((r_mat, energy));
        }
    }

    candidates.sort_by(|a, b| a.1.partial_cmp(&b.1).unwrap());
    candidates
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
