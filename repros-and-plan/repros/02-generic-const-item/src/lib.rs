//! Generic const items (`const NAME<T>: Ty = ...`).
//!
//! The parser accepts the syntax. The const body's type parameters are not in scope
//! for inference, so `A` and `B` are `{unknown}`. There is no hard diagnostic.
//!
//! Mirrors `TYPES_EQ` in `typestate/list.rs` and `REVEAL` in `typestate/describe.rs`.

#![feature(generic_const_items)]
#![allow(incomplete_features)]

const fn types_eq<A, B>() -> bool {
    let _ = core::mem::size_of::<(A, B)>();
    false
}

pub const TYPES_EQ<A, B>: bool = types_eq::<A, B>();

const fn remainder_ctfe_panic<Rem>() -> usize {
    let _ = core::mem::size_of::<Rem>();
    0
}

pub const REVEAL<Rem>: usize = remainder_ctfe_panic::<Rem>();
