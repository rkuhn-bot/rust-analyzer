# Plan: rust-analyzer vs amaru typestate (`rk/remove-gce-warnings`)

**Status: Approved** (2026-09-29), plus the quality-gates section below.

Documentation only. Nothing here has been implemented. No rust-analyzer source was edited, no Amaru source was edited, no pull request was opened.

**Not touching rustc.** Every change in the four PRs below is inside rust-analyzer crates: `hir-def`, `hir-expand`, `hir-ty`, and (only if a feature name is matched by symbol) `intern`. `crates/ide-db/src/generated/lints.rs` is generated and is not a hand-edit site; see piece 2. `rustc` and `rustc_type_ir` are dependencies. Their types (`ConstKind`, `Features`, `AnonConstKind`) are implemented by rust-analyzer in `hir-ty`. Those crates are not modified.

## (a) Problem

`amaru-pure-stage` on `pragma-org/amaru` branch `rk/remove-gce-warnings` (worktree `/home/andrew/work/amaru-gce`, commit `1f8950905581b466a07e9197e460ab4a76081e80`, toolchain `nightly-2026-09-04`) type-checks with rustc:

```
cargo check -p amaru-pure-stage --all-targets --locked
```

exit 0. rust-analyzer `03fcb77246` (built 2026-09-27 from this fork's `master`) does not.

`diagnostics` on the amaru-gce workspace (0-based `LineCol`; file lines below are 1-based) reports these error-severity diagnostics in pure-stage, all from `core::direct_const_arg!`:

| File line | Diagnostic |
| --- | --- |
| `typestate/list.rs:249` | `macro-error`: this built-in macro is not implemented |
| `typestate/list.rs:249` | E0308 expected `bool`, found `()` |
| `typestate/list.rs:261` | same pair |
| `typestate/list.rs:272` | same pair |
| `typestate/session.rs:274` | `macro-error`, plus `Syntax Error in Expansion: expected expression` |

The `amaru-protocols` `miniprotocol.rs:201` E0308 (`#[define_opaque]` / type-alias impl Trait) is out of scope. Amaru is removing that feature; the typestate API replaces it. That diagnostic is expected and ignored. Repro `04-define-opaque` was removed on 2026-09-29.

`analysis-stats --output csv` (this enables `#[cfg(test)]`; `diagnostics` does not) on pure-stage is 43 `type` rows and 0 `mismatch` rows. Grouped by root cause:

**1. Generic const item parameters are not stored (2 rows).** `list.rs:202` is `fn types_eq<{unknown}, {unknown}>() -> bool` inside `const TYPES_EQ<A, B>`. `describe.rs:44` is `fn remainder_ctfe_panic<{unknown}>() -> usize` inside `const REVEAL<Rem: ConstDesc>`. The corresponding `const fn` items are fine. Const-eval of the const items fails with `MirLowerError(NotSupported("monomorphization resulted in errors"))`.

**2. `direct_const_arg` bounds do not select among overlapping impls (3 rows).** `tests.rs:363`, `:370`, `:372` display `In<{unknown}>` as the witness argument of `assert_after` / `describe_after`. A single-impl reduction still infers the witness (repro 01, `use_it` is `assert_sel<u32, Here>`). These three rows should disappear once the bounds evaluate; they are not their own language feature.

**3. Const-argument expression nodes are never given a type (38 rows).** The type that contains the expression is already what rustc computes. These rows stay in scope: correct inference includes them.

- 8 rows, `describe.rs` lines 241, 245, 249, 253, 257, 261, and two on 265: the string literals `"Send"`, `"Call"`, `"SendAny"`, `"Receive"`, `"Schedule"`, `"External"`, `"Repeat<"`, `">"`.
- 9 rows on `describe.rs:311` (`impl_const_seq!(…)`): one `", "` const argument per recursive expansion.
- 18 rows on `describe.rs:312` (`impl_const_par!(…)`): two `" | "` arguments per level (Par and Choice).
- 1 row, `typestate/effect.rs:294`: the `4` in `type_last_segment::<[foo::Bar; 4]>()`.
- 2 rows, `tests.rs:269`: `TEXT` and `{ TEXT }` in `Remainder<{ TEXT }>`.

Repro 03 measures the same split on small crates: `let _x: StrParam<"Send">` gives `_x: StrParam<"Send">` while `"Send"` is `{unknown}`; `let ops: [u8; _] = [1, 2]` gives `ops: [u8; 2]` while `_` is `{unknown}`; `type_last_segment::<[u8; 4]>()` is `fn type_last_segment<[u8; 4]>() -> &'static str` while `4` is `{unknown}`; `StrParam<{ TEXT }>` is `StrParam<"hi">` while the block and the path are `{unknown}`.

**Done** means both of the following on amaru-gce, and nothing else:

- `diagnostics`: zero error-severity diagnostics in `amaru-pure-stage`.
- `analysis-stats --output csv`: zero `type` rows in `amaru-pure-stage` (the 2 + 3 + 38 above). That is the bar for correct inference: no `{unknown}` where rustc knows the type, including const-argument expression nodes.

The `miniprotocol.rs` E0308 is not part of either check.

Candidates that are **not** gaps for this code:

- `const_type_name`. A monomorphic `const NAME: &'static str = type_last_segment::<u8>()` is typed `fn type_last_segment<u8>() -> &'static str` and const-eval succeeds (repro 05). The shim is `type_name` in `crates/hir-ty/src/mir/eval/shim.rs`. Failures in Amaru are the generic const item in (1), which never gets as far as a monomorphic `type_name` call.
- `unsized_const_params` / `adt_const_params` for `&'static str`. `struct StrParam<const S: &'static str>` lowers `"Send"` to the const value. The pattern type is `StrParam<"Send">`. `intern_const_ref` already builds a `ValTree` for `&str`. The `valtree.rs` FIXME is for `TyKind::Adt`, which this branch does not put in const-parameter position. `Remainder<const TEXT: &'static str>` is the str case, not an ADT case.
- Associated consts used as const arguments, as values. `StrParam<{ <u8 as Desc>::TEXT }>` is `StrParam<"u8">`, and both the path and the block are `&'static str`, because that path misses `path_to_const` and falls through to anon-const inference. The `{unknown}` in `tests.rs:269` is the local-const path that `path_to_const` accepts, which skips `write_expr_ty`. The const value is still `"hi"` in the reduced crate.
- Inferred array length, as a type. `[u8; _]` in a body is `[u8; 2]`. Only the `_` expression node is untyped, which is piece 4.

`Features::generic_const_exprs` and `Features::generic_const_args` both return `false`, and `anon_const_kind` always returns `AnonConstKind::GCE`. That is a trap if someone tries to implement this by turning generic const exprs on. It is not what produces these 43 rows. Direct const arguments are `UnevaluatedConst` of a `ConstId` with substitutions, built whether or not a feature gate is set (see piece 3).

## (b) Missing pieces

Four pieces, four PRs.

| Piece | Why it is needed | Repro | rust-analyzer location | Upstream | Size / risk | Depends on |
| --- | --- | --- | --- | --- | --- | --- |
| 1. Generic parameters of const items | Without them, `TYPES_EQ` / `REVEAL` bodies and their const-eval are wrong. Required before a path `TYPES_EQ::<…>` can evaluate. | `repros/02-generic-const-item` | `ConstSignature` in `crates/hir-def/src/signatures.rs` (`// generic_params: GenericParams` is commented out, commit `aae8e50546`). `GenericParams::with_store` / `with_source_map` return `EMPTY` for `GenericDefId::ConstId` (`crates/hir-def/src/hir/generics.rs`). `ConstId::resolver` already calls `push_generic_params_scope` and the comment says nightly const items have generic params (`crates/hir-def/src/resolver.rs`). | Parse-only: [rust-lang/rust-analyzer#19643](https://github.com/rust-lang/rust-analyzer/pull/19643) (`812a035689`, ChayimFriedman2), closing [#19516](https://github.com/rust-lang/rust-analyzer/issues/19516). No semantic follow-up. `git log -S direct_const_arg` and `--grep generic_const_items` are empty. | Medium / medium. Touches a salsa signature query and every const's param scope. Param bounds (`Rem: ConstDesc`) must be lowered with the same path as functions, or the body still will not see the trait. | none |
| 2. Builtins `direct_const_arg` and `gca`, and feature-name aliases | Stops the empty expansion. That is the hard `macro-error`, the `expected bool, found ()`, and the `expected expression` syntax error. Both names are required because the sysroot attribute changed. Old `#![feature]` spellings must compare equal to the new `gca_*` names wherever rust-analyzer looks a feature name up. | `repros/01-direct-const-arg` | Macro: `register_builtin!` in `crates/hir-expand/src/builtin/fn_macro.rs`. Lookup is `find_builtin_macro` from `collect_macro_rules` in `crates/hir-def/src/nameres/collector.rs` (~2540), using the `#[rustc_builtin_macro]` name. Misses become `MacroExpander::UnimplementedBuiltIn`. Expansion is `expand_unimplemented_builtin_macro` in `crates/hir-expand/src/lib.rs`. Call-site diagnostic: `crates/ide-diagnostics/src/handlers/macro_error.rs`. New names need symbols in `crates/intern/src/symbol/symbols.rs`. Feature names: `UnstableFeatures::enable` / `is_enabled` in `crates/hir-def/src/unstable_features.rs`. | No rust-analyzer issue or PR for the builtin. rustc: [#163198](https://github.com/rust-lang/rust/pull/163198) renamed the macro to `gca!` (khyperia, merged 2026-09-23). [#163306](https://github.com/rust-lang/rust/pull/163306) renamed the feature gates (merged 2026-09-25) and did not rename the macro again. | Small / low. The expander returns the inner token tree unchanged. That alone does not make `If<{ TYPES_EQ::<…> }>: IsFalse` true. The alias is a lookup change, not a behavior change: pieces 1–4 do not branch on the gate. | none for the registration. Useless for Amaru's const bounds until piece 3. |
| 3. Path to a generic const item as `UnevaluatedConst { def, args }` | `path_to_const` returns `ConstHasGenerics` for any `ConstId` that has parameters, then `create_anon_const` builds an anon const. Where-clause anon consts are not allowed to use generic parameters (`ForbidParamsAfterReason::AnonConst`). mGCA is explicitly not implemented. The bool never becomes `true` or `false`, so overlapping `TakeHead` impls do not determine `I`. Accept the path permissively: no check of the macro and no check of `#![feature]`. | `repros/01-direct-const-arg` plus `02` | `path_to_const` and `create_anon_const` in `crates/hir-ty/src/consteval.rs` (~319 and ~354). The `ConstHasGenerics` arm is excluded from the early-success match so it falls through to anon-const creation. `evaluate_const` in `crates/hir-ty/src/next_solver/solver.rs` (~257) already calls `const_eval(const_id, subst)`. It turns every error into an error const; the FIXME says rustc returns `None` for `HasGenericsOrInfers`. | No rust-analyzer issue. rustc feature history is the mGCA series (`min_generic_const_args`, tracking issue 132980). On nightly-2026-09-04 the macro attribute is `#[rustc_builtin_macro(direct_const_arg)]` and the feature is `min_generic_const_args` (`library/core/src/marker.rs` ~1082). The doc comment still mentions `type const` and `direct!`; the macro the branch calls is `direct_const_arg`. On nightly 2026-09-28 the attribute is `#[rustc_builtin_macro(gca)]`, the macro is `gca!`, and the feature is `gca_min_const_items`. `type const` was removed by [rust-lang/rust#162517](https://github.com/rust-lang/rust/pull/162517) (merged 2026-09-12), after the Amaru pin. Amaru does not use `type const` or `#[always_gca]`. rust-analyzer parsed `type const` in [#22046](https://github.com/rust-lang/rust-analyzer/pull/22046), closing [#22038](https://github.com/rust-lang/rust-analyzer/issues/22038). That parse is dead syntax. | Medium / medium. Substituting the path's generic arguments and evaluating must not collapse an evaluation failure to `false`, or `IsFalse` will pick the wrong `TakeHead` impl. Keep these as `ConstId` unevaluated consts. Do not flip `generic_const_exprs()` or `generic_const_args()` to `true`. | Piece 1. Piece 2 for the Amaru call sites. A unit test can use the path without the macro. |
| 4. Record types of const-argument expressions | Clears the 38 `{unknown}` rows. Hover on `"Send"`, on `_` in `[T; _]`, and on `{ TEXT }` shows `{unknown}` today even though the surrounding type is right. `expr_ty` returns the error type when the expression was never inserted (`InferenceResult::expr_ty`). This is part of the done bar, not optional polish. | `repros/03-const-arg-expr-types` | `TyLoweringContext::lower_expr_as_const` / `lower_const` in `crates/hir-ty/src/lower.rs` (~453) never writes an expression type. `InferenceContext::create_anon_const` in `crates/hir-ty/src/infer.rs` (~1967) writes the expected type only on the outer expression, and only when the result is not an `AnonConstId`. Type ascriptions and turbofish in bodies go through the lowering context, not that function. Literals that `intern_const_ref` turns into values, and paths that `path_to_const` accepts, are exactly the nodes that stay untyped. | No issue found for this bookkeeping hole. | Small / low. Write the expected const type onto the expression (and, for a block whose tail was unwrapped, onto the block and the tail). Do not re-infer the expression from scratch. | none |

Not gaps, unchanged from the first draft:

| | Repro | Where it already works |
| --- | --- | --- |
| `const_type_name` | `repros/05-const-type-name` | `crates/hir-ty/src/mir/eval/shim.rs` `type_name` arm (~783). No rust-analyzer issue. |
| `&'static str` const params; `adt_const_params` for this branch | `repros/03-const-arg-expr-types` (`StrParam<"Send">`) | `intern_const_ref` in `consteval.rs` (~123) handles `&str`. `crates/hir-ty/src/next_solver/consts/valtree.rs` (~221) returns an error const for `TyKind::Adt` (FIXME from [#19095](https://github.com/rust-lang/rust-analyzer/pull/19095)). Same string in `mir/eval.rs` (~2071). No open issue. Do not start the ADT work for this branch. |

### What "alias the feature gates" means in this tree

[rust-lang/rust#163306](https://github.com/rust-lang/rust/pull/163306) renamed:

| Old `#![feature]` | New `#![feature]` |
| --- | --- |
| `min_generic_const_args` | `gca_min_const_items` |
| `generic_const_args` | `gca_const_items` |
| `macroless_generic_const_args` | `gca_macroless_args` |
| `macroless_const_item_generic_const_args` | `gca_macroless_items` |

`generic_const_items`, `const_type_name`, `unsized_const_params`, and `adt_const_params` were not renamed. The macro rename (`direct_const_arg` → `gca`) is piece 2's builtin table, not this alias table.

Three different "feature" mechanisms exist. Only the first one looks up a name the crate wrote.

1. **`hir_def::unstable_features::UnstableFeatures`** (`crates/hir-def/src/unstable_features.rs`). `define_unstable_features!` stores a bool per feature that analysis branches on, plus `all: FxHashSet<Symbol>` of every `#![feature(...)]` name. `enable` sets a bool only on an exact `sym::` match. `is_enabled` is `all.contains`. `Crate::is_unstable_feature_enabled` (`crates/hir/src/lib.rs`) calls that. None of the eight names above is in the macro today, and nothing in const lowering calls `is_enabled` on them. The alias goes here: `enable` inserts both symbols of a pair into `all`, and `is_enabled` is true if either symbol is present. No new bool field, because nothing reads one. Do this in PR 2 so a later `is_enabled(sym::gca_min_const_items)` agrees with a crate that still says `min_generic_const_args`, and the other way around.

2. **`hir_ty::next_solver::interner::Features`** (`crates/hir-ty/src/next_solver/interner.rs` ~681). This implements the `rustc_type_ir` trait. `generic_const_args()`, `generic_const_exprs()`, and `feature_bound_holds_in_crate` return `false` and do not read `#![feature]`. They are not the alias site. Leave them `false`. Permissive path acceptance is what makes that safe: piece 3 does not call them. `generic_const_exprs` was not renamed to a `gca_*` gate; do not treat it as one.

3. **`crates/ide-db/src/generated/lints.rs`**. Generated by `xtask/src/codegen/lints.rs` from rustc's unstable book. It currently has separate `Lint` entries labeled `min_generic_const_args` and `generic_const_args` because that is what the book said when codegen last ran. Those entries are hover text, not the analysis gate. Do not hand-edit the file. Regenerating against a post-#163306 rustc replaces the labels with `gca_*`. Hover on an old name after that regen will miss until someone adds an alias in the generator. That is outside these four PRs.

## (c) Order of work

**Done**, for the series, is the bar in (a): zero error-severity `diagnostics` in `amaru-pure-stage`, and zero `analysis-stats` `type` rows in `amaru-pure-stage`. The `miniprotocol.rs` E0308 stays.

Dependency order, which is the PR order:

1. **Const-item generic parameters**, or nothing that evaluates `TYPES_EQ::<A, B>` can monomorphize.
2. **Register `direct_const_arg` and `gca`**, identity expansion, plus the `UnstableFeatures` name alias. Independent of (1). Amaru stays red until (3).
3. **`path_to_const` builds `UnevaluatedConst` with the path's generic arguments, and that const evaluates.** Any such path, macro or not, feature or not. Needs (1). Needs (2) at the Amaru call sites.
4. **Write expression types for const arguments in bodies.** Independent. Required for the done bar. Does not fix a hard error.

Do not start with `unsized_const_params`, `adt_const_params`, `const_type_name`, or a new array-length solver. Those are not what the 43 rows are.

### PR 1 — generic parameters on const items

Test: `check_types` in `crates/hir-ty/src/tests/regression.rs` on

```rust
const fn id<T>() -> usize { core::mem::size_of::<T>() }
const SIZE<T>: usize = id::<T>();
```

The call `id::<T>()` must show `fn id<T>() -> usize`, not `fn id<{unknown}>()`. Add a param-bound case `const REVEAL<Rem: Trait>: usize = Rem::ASSOC` only if the bound is lowered in the same PR; otherwise the test lies.

Acceptance on amaru-gce, `analysis-stats --output csv` filtered to `amaru-pure-stage`:

- The `list.rs:202` and `describe.rs:44` rows are gone.
- Hard errors are unchanged (still the four `direct_const_arg` sites).
- `Const eval for TYPES_EQ` / `REVEAL` may still fail until PR 3, because nothing monomorphizes them yet. A remaining generic-eval failure is not a regression of this PR.

### PR 2 — builtins `direct_const_arg` and `gca`, and the feature-name alias

Test: `crates/hir-def/src/macro_expansion_tests/builtin_fn_macro.rs`. Expansion of `direct_const_arg!(TYPES_EQ::<A, B>)` and of `gca!(TYPES_EQ::<A, B>)` is the inner tokens, not empty. Register both names. The builtin name comes from `#[rustc_builtin_macro(...)]` in the sysroot the IDE is analyzing.

Test the alias next to `crates/hir-def/src/unstable_features.rs`: a crate with `#![feature(min_generic_const_args)]` reports `is_enabled` for both `min_generic_const_args` and `gca_min_const_items`, and a crate with only `gca_min_const_items` reports both as well. Same for the other three pairs. No test should require `Features::generic_const_args()` to become `true`.

Acceptance on amaru-gce `diagnostics`:

- The four `macro-error` rows and the session.rs syntax error are gone.
- E0308 may remain, with a different found-type, until PR 3. Say so in the PR. Do not make the macro expand to `false`.
- `miniprotocol.rs` E0308 is still present and is not this PR's problem.

### PR 3 — generic const items in const-argument position

Accept the path permissively. `If<{ TYPES_EQ::<u8, u16> }>` and `If<{ direct_const_arg!(TYPES_EQ::<u8, u16>) }>` lower the same way. Neither `#![feature(min_generic_const_args)]` nor `#![feature(gca_min_const_items)]` is consulted.

Test: `crates/hir-ty/src/tests/traits.rs` (or `regression.rs` if that is where where-clause tests live). Two cases, each written once as a bare path and once through `direct_const_arg!`:

- `types_eq` returns `false` for distinct types, `If<{ TYPES_EQ::<u8, u16> }>: IsFalse` holds, a `Select<E, Here>` impl with that bound is chosen, and a call `assert_sel::<u32, _>()` infers `Here`.
- A second const that returns `true` for identical types must not satisfy `IsFalse`. This is the wrong-impl guard.

Const-eval test next to `crates/hir-ty/src/mir/eval/tests.rs` or `consteval/tests`: `const EQ<A, B>: bool = …` instantiated at `u8, u8` and `u8, u16` yields the two bools. Use `size_of` or `type_name`; both already evaluate when the parameters are real (repro 05).

Implementation notes:

- In `path_to_const`, when the `ConstId` has generic parameters, lower the path's generic arguments and return `Const::new_unevaluated(UnevaluatedConst { def, args })`. Do not return `ConstHasGenerics` for that case.
- Do not wrap that path in an anon const. `create_anon_const`'s comment is right that mGCA anon consts are unimplemented; this path should not need them.
- Leave `Features::generic_const_exprs`, `Features::generic_const_args`, and `feature_bound_holds_in_crate` false. Leave `anon_const_kind` alone unless a test proves the solver refuses a `ConstId` unevaluated const because of it.
- `evaluate_const` already evaluates `ConstId` with substitutions. After PR 1 the substitutions are real types. If evaluation fails, do not invent `false`. Matching rustc's `None` on `HasGenericsOrInfers` (the existing FIXME in `solver.rs`) matters here: an error const in the `If<…>` argument is worse than an unknown.

Acceptance on amaru-gce:

- `diagnostics` has zero error-severity diagnostics in `amaru-pure-stage`.
- `analysis-stats` CSV for pure-stage no longer has the three `In<{unknown}>` rows in `tests.rs`.
- The 38 const-argument expression rows are still present until PR 4.
- `miniprotocol.rs` E0308 is ignored.

### PR 4 — expression types for const arguments

Test: `check_types` in `crates/hir-ty/src/tests/regression.rs`, with the `//^` marker on the expression that is unknown today:

```rust
fn str_in_body() {
    let _x: StrParam<"Send"> = StrParam;
      //^ StrParam<&str> or whatever display the suite uses for StrParam<"Send">
}
```

The marker has to sit on `"Send"`, on `1`, on `_`, and on `TEXT` in `{ TEXT }`, not only on the binding. Bindings already have the right type, so a test that only checks `_x` stays green and proves nothing. Copy the expected display strings from a `check_types` failure rather than guessing them.

Acceptance on amaru-gce, which is the done bar for the series:

- `diagnostics`: zero error-severity diagnostics in `amaru-pure-stage`.
- `analysis-stats --output csv` filtered to `amaru-pure-stage`: zero `type` rows (`??ty` for that crate goes from 43 to 0).
- `miniprotocol.rs` E0308 still appears in a workspace `diagnostics` run and is ignored.

## (d) Answered questions

Recorded 2026-09-29 from Roland's review.

1. **`#[define_opaque]` / TAIT is out of scope.** Amaru will remove it. The typestate API replaces it. Piece 5, PR 5, and repro `04-define-opaque` are deleted. The `miniprotocol.rs` E0308 is expected and ignored.
2. **Done means no errors and correct inference.** Zero error diagnostics in `amaru-pure-stage`, and zero `analysis-stats` `type` rows there. The 38 const-argument expression nodes are part of that bar (piece 4 / PR 4), not optional.
3. **Paths to generic const items are accepted permissively.** No gating on `direct_const_arg!` / `gca!` and no gating on the feature flag. Piece 3 and PR 3 say so. The remaining risk is evaluating the wrong bool, not missing the gate.
4. **Both macro names, and old feature names alias the new ones.** Register `direct_const_arg` and `gca`. The four renames from [rust-lang/rust#163306](https://github.com/rust-lang/rust/pull/163306) are aliases in `UnstableFeatures::enable` / `is_enabled`. `Features` in `hir-ty` stays hardcoded `false`. The generated lints table is not hand-edited.

No open questions remain.

## (e) Risks and what maintainers may object to

- **Wrong bool.** `TakeHead` has overlapping impls distinguished by `If<{ TYPES_EQ::<…> }>: IsFalse`. Evaluating the const as `false` on failure, or letting an error const satisfy `IsFalse`, silently picks an impl. The true and false tests in PR 3 are there for that reason. `evaluate_const` currently maps all failures to an error const (FIXME against rustc's `None`). Permissive path acceptance does not change this; it only means the path is lowered even when the crate forgot the macro.
- **Do not enable `generic_const_exprs`.** `Features::generic_const_exprs()` and `generic_const_args()` return `false` on purpose, and `anon_const_kind` is hardcoded to `GCE` with a FIXME. Turning the flags on to make where-clause anon consts see generic parameters would take the mGCA surface in `create_anon_const`, which this plan does not need. The representation is an unevaluated const item with substitutions. The feature-name alias does not flip these methods.
- **Builtin registry.** Every `#[rustc_builtin_macro]` name must be registered or it becomes an error at every call. ChayimFriedman2 noted that on the `define_opaque` registration ([#21183](https://github.com/rust-lang/rust-analyzer/pull/21183), `a4612ce527`): registering the name does not implement the feature. Adding `direct_const_arg` and `gca` is the same pattern. Skipping one of the two names is user-visible as soon as the sysroot moves. The `define_opaque` expander stays a no-op; this plan does not change it.
- **`ConstSignature` salsa change.** Adding `generic_params` changes the tracked signature of every const. Cycles are plausible if a const parameter's type or default mentions the const. Amaru's const items do not do that (`TYPES_EQ<A, B>`, `REVEAL<Rem: ConstDesc>`). A test with a const-parameter default that refers to an earlier parameter is worth having so this does not only get exercised by Amaru.
- **`adt_const_params`.** There is a real FIXME, and a reviewer who sees `&'static str` const parameters may ask for the ADT work in the same PR. The measurements say no. `intern_const_ref` already handles `&str`. Bundling ADT valtrees does not move the amaru-gce numbers.
- **`type const`.** rust-analyzer grew a parser for it ([#22046](https://github.com/rust-lang/rust-analyzer/pull/22046)). rustc deleted the syntax in [#162517](https://github.com/rust-lang/rust/pull/162517). The const-item work is `const NAME<T>: Ty = …`, not `type const`.
- **Generated lints.** A reviewer may want the old feature names added as aliases inside `ide-db/src/generated/lints.rs`. That file is codegen output. Hand edits will be wiped. The lookup that has to honor the alias is `UnstableFeatures` in `hir-def`.
- **Expression-type PR looks like a no-op.** `check_types` on the binding stays green today. Reviewers will want the marker on the literal. Otherwise PR 4 will not be believed, and the 38 rows will remain. Those rows are part of the done bar.

## Quality gates and code style

Every PR in this series must pass the gates below and match the neighboring code. Sources: `CONTRIBUTING.md` (it points at the book), `docs/book/src/contributing/README.md`, `style.md`, `testing.md`, `architecture.md`, `.github/workflows/ci.yaml`, `.cargo/config.toml`, and `xtask`.

### Commands committers run

`.cargo/config.toml` aliases: `xtask` is `run --package xtask --bin xtask --`, `codegen` is `run --package xtask --bin xtask -- codegen`, `lint` is `clippy --all-targets -- --cap-lints warn`. The CI clippy command is stricter than `cargo lint`.

The ubuntu tests job in `.github/workflows/ci.yaml` runs on every repository, including this fork:

```
cargo codegen --check
cargo nextest run --no-fail-fast --hide-progress-bar
cargo machete
```

`cargo codegen --check` fails when a generated file differs from what `xtask codegen` would write. Do not hand-edit those files.

These jobs are wrapped in `if: github.repository == 'rust-lang/rust-analyzer'`, so GitHub will not run them on `rkuhn-bot/rust-analyzer`. They are still the committer gates. Run them locally before pushing:

```
cargo fmt -- --check
cargo clippy --all-targets -- -D clippy::disallowed_macros -D clippy::dbg_macro -D clippy::todo -D clippy::print_stdout -D clippy::print_stderr
```

The fmt job uses stable `rustfmt`. The clippy job uses stable `clippy` plus stable `rust-src`, because clippy's output depends on whether `rust-src` is installed (rust-lang/rust-clippy#14625).

`cargo xtask tidy` is not its own CI job. `xtask/src/tidy.rs` checks the `lsp/ext.rs` hash against `lsp-extensions.md`, trailing whitespace, `#[should_panic]` (banned outside an allowlist, unless the previous line contains `FIXME`), Cargo.toml dependency versions, the license set, tidy docs, and cov marks. `docs/book/src/contributing/architecture.md` says formatting and tidy are covered by `cargo test`, and that there are no further CI checks. That sentence is behind `ci.yaml`: CI also runs fmt, clippy, codegen, nextest, and machete as separate steps. Run tidy as well:

```
cargo xtask tidy
```

Also in `ci.yaml`, not part of proving these four pieces unless the change actually touches them:

- `cargo miri test -p intern` (nightly miri). Piece 2 adds symbols in `intern`; run the intern tests. Miri is the extra gate.
- `cargo build -p rust-analyzer`, then `analysis-stats` on the rust-analyzer repo and on the sysroot library.
- Cross `cargo check` of `-p ide` for `powerpc-unknown-linux-gnu`, `x86_64-unknown-linux-musl`, and `wasm32-unknown-unknown`.

`docs/book/src/contributing/README.md`: `cargo test` is the local signal. Long tests are skipped unless `RUN_SLOW_TESTS=1`. CI itself runs `cargo nextest`, not `cargo test`. For each piece, run the tests of the crates that changed (`hir-def`, `hir-ty`, `hir-expand`, `intern`) under nextest, and `cargo test` for those crates if nextest is not installed. A green `cargo test` on the touched crates plus fmt, clippy, codegen `--check`, tidy, and machete is the bar. The workspace `rust-version` is 1.98. These gates run on stable. The nightly override on the rust-analyzer checkout is only for the Amaru release binary.

### Style rules that apply to these pieces

From `style.md` and `testing.md`. Match the function next to the edit (`ConstSignature` next to `FunctionSignature`, a new builtin next to the existing `register_builtin!` arms).

- Small PRs. These four pieces are already that split. No new crates.io dependency. No new `pub` item beyond the `ConstSignature` field this plan already calls for, and keep that diff small.
- PR titles from the user's perspective. Prefix `fix: ` (user-visible inference). `internal: ` only if nothing a user can see changed. Avoid `@mentions` in commit messages. `changelog [fix]` in the PR body is the other accepted mark. These branches are not opened as PRs here; the title prefix is still what the commit should carry.
- Tests are minimal snippets in unindented raw strings. `//- minicore: …` is required: fixtures have no std (`crates/test-utils/src/fixture.rs`, flags listed at the top of `minicore.rs`). `check_types` labels the node that is wrong today (`//^ type`), not only the binding. `check_infer` uses `expect![[ ]]`; set `UPDATE_EXPECT=1` to fill it in rather than typing the ranges by hand. One `cov_mark` per test, and do not reuse a mark. No `#[should_panic]` (tidy enforces this). No `#[ignore]`: assert the wrong behavior and leave a FIXME (`style.md`; tidy does not scan for `ignore`).
- Comments are sentences: capital letter, final period. For `.md` files the book asks for one sentence per line. This plan is already wrapped to match itself; new plan text stays wrapped. Code comments follow the sentence rule.
- Do not allocate a `Vec` or `String` where an iterator or an existing owner will do. Prefer `rustc_hash::FxHashMap` and `FxHashSet`.
- Boring names taken from the type. Established short names: `db`, `ctx`, `acc`, `res`, `it`, `n_foos`, `foo_idx`. Keyword mangling, not `r#`: `krate`, `ty`, `mac`, `func`, `enum_`, `trait_`. American spelling.
- Clippy allows belong in `[workspace.lints.clippy]` in the workspace `Cargo.toml`. Do not add a one-off `#[allow]` unless the surrounding code already has that allow for the same reason.
- Do not hand-edit `crates/ide-db/src/generated/lints.rs` or anything else `xtask codegen` writes. `crates/intern/src/symbol/symbols.rs` is hand-written `define_symbols!`; there is no symbols generator. Do not set `UPDATE_XFLAGS=1` unless `xtask/src/flags.rs` grammar changes. It should not.

## Measurements this plan is based on

- rust-analyzer `03fcb77246`, binary `target/release/rust-analyzer`. `Failed to create perf counter: Permission denied` is non-fatal.
- amaru-gce `diagnostics` log `/tmp/ra-gce-diag.txt`, `analysis-stats` CSV `/tmp/ra-gce-stats.csv`. Pure-stage CSV rows are lines 421–463 of that file (43 rows).
- Isolated crates under `repros/`, each with `cargo check --offline` and `analysis-stats` / `diagnostics` from that binary. Toolchain file `nightly-2026-09-04` so the sysroot macro is `direct_const_arg`, matching the branch. The 2026-09-28 sysroot that the rust-analyzer checkout itself uses already has `gca`.
- `diagnostics` `LineCol` is 0-based. `analysis-stats` CSV lines are 1-based (`line_index.line_col` plus one in `location_csv_expr`). `--only` is an exact function name or full path, not a prefix. `-vv` is not accepted; the flag is `-v -v`.
