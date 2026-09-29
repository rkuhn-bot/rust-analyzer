# 04 — `#[define_opaque]` does not define a free type-alias impl Trait

What this shows: the false `E0308` in `amaru-protocols` `miniprotocol.rs`. The attribute is registered and expands as the identity. Defining bodies of a free `type Alias = impl Trait` are not collected, so the closure does not satisfy the opaque type.

Consolidated from `/home/andrew/work/ra-repro` (that tree used nightly-2026-08-03). The mismatch is the same on nightly-2026-09-04. Two cases:

1. `IdFn`, a free alias of `impl Fn(u32) -> u32 + Send + 'static`.
2. `Mini<S>`, the same shape as `Miniprotocol`, with `StageState::LocalIn` in the function type.

The associated type in case 2 is resolved on both sides of the mismatch. The failure is the opaque alias, not the typestate projection.

## rustc

Toolchain: `nightly-2026-09-04`.

```
cargo check --offline
```

Exit 0.

## rust-analyzer

```
unset RUSTUP_TOOLCHAIN
rust-analyzer diagnostics .
```

Exit 1.

```
Error RustcHardError("E0308") LineCol { line: 17, col: 4 }..{ line: 17, col: 13 }:
  expected impl Fn(u32) -> u32 + Send + 'static, found {closure}
Error RustcHardError("E0308") LineCol { line: 37, col: 4 }..{ line: 37, col: 61 }:
  expected impl Fn(S, <S as StageState>::LocalIn, Effects<...>) -> Pin<Box<dyn Future<Output = S> + Send + 'static, Global>> + Send + 'static,
  found {closure}
```

`LineCol` is 0-based. Those are file lines 18 and 38.

```
rust-analyzer analysis-stats --output csv .
```

```
src/lib.rs,18:4,18:13,mismatch,"impl Fn(u32) -> u32 + Send + 'static","impl Fn(u32) -> u32"
src/lib.rs,38:4,38:61,mismatch,"impl Fn(S, <S as StageState>::LocalIn, Effects<<S as StageState>::LocalIn>) -> Pin<Box<dyn Future<Output = S> + Send + 'static, Global>> + Send + 'static","impl Fn(S, <S as StageState>::LocalIn, Effects<<S as StageState>::LocalIn>) -> Pin<Box<dyn Future<Output = S> + Send + 'static, Global>>"
```

`??ty: 0`. The found type drops `+ Send + 'static` on `IdFn`. On `Mini` the found and expected function types match except the expected type still carries the opaque's extra auto-trait bounds. MIR lowering reports `HasErrors` for both bodies.
