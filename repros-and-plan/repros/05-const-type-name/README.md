# 05 — control: `const_type_name` already evaluates

What this shows: a monomorphic const that calls `core::any::type_name` is typed and evaluated. This was a candidate cause of the `{unknown}` rows. It is not one.

`type_name` inside `amaru-pure-stage` fails only when it is reached through a generic const item whose parameters are not in scope (repro 02). The interpreter already has a `type_name` shim.

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

Exit 0.

```
rust-analyzer -v -v analysis-stats --only NAME .
```

```
15:31-15:54: fn type_last_segment<u8>() -> &'static str
15:31-15:56: &'static str
Failed const evals: 0 (0%)
```

No CSV `type` or `mismatch` rows. `??ty: 0`.
