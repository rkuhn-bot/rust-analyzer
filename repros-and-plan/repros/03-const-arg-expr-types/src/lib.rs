//! Const-argument expressions that rust-analyzer leaves as `{unknown}` in the
//! body expression type map, even when the type that contains them is right.
//!
//! This is the bulk of the `{unknown}` rows in `typestate/describe.rs` (`"Send"`,
//! `", "`, `" | "`), `typestate/effect.rs` (`[foo::Bar; 4]`), and
//! `typestate/tests.rs` (`Remainder<{ TEXT }>`). It is also the `[T; _]` row in
//! the older pure-stage checkout.
//!
//! `&'static str` const parameters and associated-const arguments already lower
//! to real const values. `unsized_const_params` / `adt_const_params` are not the
//! cause of these rows.

#![feature(unsized_const_params, adt_const_params)]
#![allow(incomplete_features)]

pub struct IntParam<const N: usize>;
pub struct StrParam<const S: &'static str>;

pub fn str_in_body() {
    let _x: StrParam<"Send"> = StrParam;
}

pub fn int_in_body() {
    let _x: IntParam<1> = IntParam;
}

pub trait Desc {
    const TEXT: &'static str;
}
impl Desc for u8 {
    const TEXT: &'static str = "u8";
}

pub fn assoc_in_body() {
    let _x: StrParam<{ <u8 as Desc>::TEXT }> = StrParam;
}

pub fn local_const_arg() {
    const TEXT: &'static str = "hi";
    let _x: StrParam<{ TEXT }> = StrParam;
}

pub fn array_underscore() -> usize {
    let ops: [u8; _] = [1, 2];
    ops.len()
}

pub const fn type_last_segment<T>() -> &'static str {
    "x"
}

pub fn array_in_turbofish() -> &'static str {
    type_last_segment::<[u8; 4]>()
}
