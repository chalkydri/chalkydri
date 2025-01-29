mod api;

use crate::Subsystem;

use pyo3::prelude::*;

pub struct PythonSubsys {}
impl<'subsys> Subsystem<'subsys> for PythonSubsys {
    fn init() -> Result<Box<Self>, Box<dyn std::error::Error>> {
        Ok(Box::new(Self {}))
    }
    fn run(&self, rt: tokio::runtime::Runtime) {
        rt.spawn(async {
            Python::with_gil(|py| -> PyResult<()> {
                api::module(py)?;
                let m = PyModule::from_code(py, "code", "file_name", "module_name").unwrap();
                Ok(())
            })
            .unwrap();
        });
    }
}
