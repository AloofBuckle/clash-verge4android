#![allow(non_snake_case)]
#![recursion_limit = "512"]

#[cfg(not(target_os = "android"))]
include!("desktop.rs");

#[cfg(target_os = "android")]
mod android_vpn;
#[cfg(target_os = "android")]
mod mobile;
#[cfg(target_os = "android")]
pub use mobile::run;
