# 02 — generic const item parameters are discarded

What this shows: `const TYPES_EQ<A, B>: bool = types_eq::<A, B>();` and `const REVEAL<Rem>: usize = remainder_ctfe_panic::<Rem>();`.

The syntax parses. `ConstId::resolver` pushes a generic-param scope, but `GenericParams::with_store` returns an empty list for `GenericDefId::ConstId`, and `ConstSignature` has `generic_params` commented out. Inside the const body the parameters are `{unknown}`. There is no hard diagnostic. Const-eval then fails monomorphization.

This is `list.rs:202` and `describe.rs:44` in the amaru-gce `analysis-stats` CSV.

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

Exit 0. `diagnostic scan complete`, no errors.

```
rust-analyzer -v -v analysis-stats --output csv .
```

```
src/lib.rs,16:33,16:49,type,"fn types_eq<{unknown}, {unknown}>() -> bool"
src/lib.rs,23:31,23:58,type,"fn remainder_ctfe_panic<{unknown}>() -> usize"
Const eval for TYPES_EQ failed due MirLowerError(NotSupported("monomorphization resulted in errors"))
Const eval for REVEAL failed due MirLowerError(NotSupported("monomorphization resulted in errors"))
```

The `const fn` items `types_eq` and `remainder_ctfe_panic` themselves have no unknown types. Only the const items' initializers do.
