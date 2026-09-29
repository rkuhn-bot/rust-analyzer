# 01 — `direct_const_arg!` is an unimplemented builtin

What this shows: `core::direct_const_arg!(TYPES_EQ::<…>)` and `core::direct_const_arg!(REVEAL::<…>)`, the two shapes in `amaru-pure-stage` on `rk/remove-gce-warnings` (`If<{ … }>: IsFalse` and `[(); …]:,`).

rust-analyzer expands an unknown `#[rustc_builtin_macro]` to an empty token tree. Inside `{ … }` that is `()`, so the const argument is `()` where a `bool` is required. Inside `[(); …]` the empty expansion is a syntax error.

A single `Select` impl still assigns the witness (`use_it` infers `Here`) even while the macro is an error. The `In<{unknown}>` rows in `typestate/tests.rs` need the real overlapping `TakeHead` / `Select` impls; they are a consequence of these bounds not being solved, not a separate language feature. See `PLAN.md`.

## rustc

Toolchain: `nightly-2026-09-04` (`rust-toolchain.toml`).

```
cargo check --offline
```

Exit 0.

## rust-analyzer

Binary: `/home/andrew/work/rust-analyzer/target/release/rust-analyzer` (`03fcb77246`, 2026-09-27). `LineCol` in `diagnostics` is 0-based; file lines are one higher.

```
unset RUSTUP_TOOLCHAIN
rust-analyzer diagnostics .
```

Exit 1. Errors (file lines in parentheses):

```
Error Ra("macro-error", Error) LineCol { line: 27, col: 9 }..{ line: 27, col: 31 }: this built-in macro is not implemented
Error RustcHardError("E0308") LineCol { line: 27, col: 54 }..{ line: 27, col: 55 }: expected bool, found ()
Error Ra("macro-error", Error) LineCol { line: 37, col: 9 }..{ line: 37, col: 31 }: this built-in macro is not implemented
Error SyntaxError LineCol { line: 37, col: 15 }..{ line: 37, col: 31 }: Syntax Error in Expansion: expected expression
Error Ra("macro-error", Error) LineCol { line: 48, col: 9 }..{ line: 48, col: 31 }: this built-in macro is not implemented
Error RustcHardError("E0308") LineCol { line: 48, col: 54 }..{ line: 48, col: 55 }: expected bool, found ()
```

Those are file lines 28, 38, and 49.

```
rust-analyzer analysis-stats --output csv .
```

```
src/lib.rs,15:29,15:45,type,"fn types_eq<{unknown}, {unknown}>() -> bool"
```

That row is the generic-const-item bug (repro 02), not the macro. `use_it` is typed `fn assert_sel<u32, Here>()` / `()`. Const-eval of `TYPES_EQ` fails with `MirLowerError(NotSupported("monomorphization resulted in errors"))`.

The `diagnostics` process then prints an anyhow backtrace because any error makes the CLI exit 1. That backtrace is not an analysis panic.
