//! `core::direct_const_arg!` as used by amaru-pure-stage on `rk/remove-gce-warnings`.
//!
//! rustc nightly-2026-09-04 accepts this. rust-analyzer does not implement the builtin,
//! so the call expands to nothing.

#![feature(generic_const_items, generic_const_args, min_generic_const_args)]
#![allow(incomplete_features)]

const fn types_eq<A, B>() -> bool {
    let _ = core::mem::size_of::<(A, B)>();
    false
}

/// Same shape as `amaru_pure_stage::typestate::list::TYPES_EQ`.
const TYPES_EQ<A, B>: bool = types_eq::<A, B>();

pub struct If<const B: bool>;
pub trait IsFalse {}
impl IsFalse for If<false> {}

/// Bound used like `TakeHead`'s `If<{ direct_const_arg!(TYPES_EQ::<...>) }>: IsFalse`.
pub trait NotSame<E> {
    type Index;
}

impl<Body, E> NotSame<E> for Body
where
    If<{ core::direct_const_arg!(TYPES_EQ::<Body, E>) }>: IsFalse,
{
    type Index = ();
}

const REVEAL<T>: usize = 0;

/// Where-clause form used by `Session::reveal` / `reveal_remainder!`.
pub fn reveal<T>()
where
    [(); core::direct_const_arg!(REVEAL::<T>)]:,
{
}

/// Witness parameter, same role as `I` on `Select` / `assert_after::<..., _>()`.
pub struct Here;

pub trait Select<E, I> {}

impl<Body, E> Select<E, Here> for Body
where
    If<{ core::direct_const_arg!(TYPES_EQ::<Body, E>) }>: IsFalse,
{
}

pub fn use_it() {
    fn assert_sel<T: Select<u8, I>, I>() {}
    assert_sel::<u32, _>();
}
