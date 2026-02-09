use nalgebra::{Isometry3, Point3, Rotation3, SMatrix, SVector};
use std::{ops::AddAssign}; //trust.

/*Usage:
    1. Create a Solver
    2. .solve with 3d points in space and their normalized 2d coordinate vectors on the camera
*/

//These should be all that we need
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

//R is rot matrix, r_vec is column slice of R
//Jacobian is just partial derivatives across a 3d space
//fixed_view_mut is an edit in place
//svd is single value decomposition

#[inline(always)]
fn nearest_so3(r_vec: &Vec9) -> Vec9 {
    //first make r_vec a matrix again
    let m = Mat3::from_column_slice(r_vec.as_slice());

    let svd = m.svd(true, true);
    let u = svd.u.unwrap_or_default();
    let vt = svd.v_t.unwrap_or_default();

    let mut rot = u * vt;
    //fix chirality
    if rot.determinant() < 0.0 {
        let mut u_prime = u;
        u_prime.column_mut(2).scale_mut(-1.0);
        rot = u_prime * vt;
    }
    Vec9::from_column_slice(rot.as_slice())
}

#[inline(always)]
fn constraints_and_jacobian(r_vec: &Vec9) -> (Vec6, Mat6x9) {
    //references to each column of rot matrix
    let c1 = r_vec.fixed_view::<3, 1>(0, 0);
    let c2 = r_vec.fixed_view::<3, 1>(3, 0);
    let c3 = r_vec.fixed_view::<3, 1>(6, 0);

    //residuals
    let h = Vec6::new(
        c1.norm_squared() - 1.0,
        c2.norm_squared() - 1.0,
        c3.norm_squared() - 1.0, //normality contraints
        c1.dot(&c2),
        c1.dot(&c3),
        c2.dot(&c3), //orthogonality constraints
    );

    let mut jac = Mat6x9::zeros();

    //derivatives of normality constraints
    jac.fixed_view_mut::<1, 3>(0, 0)
        .copy_from(&(2.0 * c1.transpose()));
    jac.fixed_view_mut::<1, 3>(1, 3)
        .copy_from(&(2.0 * c2.transpose()));
    jac.fixed_view_mut::<1, 3>(2, 6)
        .copy_from(&(2.0 * c3.transpose()));

    //derivatives of orthogonality
    jac.fixed_view_mut::<1, 3>(3, 0).copy_from(&c2.transpose());
    jac.fixed_view_mut::<1, 3>(3, 3).copy_from(&c1.transpose()); //row 3, c1 . c2
    jac.fixed_view_mut::<1, 3>(4, 0).copy_from(&c3.transpose());
    jac.fixed_view_mut::<1, 3>(4, 6).copy_from(&c1.transpose()); //row 4, c1 . c3
    jac.fixed_view_mut::<1, 3>(5, 3).copy_from(&c3.transpose());
    jac.fixed_view_mut::<1, 3>(5, 6).copy_from(&c2.transpose()); //row 5, c2 . c3

    (h, jac)
}

#[inline(always)]
fn solve_newton(r: &Vec9, omega: &Mat9, h: &Vec6, jac: &Mat6x9) -> Option<Vec9> {
    let mut lhs = Mat15::zeros(); //left hand side (KKT Matrix)
    lhs.fixed_view_mut::<9, 9>(0, 0).copy_from(omega);
    lhs.fixed_view_mut::<9, 6>(0, 9).copy_from(&jac.transpose());
    lhs.fixed_view_mut::<6, 9>(9, 0).copy_from(jac);

    let mut rhs = Vec15::zeros();
    let omega_r = omega * r;

    //gradient descent
    rhs.fixed_view_mut::<9, 1>(0, 0).copy_from(&(-omega_r));
    //constraint correction
    rhs.fixed_view_mut::<6, 1>(9, 0).copy_from(&(-h));

    //lu used bc symmetric indef
    match lhs.lu().solve(&rhs) {
        Some(sol) => {
            let sol: Vec15 = sol;
            Some(sol.fixed_view::<9, 1>(0, 0).into_owned())
        }
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
        //projection matrix P = I - (v * v^T) / (v^T * v)
        let sq_norm = p_img.norm_squared();
        let inv_norm = 1.0 / sq_norm;
        let v_vt = p_img * p_img.transpose();
        let p = Mat3::identity() - v_vt.scale(inv_norm);

        q_tt += p;

        let px = p.scale(p_3d.x);
        let py = p.scale(p_3d.y);
        let pz = p.scale(p_3d.z);

        //No rust, I in fact did not want to assign a value to the result of a function call. Thanks though.
        //q_rt
        q_rt.fixed_view_mut::<3, 3>(0, 0).add_assign(&px);
        q_rt.fixed_view_mut::<3, 3>(3, 0).add_assign(&py);
        q_rt.fixed_view_mut::<3, 3>(6, 0).add_assign(&pz);

        //diagonal
        q_rr.fixed_view_mut::<3, 3>(0, 0)
            .add_assign(&px.scale(p_3d.x));
        q_rr.fixed_view_mut::<3, 3>(3, 3)
            .add_assign(&py.scale(p_3d.y));
        q_rr.fixed_view_mut::<3, 3>(6, 6)
            .add_assign(&pz.scale(p_3d.z));

        //not diagonal
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
            corner_distance: 0.01651f64 / 2.0, //2026 in cm
        }
    }
}

impl SqPnP {
    pub fn new() -> Self {
        Self::default()
    }
    pub const fn max_iter(mut self, max_iter: usize) -> Self {
        //number of iterations in gradient descent
        self.max_iter = max_iter;
        self
    }
    pub const fn tolerance(mut self, tol: f64) -> Self {
        //how close should we get to the end of the gradient descent before calling it
        self.tol_sq = tol * tol;
        self
    }
    pub fn with_tag_side_size(mut self, size: f64) -> Self {
        self.corner_distance = size / 2.0;
        self
    }

    pub fn solve(
        &self,
        points_isometry: &[Isometry3<f64>],
        points_2d: &[Vec3],
        gyro: f64,
        sign_change_error: f64,
        buffer: &mut Vec<Pnt3>,
    ) -> Option<(Rot3, Vec3)> {
        self.corner_points_from_center(points_isometry, buffer);
        let mut points_3d: Vec<Vec3> = Vec::new();
        for point in buffer {
            points_3d.push(Vec3::new(point.x, point.y, point.z));
        }

        if points_3d.len() < 3 || points_3d.len() != points_2d.len() {
            return None;
        }

        let sys = build_linear_system(&points_3d, points_2d);

        let r_mat = self.solve_rotation(&sys.omega, gyro, sign_change_error);

        //t = -q_tt^-1 * q_rt^T * r
        let r_vec = Vec9::from_column_slice(r_mat.as_slice());
        let t_vec = -sys.q_tt_inv * sys.q_rt.transpose() * r_vec;

        let rot = Rot3::from_matrix(&r_mat);

        Some((rot, t_vec))
    }

    fn corner_points_from_center(&self, isometry: &[Iso3], buffer: &mut Vec<Pnt3>) -> () {
        buffer.clear();
        isometry.iter().for_each(|iso: &Iso3| {
            let corners = [
                Pnt3::new(self.corner_distance, self.corner_distance, 0.0),
                Pnt3::new(self.corner_distance, -self.corner_distance, 0.0),
                Pnt3::new(-self.corner_distance, self.corner_distance, 0.0),
                Pnt3::new(-self.corner_distance, -self.corner_distance, 0.0),
            ];
            for c in corners {
                buffer.push(iso * c);
            }
        });
    }

    fn solve_rotation(&self, omega: &Mat9, gyro: f64, sign_change_error: f64) -> Mat3 {
        let eigen = omega.symmetric_eigen();

        let mut best_r = Vec9::zeros();
        let mut min_energy = f64::MAX;

        for i in 0..3 {
            let e = eigen.eigenvectors.column(i);
            for sign in [-1.0, 1.0] {
                let guess = e.scale(sign);
                let r_start = nearest_so3(&guess);
                let (refined_r, mut energy) = self.optimization(r_start, omega);

                let test_r_mat = Mat3::from_column_slice(refined_r.as_slice());

                let dot = (test_r_mat[(0, 0)] * gyro.cos()) + (test_r_mat[(1, 0)] * gyro.sin());

                if dot < 0.0{
                    energy += sign_change_error;
                }

                if energy < min_energy {
                    min_energy = energy;
                    best_r = refined_r;
                }
                if min_energy < 1e-12 {
                    break;
                }
            }
        }
        println!("Solution Entropy: {}", min_energy);
        Mat3::from_column_slice(best_r.as_slice())
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
                None => break, //shouldn't happen like ever
            }
        }
        let energy = r.dot(&(omega * r));
        (r, energy)
    }
}
