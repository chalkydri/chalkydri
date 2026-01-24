use std::{fmt::Debug, marker::PhantomData, sync::Arc};

use nt_client::ClientHandle as NTClientHandle;
use tokio::sync::watch;

use crate::{cameras::preproc::Preprocessor, config};

#[cfg(feature = "apriltags")]
pub mod apriltags;
pub mod calibration;
#[cfg(feature = "capriltags")]
pub mod capriltags;
mod manager;
#[cfg(feature = "python")]
pub use chalkydri_subsys_python as python;

pub use manager::SubsysManager;
