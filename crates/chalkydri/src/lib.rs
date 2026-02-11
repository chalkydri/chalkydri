//!
//! # Chalkydri
//!
//! This crate contains Chalkydri itself.
//!
//! This code runs on the vision coprocessor(s) and does all the heavy lifting.
//!

#![feature(coroutines, coroutine_trait)]
#![allow(unreachable_code)]
#![deny(
    unused_must_use,
    clippy::infinite_iter,
    clippy::infinite_loop,
    clippy::unconditional_recursion,
    clippy::while_immutable_condition
)]

// These deps are needed no matter what
extern crate tracing;
#[macro_use]
extern crate serde;

pub mod cameras;
pub mod subsystems;
pub mod utils;

use chalkydri_core::prelude::config;
use mimalloc::MiMalloc;

// mimalloc is an excellent general purpose allocator
#[global_allocator]
static GLOBAL: MiMalloc = MiMalloc;
