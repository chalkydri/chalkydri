use factrs::{
    assign_symbols,
    containers::FactorBuilder,
    core::{Factor, GaussNewton, Graph, SE2, SE3, SO2, SO3, Values},
    linalg::{Matrix1x3, Matrix3},
    traits::*,
};

#[macro_use]
extern crate factrs;

pub struct Slam {}

assign_symbols!(Tag: SE3; Robot: SE3);

fn main() {
    let mut graph = Graph::new();

    let mut values = Values::new();
    values.insert(Robot(0), SE3::identity());

    //SE3::from_rot_trans(SO3::from_xyzw(), Matrix1x3::new(1.0, 1.0, 1.0));
    //`graph.add_factor(FactorBuilder::new3());
    let mut opt: GaussNewton = GaussNewton::new(graph);
    opt.optimize(values).unwrap();
}
