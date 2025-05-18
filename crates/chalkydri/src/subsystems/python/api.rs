use std::borrow::Cow;

use pyo3::{prelude::*, types::PyBytes};

pub(crate) fn module(py: Python) -> PyResult<()> {
    let m = PyModule::new(py, "chalkydri")?;
    m.add_class::<Camera>()?;
    Ok(())
}

#[pyclass]
pub(crate) struct Camera {}
#[pymethods]
impl Camera {
    fn get_frame(self_: PyRef<'_, Self>) -> PyResult<Cow<[u8]>> {
        Ok(Cow::Borrowed(&[0u8]))
    }
}
