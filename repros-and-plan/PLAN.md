# Plan: rust-analyzer vs amaru typestate (`rk/remove-gce-warnings`)

Investigation only. Nothing in this document has been implemented. No rust-analyzer source was edited, no Amaru source was edited, no pull request was opened.

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

`amaru-protocols` `protocol/miniprotocol.rs:201` is E0308 on the `#[define_opaque]` closure. That file is not in this branch's diff. It is in scope because it is a user of the typestate API (`StageState::LocalIn` is already resolved on both sides of the mismatch).

`analysis-stats --output csv` (this enables `#[cfg(test)]`; `diagnostics` does not) on pure-stage is 43 `type` rows and 0 `mismatch` rows. Grouped by root cause:

**1. Generic const item parameters are not stored (2 rows).** `list.rs:202` is `fn types_eq<{unknown}, {unknown}>() -> bool` inside `const TYPES_EQ<A, B>`. `describe.rs:44` is `fn remainder_ctfe_panic<{unknown}>() -> usize` inside `const REVEAL<Rem: ConstDesc>`. The corresponding `const fn` items are fine. Const-eval of the const items fails with `MirLowerError(NotSupported("monomorphization resulted in errors"))`.

**2. `direct_const_arg` bounds do not select among overlapping impls (3 rows).** `tests.rs:363`, `:370`, `:372` display `In<{unknown}>` as the witness argument of `assert_after` / `describe_after`. A single-impl reduction still infers the witness (repro 01, `use_it` is `assert_sel<u32, Here>`). These three rows should disappear once the bounds evaluate; they are not their own language feature.

**3. Const-argument expression nodes are never given a type (38 rows).** The type that contains the expression is already what rustc computes.

- 8 rows, `describe.rs` lines 241, 245, 249, 253, 257, 261, and two on 265: the string literals `"Send"`, `"Call"`, `"SendAny"`, `"Receive"`, `"Schedule"`, `"External"`, `"Repeat<"`, `">"`.
- 9 rows on `describe.rs:311` (`impl_const_seq!(…)`): one `", "` const argument per recursive expansion.
- 18 rows on `describe.rs:312` (`impl_const_par!(…)`): two `" | "` arguments per level (Par and Choice).
- 1 row, `typestate/effect.rs:294`: the `4` in `type_last_segment::<[foo::Bar; 4]>()`.
- 2 rows, `tests.rs:269`: `TEXT` and `{ TEXT }` in `Remainder<{ TEXT }>`.

Repro 03 measures the same split on small crates: `let _x: StrParam<"Send">` gives `_x: StrParam<"Send">` while `"Send"` is `{unknown}`; `let ops: [u8; _] = [1, 2]` gives `ops: [u8; 2]` while `_` is `{unknown}`; `type_last_segment::<[u8; 4]>()` is `fn type_last_segment<[u8; 4]>() -> &'static str` while `4` is `{unknown}`; `StrParam<{ TEXT }>` is `StrParam<"hi">` while the block and the path are `{unknown}`.

Candidates that are **not** gaps for this code:

- `const_type_name`. A monomorphic `const NAME: &'static str = type_last_segment::<u8>()` is typed `fn type_last_segment<u8>() -> &'static str` and const-eval succeeds (repro 05). The shim is `type_name` in `crates/hir-ty/src/mir/eval/shim.rs`. Failures in Amaru are the generic const item in (1), which never gets as far as a monomorphic `type_name` call.
- `unsized_const_params` / `adt_const_params` for `&'static str`. `struct StrParam<const S: &'static str>` lowers `"Send"` to the const value. The pattern type is `StrParam<"Send">`. `intern_const_ref` already builds a `ValTree` for `&str`. The `valtree.rs` FIXME is for `TyKind::Adt`, which this branch does not put in const-parameter position. `Remainder<const TEXT: &'static str>` is the str case, not an ADT case.
- Associated consts used as const arguments, as values. `StrParam<{ <u8 as Desc>::TEXT }>` is `StrParam<"u8">`, and both the path and the block are `&'static str`, because that path misses `path_to_const` and falls through to anon-const inference. The `{unknown}` in `tests.rs:269` is the local-const path that `path_to_const` accepts, which skips `write_expr_ty`. The const value is still `"hi"` in the reduced crate.
- Inferred array length, as a type. `[u8; _]` in a body is `[u8; 2]`. Only the `_` expression node is untyped, which is piece 3.

`Features::generic_const_exprs` and `Features::generic_const_args` both return `false`, and `anon_const_kind` always returns `AnonConstKind::GCE`. That is a trap for later work. It is not what produces these 43 rows. Direct const arguments must be `UnevaluatedConst` of a `ConstId` with substitutions, not GCE anon consts.

## (b) Missing pieces

| Piece | Why it is needed | Repro | rust-analyzer location | Upstream | Size / risk | Depends on |
| --- | --- | --- | --- | --- | --- | --- |
| 1. Generic parameters of const items | Without them, `TYPES_EQ` / `REVEAL` bodies and their const-eval are wrong. Required before a `direct_const_arg!(TYPES_EQ::<…>)` can evaluate. | `repros/02-generic-const-item` | `ConstSignature` in `crates/hir-def/src/signatures.rs` (`// generic_params: GenericParams` is commented out, commit `aae8e50546`). `GenericParams::with_store` / `with_source_map` return `EMPTY` for `GenericDefId::ConstId` (`crates/hir-def/src/hir/generics.rs`). `ConstId::resolver` already calls `push_generic_params_scope` and the comment says nightly const items have generic params (`crates/hir-def/src/resolver.rs`). | Parse-only: [rust-lang/rust-analyzer#19643](https://github.com/rust-lang/rust-analyzer/pull/19643) (`812a035689`, ChayimFriedman2), closing [#19516](https://github.com/rust-lang/rust-analyzer/issues/19516). No semantic follow-up. `git log -S direct_const_arg` and `--grep generic_const_items` are empty. | Medium / medium. Touches a salsa signature query and every const's param scope. Param bounds (`Rem: ConstDesc`) must be lowered with the same path as functions, or the body still will not see the trait. | none |
| 2. Builtin `direct_const_arg` and `gca` | Stops the empty expansion. That is the hard `macro-error`, the `expected bool, found ()`, and the `expected expression` syntax error. | `repros/01-direct-const-arg` | `register_builtin!` in `crates/hir-expand/src/builtin/fn_macro.rs` (no such name). Lookup is `find_builtin_macro` from `collect_macro_rules` in `crates/hir-def/src/nameres/collector.rs` (~2540), using the `#[rustc_builtin_macro]` name. Misses become `MacroExpander::UnimplementedBuiltIn`. Expansion is `expand_unimplemented_builtin_macro` in `crates/hir-expand/src/lib.rs` (empty tokens + "this built-in macro is not implemented"). Call-site diagnostic: `crates/ide-diagnostics/src/handlers/macro_error.rs`. New names need symbols in `crates/intern/src/symbol/symbols.rs`. | No rust-analyzer issue or PR. rustc: [#163198](https://github.com/rust-lang/rust/pull/163198) renamed the macro to `gca!` (khyperia, merged 2026-09-23). [#163306](https://github.com/rust-lang/rust/pull/163306) renamed the feature gates (merged 2026-09-25) and explicitly did not rename the macro again. | Small / low for the expander. The expander should return the inner token tree unchanged. That alone does not make `If<{ TYPES_EQ::<…> }>: IsFalse` true. | none for the registration. Useless for Amaru until piece 3. |
| 3. Path to a generic const item as `UnevaluatedConst { def, args }` | `path_to_const` returns `ConstHasGenerics` for any `ConstId` that has parameters, then `create_anon_const` builds an anon const. Where-clause anon consts are not allowed to use generic parameters (`ForbidParamsAfterReason::AnonConst`). mGCA is explicitly not implemented. The bool never becomes `true` or `false`, so overlapping `TakeHead` impls do not determine `I`. | `repros/01-direct-const-arg` plus `02` | `path_to_const` and `create_anon_const` in `crates/hir-ty/src/consteval.rs` (~319 and ~354). The `ConstHasGenerics` arm is excluded from the early-success match so it falls through to anon-const creation. `evaluate_const` in `crates/hir-ty/src/next_solver/solver.rs` (~257) already calls `const_eval(const_id, subst)` and then `Const::new_from_allocation`. It turns every error into an error const; the FIXME says rustc returns `None` for `HasGenericsOrInfers`. `Features` in `crates/hir-ty/src/next_solver/interner.rs` (~682) hardcodes both const-generic flags to `false`. `anon_const_kind` (~2180) always returns `GCE`. | No rust-analyzer issue. rustc feature history is the mGCA series (`min_generic_const_args`, tracking issue 132980). On nightly-2026-09-04 the macro attribute is `#[rustc_builtin_macro(direct_const_arg)]` and the feature is `min_generic_const_args` (`library/core/src/marker.rs` ~1082). The doc comment still mentions `type const` and `direct!`; the macro the branch calls is `direct_const_arg`. On the nightly this rust-analyzer was built with (2026-09-28) the attribute is `#[rustc_builtin_macro(gca)]`, the macro is `gca!`, and the feature is `gca_min_const_items`. `type const` was removed by [rust-lang/rust#162517](https://github.com/rust-lang/rust/pull/162517) (merged 2026-09-12), after the Amaru pin. Amaru does not use `type const`, `gca!`, or `#[always_gca]`. rust-analyzer parsed `type const` in [#22046](https://github.com/rust-lang/rust-analyzer/pull/22046), closing [#22038](https://github.com/rust-lang/rust-analyzer/issues/22038). That parse is now dead syntax. | Medium / medium-high. Substituting the path's generic arguments and evaluating must not collapse an evaluation failure to `false`, or `IsFalse` will pick the wrong `TakeHead` impl. Keep these as `ConstId` unevaluated consts. Do not flip `generic_const_exprs()` to `true`. | Piece 1. Piece 2 for the Amaru call sites. A unit test can use the path without the macro. |
| 4. Record types of const-argument expressions | Clears the 38 `{unknown}` rows. Hover on `"Send"`, on `_` in `[T; _]`, and on `{ TEXT }` shows `{unknown}` today even though the surrounding type is right. `expr_ty` returns the error type when the expression was never inserted (`InferenceResult::expr_ty`). | `repros/03-const-arg-expr-types` | `TyLoweringContext::lower_expr_as_const` / `lower_const` in `crates/hir-ty/src/lower.rs` (~453) never writes an expression type. `InferenceContext::create_anon_const` in `crates/hir-ty/src/infer.rs` (~1967) writes the expected type only on the outer expression, and only when the result is not an `AnonConstId`. Type ascriptions and turbofish in bodies go through the lowering context, not that function. Literals that `intern_const_ref` turns into values, and paths that `path_to_const` accepts, are exactly the nodes that stay untyped. | No issue found for this bookkeeping hole. Array-length search on rust-lang/rust-analyzer returned nothing relevant. | Small / low. Write the expected const type onto the expression (and, for a block whose tail was unwrapped, onto the block and the tail). Do not re-infer the expression from scratch. Risk is writing a type that later inference would have refined; for these literals and const paths the expected type is the answer. | none |
| 5. `#[define_opaque]` for free TAIT | Removes the false E0308 in `amaru-protocols`. Not required for the 43 pure-stage rows. | `repros/04-define-opaque` | Registered as a no-op: `crates/hir-expand/src/builtin/attr_macro.rs` `(define_opaque, DefineOpaque) => dummy_attr_expand`, added in [#21183](https://github.com/rust-lang/rust-analyzer/pull/21183) (`a4612ce527`). ChayimFriedman2: "I don't like that we have to register every builtin macro even if we don't need it, but that's what we got." `crates/hir-ty/src/opaques.rs`: FIXME line 80 "Collect opaques from `#[define_opaque]`". `tait_defining_bodies` returns `Vec::new()` for free aliases (FIXME line 208). Ignored test `crates/hir-ty/src/tests/opaque_types.rs::type_alias_impl_trait_simple`: "TAIT support was removed, need to rework it to work with `#[define_opaque]`". `InferCtxt::can_define_opaque_ty` is in `crates/hir-ty/src/next_solver/infer/mod.rs` (~906). | [#21474](https://github.com/rust-lang/rust-analyzer/issues/21474) closed 2026-01-15. flodiebold: TAIT is not supported; disable the `type-mismatch` diagnostic. ChayimFriedman2: ATPIT and RPIT are supported, "it's not clear if we can support TAIT at all in its current shape." Older [#13824](https://github.com/rust-lang/rust-analyzer/issues/13824) was closed with "this works now"; that predates the next-solver removal. | Large / high. This is a next-solver rework of something that was removed, not a missing attribute. | none. Do not block pieces 1–4 on it. |
| Not a piece: `const_type_name` | Already works for monomorphic consts. | `repros/05-const-type-name` | `crates/hir-ty/src/mir/eval/shim.rs` `type_name` arm (~783). | No rust-analyzer hit for `const_type_name`. | none | — |
| Not a piece: `&'static str` const params, `adt_const_params` for this branch | Values already lower. The ADT valtree hole is real and unused here. | covered by `repros/03-const-arg-expr-types` (`StrParam<"Send">`) | `intern_const_ref` in `consteval.rs` (~123) handles `&str`. `crates/hir-ty/src/next_solver/consts/valtree.rs` (~221) returns an error const for `TyKind::Adt` with FIXME "requires `adt_const_params`". Same string in `mir/eval.rs` (~2071). FIXME added by [#19095](https://github.com/rust-lang/rust-analyzer/pull/19095). | No open rust-analyzer issue on `adt_const_params` or `unsized_const_params`. | none for Amaru. A later ADT-const-param effort is large and should not be started for this branch. | — |

## (c) Order of work

Dependency order, which is also the suggested PR order:

1. **Const-item generic parameters**, or nothing that evaluates `TYPES_EQ::<A, B>` can monomorphize.
2. **Register `direct_const_arg` and `gca`**, identity expansion of the argument token tree. Independent of (1), but Amaru stays red until (3).
3. **`path_to_const` builds `UnevaluatedConst` with the path's generic arguments, and that const evaluates under the solver.** Needs (1). Needs (2) at the Amaru call sites.
4. **Write expression types for const arguments in bodies.** Independent. Can land any time. It does not fix a hard error.
5. **`define_opaque` / free TAIT.** Independent, and only if Roland still wants it after the maintainer comments in (e).

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
- `Const eval for TYPES_EQ` / `REVEAL` may still fail until PR 3, because nothing monomorphizes them yet. Do not treat a remaining generic-eval failure as a regression of this PR.

### PR 2 — builtins `direct_const_arg` and `gca`

Test: `crates/hir-def/src/macro_expansion_tests/builtin_fn_macro.rs`. Expansion of `direct_const_arg!(TYPES_EQ::<A, B>)` and of `gca!(TYPES_EQ::<A, B>)` is the inner tokens, not empty. Register both names. The builtin name comes from `#[rustc_builtin_macro(...)]` in the sysroot the IDE is analyzing, so a single name does not cover both nightly-2026-09-04 and current nightly.

Acceptance on amaru-gce `diagnostics`:

- The four `macro-error` rows and the session.rs syntax error are gone.
- E0308 may remain, with a different found-type, until PR 3. Say so in the PR. Do not "fix" it by making the macro expand to `false`.

### PR 3 — generic const items in const-argument position

Test: `crates/hir-ty/src/tests/traits.rs` (or `regression.rs` if that is where where-clause tests live). Two cases, both using the path directly and, in a second test, `direct_const_arg!`:

- `types_eq` returns `false` for distinct types, `If<{ TYPES_EQ::<u8, u16> }>: IsFalse` holds, a `Select<E, Here>` impl with that bound is chosen, and a call `assert_sel::<u32, _>()` infers `Here`.
- A second const that returns `true` for identical types must **not** satisfy `IsFalse`. This is the wrong-impl guard.

Const-eval test next to `crates/hir-ty/src/mir/eval/tests.rs` or `consteval/tests`: `const EQ<A, B>: bool = …` instantiated at `u8, u8` and `u8, u16` yields the two bools. Use `size_of` or `type_name`; both already evaluate when the parameters are real (repro 05).

Implementation notes, deliberately small:

- In `path_to_const`, when the `ConstId` has generic parameters, lower the path's generic arguments and return `Const::new_unevaluated(UnevaluatedConst { def, args })`. Do not return `ConstHasGenerics` for that case.
- Do not wrap that path in an anon const. `create_anon_const`'s comment is right that mGCA anon consts are unimplemented; this path should not need them.
- Leave `Features::generic_const_exprs` and `generic_const_args` false. Leave `anon_const_kind` alone unless a test proves the solver refuses a `ConstId` unevaluated const because of it.
- `evaluate_const` already evaluates `ConstId` with substitutions. After PR 1 the substitutions are real types. If evaluation fails, do not invent `false`. Matching rustc's `None` on `HasGenericsOrInfers` (the existing FIXME in `solver.rs`) matters here: an error const in the `If<…>` argument is worse than an unknown.

Acceptance on amaru-gce:

- `diagnostics` has zero error-severity diagnostics in `amaru-pure-stage` (the four call sites).
- `analysis-stats` CSV for pure-stage no longer has the three `In<{unknown}>` rows in `tests.rs`.
- The 38 const-argument expression rows are still present until PR 4. Do not block this PR on them.

### PR 4 — expression types for const arguments

Test: `check_types` in `crates/hir-ty/src/tests/regression.rs`, with the `//^` marker on the expression that is unknown today:

```rust
fn str_in_body() {
    let _x: StrParam<"Send"> = StrParam;
      //^ StrParam<&str> or whatever display the suite uses for StrParam<"Send">
}
```

The marker has to sit on `"Send"`, on `1`, on `_`, and on `TEXT` in `{ TEXT }`, not only on the binding. Bindings already have the right type, so a test that only checks `_x` stays green and proves nothing. Copy the expected display strings from a `check_types` failure rather than guessing them.

Acceptance on amaru-gce `analysis-stats --output csv` filtered to `amaru-pure-stage`: zero `type` rows. Together with PR 3, zero error-severity `diagnostics` in pure-stage. `??ty` for the crate goes from 43 to 0.

### PR 5 — `define_opaque`, only with an explicit yes

Test: replace or un-ignore `type_alias_impl_trait_simple` in `crates/hir-ty/src/tests/opaque_types.rs`, written in the current style (`#[define_opaque(Alias)]` on the defining function, not a `static` define-use). Add the `Mini<S>` shape from repro 04 as a second test so the associated type in the opaque signature does not regress.

Acceptance: repro 04 `diagnostics` exit 0, and the `miniprotocol.rs:201` E0308 is gone from an amaru-gce `diagnostics` run. Pure-stage numbers must stay at the PR 4 baseline.

The ignored-test comment says TAIT support was removed. Budget this as a next-solver change in `opaques.rs` (`opaque_types_defined_by`, `tait_defining_bodies`) and `can_define_opaque_ty`, not as an attribute-macro tweak. `dummy_attr_expand` can stay the expander; the attribute's job is to name the aliases, which rustc also treats as a builtin with an empty body.

## (d) Open questions for Roland

1. Is PR 5 in scope? ChayimFriedman2 wrote on [#21474](https://github.com/rust-lang/rust-analyzer/issues/21474) that it is not clear rust-analyzer can support TAIT in its current shape, and the in-tree test says support was removed for the next solver. The protocols error is real and reproduced. It is also the piece most likely to be rejected, and it does not unblock pure-stage.
2. The 38 `{unknown}` rows are expression nodes. The types users see on the bindings (`StrParam<"Send">`, `[u8; 2]`, `Remainder`'s constructed string) are already right, except the three witness rows. Is "CSV `type` rows in pure-stage go to zero" the bar, or is it "no error diagnostic, and the witness infers"?
3. Identity-expanding the macro and then accepting **any** path to a generic const item (not only one that came out of the macro) is more permissive than nightly-2026-09-04, which requires `direct_const_arg!` unless a macroless feature is on. It matches where rustc is going (`gca_macroless_args` in [#163306](https://github.com/rust-lang/rust/pull/163306)). Should the lowering be gated on the macro, or is the permissive IDE behavior what you want?
4. Both builtin names? The Amaru pin is `direct_const_arg` / `min_generic_const_args`. Nightly after 2026-09-23 is `gca` / `gca_min_const_items`. Implementing only the name in today's sysroot will break the other one. No further rename of `gca!` showed up after #163198; #163306 says the macro rename is done.
5. amaru-gce `diagnostics` takes about 160s and `analysis-stats` about 80s, and the CLI exit 1 prints an anyhow backtrace whenever any crate in the workspace has an error (not only pure-stage). Fine as a local acceptance check. Not something to wire into rust-analyzer CI.

## (e) Risks and what maintainers may object to

- **TAIT.** The current stated position is "not supported" and "support was removed". A PR that reintroduces free TAIT will be asked why the next-solver removal should be reversed. ATPIT already works and is not this bug. flodiebold's suggested workaround is disabling the `type-mismatch` diagnostic, which hides the error rather than inferring the closure.
- **Wrong bool.** `TakeHead` has overlapping impls distinguished by `If<{ TYPES_EQ::<…> }>: IsFalse`. Evaluating the const as `false` on failure, or letting an error const satisfy `IsFalse`, silently picks an impl. The true and false tests in PR 3 are there for that reason. `evaluate_const` currently maps all failures to an error const (FIXME against rustc's `None`).
- **Do not enable `generic_const_exprs`.** `Features::generic_const_exprs()` and `generic_const_args()` return `false` on purpose, and `anon_const_kind` is hardcoded to `GCE` with a FIXME. Turning the flags on to make where-clause anon consts see generic parameters would take a much larger surface (the mGCA comment in `create_anon_const`) than Amaru needs. The needed representation is an unevaluated const item with substitutions.
- **Builtin registry.** Every `#[rustc_builtin_macro]` name must be registered or it becomes an error at every call. ChayimFriedman2 already complained about that on the `define_opaque` PR. Adding `direct_const_arg` and `gca` is the same pattern. Skipping one of the two names will be user-visible the next time the sysroot moves.
- **`ConstSignature` salsa change.** Adding `generic_params` changes the tracked signature of every const. Cycles are plausible if a const parameter's type or default mentions the const. Amaru's const items do not do that (`TYPES_EQ<A, B>`, `REVEAL<Rem: ConstDesc>`). A test with a const-parameter default that refers to an earlier parameter is worth having so this does not only get exercised by Amaru.
- **`adt_const_params`.** There is a real FIXME, and a reviewer who sees `&'static str` const parameters may ask for the ADT work in the same PR. The measurements say no. `intern_const_ref` already handles `&str`. Bundling ADT valtrees makes the PR large and does not move the amaru-gce numbers.
- **`type const`.** rust-analyzer grew a parser for it ([#22046](https://github.com/rust-lang/rust-analyzer/pull/22046)). rustc deleted the syntax in [#162517](https://github.com/rust-lang/rust/pull/162517). Do not build the const-item work on `type const`. The Amaru items are `const NAME<T>: Ty = …`.
- **Permissive const paths.** Accepting `If<{ TYPES_EQ::<u8, u16> }>` without the macro may be called a false negative against the feature gate. The alternative, threading a "this expression came from `direct_const_arg`" flag through lowering, is more code and still has to live next to the rename to `gca`. Question 3 is the place that gets decided.
- **Expression-type PR looks like a no-op.** `check_types` on the binding stays green today. Reviewers will want the marker on the literal. Otherwise PR 4 will not be believed, and the 38 rows will remain.

## Measurements this plan is based on

- rust-analyzer `03fcb77246`, binary `target/release/rust-analyzer`. `Failed to create perf counter: Permission denied` is non-fatal.
- amaru-gce `diagnostics` log `/tmp/ra-gce-diag.txt`, `analysis-stats` CSV `/tmp/ra-gce-stats.csv`. Pure-stage CSV rows are lines 421–463 of that file (43 rows).
- Isolated crates under `repros/`, each with `cargo check --offline` and `analysis-stats` / `diagnostics` from that binary. Toolchain file `nightly-2026-09-04` so the sysroot macro is `direct_const_arg`, matching the branch. The 2026-09-28 sysroot that the rust-analyzer checkout itself uses already has `gca`.
- `diagnostics` `LineCol` is 0-based. `analysis-stats` CSV lines are 1-based (`line_index.line_col` plus one in `location_csv_expr`). `--only` is an exact function name or full path, not a prefix. `-vv` is not accepted; the flag is `-v -v`.
