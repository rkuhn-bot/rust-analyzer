//! `#[define_opaque]` + `type Alias = impl Trait`, as in `amaru-protocols`.
//!
//! rustc accepts this. rust-analyzer registers the attribute as a no-op and
//! does not collect defining bodies for free type-alias impl Trait, so the
//! closure fails to match the opaque alias.
//!
//! Consolidated from `/home/andrew/work/ra-repro` (originally nightly-2026-08-03).
//! The mismatch is unchanged on nightly-2026-09-04.

#![feature(type_alias_impl_trait)]

use std::{future::Future, marker::PhantomData, pin::Pin};

pub type IdFn = impl Fn(u32) -> u32 + Send + 'static;

#[define_opaque(IdFn)]
pub fn id_fn() -> IdFn {
    |x| x + 1
}

pub struct Effects<M> {
    _ph: PhantomData<M>,
}

pub trait StageState: Sized + Send {
    type LocalIn: Send;
}

pub type Mini<S>
where
    S: StageState + 'static,
= impl Fn(S, <S as StageState>::LocalIn, Effects<<S as StageState>::LocalIn>) -> Pin<Box<dyn Future<Output = S> + Send>>
    + Send
    + 'static;

#[define_opaque(Mini)]
pub fn mini<S: StageState + 'static>() -> Mini<S> {
    move |stage, _input, _eff| Box::pin(async move { stage })
}
