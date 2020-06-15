#![no_std]
#![cfg(any(target_arch = "x86", target_arch = "x86_64"))]

#[cfg(unix)]
mod os {
    mod unix;

    pub use unix::*;
}

#[cfg(windows)]
mod os {
    mod windows;

    pub use windows::*;
}

mod util;

pub mod raw;
pub mod remote;
