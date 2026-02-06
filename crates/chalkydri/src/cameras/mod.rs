/***
 * THIS FILE IS CURSED
 * PLES SEND HELP
 */

pub(crate) mod gst_to_cu;
//pub(crate) mod mjpeg;
pub(crate) mod pipeline;
pub(crate) mod providers;
//mod format_selection;

pub use gst_to_cu::GstToCuImage;
use gstreamer::{
    Bin, Bus, BusSyncReply, Caps, Device, DeviceProvider, DeviceProviderFactory, Element,
    ElementFactory, FlowError, FlowSuccess, Fraction, Message, MessageView, PadDirection, Pipeline,
    State, Structure, glib::WeakRef, prelude::*,
};

use gstreamer_app::{AppSink, AppSinkCallbacks};
use pipeline::CamPipeline;
use providers::{CamProvider, ProviderEvent, V4l2Provider};
#[cfg(feature = "rerun")]
use re_types::archetypes::EncodedImage;
use std::{collections::HashMap, mem::ManuallyDrop, sync::Arc};
use tokio::{
    sync::{Mutex, MutexGuard, RwLock, mpsc, watch},
    task::JoinHandle,
};
use tracing::Level;

#[cfg(feature = "rerun")]
use crate::Rerun;
use chalkydri_core::prelude::*;

#[derive(Clone)]
pub struct CameraCtx {
    cfgg: config::Camera,
    tee: WeakRef<Element>,
}
