#![no_std]
#![cfg(any(target_arch = "x86", target_arch = "x86_64"))]

mod util;

pub mod local;
pub mod remote;
