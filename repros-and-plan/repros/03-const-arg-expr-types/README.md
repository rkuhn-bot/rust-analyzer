# 03 — const-argument expressions have no type in the body map

What this shows: the expression nodes of const arguments that occur in bodies are `{unknown}` in `analysis-stats`, while the type that contains them is already correct.

This is one bug, not several features:

| Body | Expr node `analysis-stats` reports | Type of the containing pattern or call |
| --- | --- | --- |
| `let _x: StrParam<"Send">` | `"Send"` is `{unknown}` | `_x` is `StrParam<"Send">` |
| `let _x: IntParam<1>` | `1` is `{unknown}` | `_x` is `IntParam<1>` |
| `let _x: StrParam<{ <u8 as Desc>::TEXT }>` | path and block are `&'static str` | `_x` is `StrParam<"u8">` |
| `let _x: StrParam<{ TEXT }>` | `TEXT` and `{ TEXT }` are `{unknown}` | `_x` is `StrParam<"hi">` |
| `let ops: [u8; _] = [1, 2]` | `_` is `{unknown}` | `ops` is `[u8; 2]` |
| `type_last_segment::<[u8; 4]>()` | `4` is `{unknown}` | call is `fn type_last_segment<[u8; 4]>() -> &'static str` |

`&'static str` const parameters work (`unsized_const_params` / `adt_const_params` are not what makes `"Send"` unknown). Associated const arguments that fail `path_to_const` become anon consts and do get expression types. Local const paths that `path_to_const` accepts, and literals interned as values, never get `write_expr_ty`. Array length inference already produces `[u8; 2]` and `[u8; 4]`; only the length expression node is untyped.

Mapped onto amaru-gce, this is 38 of the 43 `{unknown}` rows: the string literals in `describe.rs` (including the `", "` / `" | "` arguments inside `impl_const_seq!` / `impl_const_par!`), the `4` in `typestate/effect.rs`, `{ TEXT }` in `typestate/tests.rs`, and the older `[T; _]` row.

`diagnostics` is clean. rustc accepts the crate.

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
rust-analyzer -v -v analysis-stats --only str_in_body .
```

Same flag for `int_in_body`, `assoc_in_body`, `local_const_arg`, `array_underscore`, `array_in_turbofish`. Expression lines:

```
20:21-20:27: {unknown}
20:31-20:39: StrParam<"Send">
20:8-20:10: StrParam<"Send">

24:21-24:22: {unknown}
24:26-24:34: IntParam<1>
24:8-24:10: IntParam<1>

35:23-35:41: &'static str
35:21-35:43: &'static str
35:47-35:55: StrParam<"u8">
35:8-35:10: StrParam<"u8">

40:23-40:27: {unknown}
40:21-40:29: {unknown}
40:33-40:41: StrParam<"hi">
40:8-40:10: StrParam<"hi">

44:18-44:19: {unknown}
44:23-44:29: [u8; 2]
44:8-44:11: [u8; 2]

53:29-53:30: {unknown}
53:4-53:32: fn type_last_segment<[u8; 4]>() -> &'static str
```

CSV unknown rows: 6. `??ty: 6`. No mismatches. No failed const evals worth treating as this bug (`TEXT` is a string literal and evaluates).
