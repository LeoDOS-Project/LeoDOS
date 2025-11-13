/*!
Low-level FFI bindings for libcsp (Cubesat Space Protocol).

This module contains the raw, `unsafe` function and type definitions generated
by `rust-bindgen`. It is not intended for direct use by applications. Instead,
safe wrappers should be used once implemented.
*/

#![allow(clippy::all)]
#![allow(non_upper_case_globals)]
#![allow(non_camel_case_types)]
#![allow(non_snake_case)]
#![allow(dead_code)]
#![allow(overflowing_literals)]
#![allow(missing_docs)]

include!(concat!(env!("OUT_DIR"), "/bindings.rs"));

pub(crate) type csp_queue_handle_t = *mut libc::c_void;
pub(crate) type csp_static_queue_t = [u8; 128];
