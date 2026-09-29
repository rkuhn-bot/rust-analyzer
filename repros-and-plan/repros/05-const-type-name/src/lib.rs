//! Control: `const_type_name` already works for a monomorphic const.
//!
//! rustc and rust-analyzer both accept this. `analysis-stats` reports no
//! `{unknown}` on the call, and const-eval of `NAME` succeeds.
//!
//! The `type_name` uses inside `amaru-pure-stage` fail only when they sit in
//! a generic const item whose parameters are not in scope (repro 02).

#![feature(const_type_name)]

pub const fn type_last_segment<T>() -> &'static str {
    core::any::type_name::<T>()
}

pub const NAME: &'static str = type_last_segment::<u8>();

pub fn read() -> &'static str {
    NAME
}
