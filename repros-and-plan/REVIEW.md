# Review: generic const items (four pieces)

Reviewed as an upstream rust-analyzer change, against `master` `03fcb77246`. The four branches were not modified. Design context is `repros-and-plan/PLAN.md` on `rk/repros-and-plan` (`f24d9a2c68`). Goal: stop false errors and unknown types in `amaru-pure-stage` (Amaru checkout `/home/andrew/work/amaru-gce`, `nightly-2026-09-04`). Roland's decisions stand: accept generic const item paths permissively, register both macro names, `define_opaque` / TAIT out of scope, no rustc changes. The feature-gate alias is the decision this review rejects as implemented.

| Piece | Branch | Tip | Verdict |
| --- | --- | --- | --- |
| 1 | `rk/piece-1-const-item-generics` | `73432735eb` | **Ready** |
| 2 | `rk/piece-2-gca-builtins` | `9928cf4b4f` | **Not ready** |
| 3 | `rk/piece-3-const-path` | `9e1ca7ae7e` (contains piece 1) | **Not ready** |
| 4 | `rk/piece-4-const-arg-expr-types` | `4d65775972` | **Ready** |

## Prioritized fix list

1. **Piece 2, blocking.** Do not insert a renamed or removed feature name into `UnstableFeatures::all`. Split the alias out of this PR, or confine it to `is_enabled` and say outright that rustc removed the old names. Land the two identity builtins on their own if the alias needs another round.
2. **Piece 3, blocking.** Do not build an `UnevaluatedConst` whose parent generic arguments are error types. If the const's parent has parameters, return `None` from `explicit_args_for_generic_const` (and do not record those args for MIR) so the old `ConstHasGenerics` fallthrough stays. Add a test that an associated const which declares its own parameters, inside a generic impl, does not change behaviour.
3. **Piece 1.** Replace `const_param_default_mentions_earlier_type_param` with a test that fails when `T` is not in scope inside the default. The current marker sits on `N`, whose type is written `usize`.
4. **Piece 3.** Add a const whose body cannot evaluate, and assert that `If<{ THAT }>: IsFalse` does **not** select. `evaluate_const` still maps every eval error to an error const, and this PR puts that const on the impl-selection path.
5. Nits, in any order: return `&'static Symbol` from the alias helper; drop the `#163306` sentence or rewrite it without a ticket id and without calling the two names the same gate; mention the alias in the piece 2 subject only if it stays; rename `generic_const_path_through_identity_macro` (it expands a `macro_rules!`, not `direct_const_arg!` / `gca!`).
6. **Piece 4.** Skip `note_const_arg_expr_ty` when `create_anon_const` returns `Err`. Also record the block expression `{ TEXT }`, not only its tail.
7. Merge order: piece 1, then piece 3 stacked on it. Piece 2 any time after the alias fix. Piece 4 any time, rebased onto piece 3 because both edit `lower_expr_as_const`.

## How this was reviewed

Style and layering checked against `docs/book/src/contributing/style.md`, `architecture.md`, and `testing.md`: syntax stays the API boundary, hir-def / hir-ty are incremental and not a public API, fixtures use minicore and `check_*` / `expect`, comments are sentences, no panic on user code, no new `hir` facade API.

Each piece was checked out detached under `/home/andrew/work/ra-review-wt/p{1,2,3,4}` with its own `CARGO_TARGET_DIR` at `/home/andrew/work/ra-review-targets/p{1,2,3,4}`. Those directories were seeded with `cp -a` from the existing per-branch `target` dirs at the same commits (copies, not hardlinks). Piece 3 and 4 seeds had been built with nightly; `cargo +stable` invalidated them and rebuilt. Stable is rustc 1.98.0 (`88d9e12ae` 2026-08-18), which is what CI's `fmt` and `clippy` jobs use.

On every piece, all of these exited 0:

- `cargo +stable fmt --all -- --check`
- `cargo +stable xtask tidy`
- `cargo +stable codegen --check`
- `cargo +stable test -p hir-def -p hir-expand -p hir-ty -p ide-diagnostics -p ide -p ide-db -p intern`
- `cargo +stable clippy --all-targets -- -D clippy::disallowed_macros -D clippy::dbg_macro -D clippy::todo -D clippy::print_stdout -D clippy::print_stderr`, with `RUSTFLAGS="-D warnings -W unreachable-pub --cfg no_salsa_async_drops"` (the CI workflow env)

No test failed, so there is nothing to compare against `master`. Ignored counts are pre-existing (`hir-def` 1, `hir-ty` 3, `ide-diagnostics` 1). Passed counts: `hir-def` 499 (501 on piece 2), `hir-expand` 33, `hir-ty` 1049 / 1046 / 1053 / 1050 on pieces 1–4, `ide` 1340, `ide-db` 196, `ide-diagnostics` 738, `intern` 4. Clippy did compile `hir-def`, `hir-expand`, `hir-ty`, and `intern`. Build scripts printed `Could not find .git/HEAD from manifest dir` because a worktree's `.git` is a file. Exit code was still 0.

Not run, and not installed: `cargo nextest`, `cargo machete`, `cargo-miri` (no toolchain has the miri component). The workspace-wide nextest job from `ci.yaml` was not run. `cargo rustc --version` was a bad invocation in the runner script; the real commands were `cargo +stable`.

Commits match the local convention: one `fix:` commit each, user-facing subject, body, `Signed-off-by`, no `@` mentions. Piece 3's branch also contains piece 1's commit. Opening piece 3 against `master` publishes both diffs.

## rustc names (`#163198`, `#163306`)

Local sysroot and a small `rustc` experiment, not a patch to rust-lang/rust:

- `nightly-2026-09-04` `library/core/src/marker.rs`: `#[rustc_builtin_macro(direct_const_arg)]`, `#![unstable(feature = "min_generic_const_args")]`, `macro_rules! direct_const_arg`.
- Current nightly (`1.101.0-nightly`, `c1070d693` 2026-09-28): `#[rustc_builtin_macro(gca)]`, feature `gca_min_const_items`, `macro_rules! gca`.
- Both macros are `$($arg:tt)* => /* compiler built-in */`.

`#163198` (merged 2026-09-23) renamed the macro `direct_const_arg!` to `gca!`. `#163306` (merged 2026-09-25) renamed gates and **removed** the old names (`E0557`, "renamed to …"). It did not keep the old name as an alias that still enables the feature.

| Old name | New name | On `nightly-2026-09-04` | On current nightly |
| --- | --- | --- | --- |
| `min_generic_const_args` | `gca_min_const_items` | incomplete warning (accepted) | `E0557` removed |
| `generic_const_args` | `gca_const_items` | incomplete warning | `E0557` removed |
| `macroless_generic_const_args` | `gca_macroless_args` | incomplete warning | `E0557` removed |
| `macroless_const_item_generic_const_args` | `gca_macroless_items` | `E0635` unknown | `E0557` removed |

The new `gca_*` names are unknown (`E0635`) on the Amaru pin and incomplete-warnings on current nightly. `generic_const_items`, `const_type_name`, `unsized_const_params`, `adt_const_params`, and `generic_const_exprs` were not part of that rename. The fourth pair did not exist yet on `nightly-2026-09-04`, so it is not an Amaru-pin compatibility alias.

Registering **both macro names** matches the two sysroots. Treating the eight feature spellings as one gate does not match either compiler.

## Piece 1 — generic params on const items

**Verdict: ready.**

`ConstSignature` gains `generic_params`, filled by `lower_const` (`crates/hir-def/src/expr_store/lower.rs:326`). One `ExprCollector` lowers the param list, the where-clause, and the const's type, which is the same shape as `lower_function` / `lower_type_alias`. `GenericParams::with_store` and `with_source_map` return that field for `ConstId` (`crates/hir-def/src/hir/generics.rs:230`, `:274`) instead of `EMPTY`. `ConstSignature::with_source_map` (`crates/hir-def/src/signatures.rs:349`) is the existing tracked query; this adds a field to a tracked value that `GenericParams::of` already depended on. No new salsa query, no new `hir` facade API. `hir-ty`'s `parent_generic_def` already walks `ConstId`'s container (`crates/hir-ty/src/generics.rs:392`), and `generic_predicates` / `generic_defaults` already have `cycle_result`, so bounds and defaults flow once the params exist. The resolver comment that nightly consts can have generic params was already there; this stores what it was pushing.

`pub generic_params` matches the other signature structs. Naming (`const_`) follows the keyword-mangling rule. No ticket comments. Tests are `check_types` fixtures with `minicore: size_of` where `size_of` is used (`crates/hir-ty/src/tests/regression.rs:3221`, `:3236`, `:3249`). `ide` stayed at 1340 with no snapshot edits, which is the right amount of churn: nothing in `ide` changed.

### Blocking

None.

### Non-blocking

- `const_param_default_mentions_earlier_type_param` (`regression.rs:3249`) does not fail if the default `{ size_of::<T>() }` cannot see `T`. The marker is on `N`, and `N` is written `: usize`. Assert `check_number` of `WITH_DEFAULT::<u8>` (or a marker on the default expression) so a resolver bug goes red.
- `const_item_param_bound` (`regression.rs:3236`) is a real positive test: `Rem: Trait` is an inline bound and `Rem::ASSOC` types as `usize`. It does not cover a separate where-clause, a lifetime param, or a const param on the const item.

### Nits

None that I would mention in review.

### Test gaps

Lifetime parameters, a const parameter on the const item, a where-clause written separately from the inline bound, an associated const that declares its own parameters, a macro-expanded `const` item, and an incomplete `const FOO<` while typing. Goto / hover / find-refs have no fixture; the unchanged `ide` snapshots do not prove those paths, they only prove no existing fixture broke.

### Risks

Low. Typing in a function body still does not invalidate this signature. A const with generics now invalidates `GenericParams::of` consumers, which is the point. Statics correctly stay on `EMPTY`.

## Piece 2 — builtin macros and feature-name alias

**Verdict: not ready** as one PR. The macro half is ready. The alias half should be dropped or rewritten before upload.

### Blocking

`enable` inserts the other spelling into `UnstableFeatures::all` (`crates/hir-def/src/unstable_features.rs:73`). The module docs (`unstable_features.rs:1`) say `all` is the set of feature names the crate wrote, mostly for external consumers, and `iter` (`:36`) yields that set. After `#![feature(min_generic_const_args)]`, `all` also contains `gca_min_const_items`, a name that crate did not write and that `nightly-2026-09-04` rejects as unknown. After `#![feature(gca_min_const_items)]`, `all` also contains `min_generic_const_args`, which current nightly rejects with `E0557`.

`is_enabled` (`:30`) then reports both names. That is false for both compilers this fork cares about: the Amaru pin does not know the `gca_*` gates, and current nightly does not accept the old names. Nothing in pieces 1–4 calls `is_unstable_feature_enabled` for these eight symbols. `Features::generic_const_args` / `generic_const_exprs` in `hir-ty` stay false. The alias is dead for Amaru and locks the wrong model into `generic_const_arg_feature_names_alias_each_other` (`:136`), which asserts both directions.

`#163306` removed the old names. It did not alias them. A maintainer can reject this in principle, and should.

How to frame it if a lookup stays at all: rust-analyzer has one feature table for every sysroot. Between 2026-09-04 and `#163306` the same gate has two spellings, and neither compiler accepts both. Keep `all` equal to the names written in source. If `is_enabled` must answer for a neighbour spelling, do that only inside `is_enabled`, and say that it is a cross-toolchain lookup rather than "the crate enabled this." Prefer dropping the alias from this PR. `macroless_const_item_generic_const_args` was `E0635` on the Amaru pin, so that pair is not even a rename of a gate Amaru can write.

The identity macros are a different matter and are fine to defend: the sysroot attribute changed from `direct_const_arg` to `gca`, both definitions are a tt pass-through implemented in the compiler, and rust-analyzer has to expand the attribute it actually sees. `identity_expand` (`crates/hir-expand/src/builtin/fn_macro.rs:1005`) clones the token tree and sets `DelimiterKind::Invisible`, the same delimiter trick as `pattern_type_expand` (`:1018`). That is what makes `{ gca!(PATH) }` lower as `PATH` rather than `(PATH)`. The expansion test (`crates/hir-def/src/macro_expansion_tests/builtin_fn_macro.rs:664`) expects `TYPES_EQ::<A, B>` with no extra parens, and it can fail. Registering the name the way `define_opaque` is registered does **not** implement the feature; here the expansion is load-bearing and should be described as "pass the tokens through," not as "generic const args work."

### Non-blocking

- Split the PR if the alias is rewritten: builtins in one commit, the lookup in another. A maintainer will ask for that split anyway. The subject already describes only the macros; the body mentions the alias. If the alias stays, the subject has to say so.
- `gca_feature_alias` (`unstable_features.rs:82`) clones `Symbol`. These are preinterned; clone is a refcount bump, not a deep copy. Return `&'static Symbol`.
- The `is_enabled` alias check is redundant once `enable` inserts both names, and harmful if `enable` stops inserting them without updating `is_enabled`. Pick one site.

### Nits

- `unstable_features.rs:82` cites `#163306` and says either name "is the same gate." Comments in this tree are sentences about the code, and the sentence is wrong. Say which names are recognised and what `is_enabled` returns.
- New symbols in `intern/src/symbol/symbols.rs` sit near `min_specialization` rather than in one global alphabetical list. `intern` tests passed (4). Leave them unless tidy or the intern convention in that file says otherwise; they are grouped with the feature they neighbour.

### Test gaps

The feature test matches `nameres` collector style (`TestDB`, `crate_def_map`, `is_enabled`) and can fail. It never asserts that `iter()` / `all` contains only the spelling from source. The macro test does not feed the expanded path into const lowering; that combination waits until piece 2 and piece 3 are both present. Piece 2 alone removes the "built-in macro is not implemented" diagnostic. It does not make `If<{ ... }>: IsFalse` select. Do not describe the macro PR as turning the bound true.

### Risks

Shipping the alias means external consumers of `UnstableFeatures::iter` (the docs call this out) will see gates the crate did not enable. Analysis behaviour of pieces 1, 3, and 4 does not depend on it. Identity expansion of an unstable macro is appropriate because the tokens are already parsed as a macro call; the risk is over-claiming, not a wrong expansion. `{ gca!(PATH) }` with the invisible delimiter is the behaviour the expansion test locks in.

## Piece 3 — `path_to_const` for generic const items

**Verdict: not ready.** The free-const path Amaru needs is correct and tested. The parent-argument path is not, and the comments claim it is.

Piece 3 is stacked on piece 1 (`9e1ca7ae7e` is piece 1 plus one commit). A PR against `master` should wait until piece 1 lands, or be opened as stacked and reviewed as the second commit only.

### Blocking

`explicit_args_for_generic_const` (`crates/hir-ty/src/lower.rs:460`) returns `Some` whenever the path **fully** resolves to a `ConstId` whose own `GenericParams` are non-empty (`:474`). It then calls `substs_from_path(..., infer_args: true, Span::Dummy)` (`:488`).

`parent_arg` in that lowering (`crates/hir-ty/src/lower/path.rs:811`) fills every parent parameter with an error type, an error const, or an error region. The body path in `infer/path.rs` repairs that afterwards: `resolve_value_path` splices the real parent substitution with `GenericArgs::fill_rest` (`crates/hir-ty/src/infer/path.rs:143`). `generic_args_for_const_expr` does not.

So a `ConstId` that resolves fully and has **both** parent parameters and its own parameters becomes an `UnevaluatedConst` whose parent arguments are errors (`consteval.rs:334`). That is worse than the previous `ConstHasGenerics` fallthrough. An error const on `If<{ ... }>: IsFalse` is the wrong-bool hazard: `evaluate_const` (`crates/hir-ty/src/next_solver/solver.rs:279`) still maps every eval error to an error const, with the existing FIXME that rustc returns `None` for `HasGenericsOrInfers`.

The comment at `lower.rs:472` says parent generics keep the old fallthrough. That is true only when the const's own `GenericParams` are empty. The same overclaim is at `infer/path.rs:53`: `write_assoc_resolution` runs for every const with non-empty own params. In a function body the args have usually been through `fill_rest` already, so the MIR write is less wrong than the where-clause path; the comment is still broader than the code.

`Type::CONST` and `<T as Trait>::CONST` stay `ResolveValueResult::Partial`, and `resolve_path_in_value_ns_fully` returns `None` (`crates/hir-def/src/resolver.rs:520`). Those paths are unchanged. The hole is the fully resolved const: a bare name in scope inside a generic impl, or any other `ConstId` whose parent chain is non-empty. Fix by returning `None` from `explicit_args_for_generic_const` when `generics(db, const_id.into()).parent()` has parameters, and skip the new `write_assoc_resolution` arm in that same case (or only write it when the parent substitution was actually applied). Add a fixture that would go red if that guard is deleted.

Permissive acceptance of `const FOO<T>` without consulting `generic_const_args` is what Roland asked for, and it is defensible: the grammar already parses the params, piece 1 stores them, and `hir-ty`'s `Features` flags for this nightly feature are still false. Frame the PR as "a path to a const item that declares parameters is an unevaluated const with those arguments," not as "the feature gate is on." A maintainer who wants the gate checked will ask; point at the false `Features` flags and at Amaru, and do not pretend the gate was consulted.

### Non-blocking

- Diagnostics are swallowed (`lower.rs:477`, empty callback, `TypeRefId` raw index 0, `Span::Dummy`). The dummy id matches `at_path_forget_diagnostics`. Say so in one sentence. Omitted turbofish becomes inference variables when a table exists, and error types when `next_ty_var` has nowhere to put them, with no diagnostic. The tests always write `IS_U8::<u16>`.
- `evaluate_const`'s error-const mapping is pre-existing and now sits on the selection path. `generic_const_path_true_does_not_select_impl` (`regression.rs:3285`) guards the successful-eval direction: on `master` the witness is already `{unknown}`, and the test goes red if `IS_U8::<u8>` is treated as `false` and selects `Here`. It does **not** guard a failed eval. Add a body that cannot evaluate and assert the witness stays `{unknown}`.
- `generic_const_path_through_identity_macro` (`regression.rs:3310`) uses `macro_rules! pass`, not `direct_const_arg!` / `gca!`. The invisible-delimiter builtin plus `path_to_const` is untested until both pieces are combined. Rename the test. A follow-up fixture with `#[rustc_builtin_macro]` can wait for the stack, but do not claim this test covers piece 2.
- `path.clone()` at `lower.rs:507` is a borrow workaround. Fine; a maintainer may ask for a match that doesn't clone the `Path`.

### Nits

The `#[expect(clippy::manual_map)]` on the `create_var` binding is pre-existing style in this function. Leave it.

### Test gaps

`generic_const_item_evaluates_with_its_args` (`consteval/tests.rs:157`) is the right test: `check_number` 1 for `u8` and 0 for `u16`. `generic_const_path_selects_false_impl` (`regression.rs:3260`) fails on `master` (witness is not `Here`). Keep the true-test beside it; alone, the true-test is green on `master` and does not prove the feature.

Missing: lifetime or const parameters on the const item, a default used from the path, a cyclic const, an incomplete path, and the parent-plus-own-params negative above. No goto / hover / find-refs fixture for the new args.

### Risks

Salsa: `UnevaluatedConst` args are part of the const identity the solver already hashes. A wrong parent subst will stick in the query cache until the const signature changes, which is why the guard belongs in this PR rather than a follow-up. Lowering still runs before inference of the body; signature positions have no inference variables, so a bad subst there becomes an error const immediately. Piece 4 edits the same `lower_expr_as_const`. Rebase, do not merge both against unmodified `master`.

## Piece 4 — expected types on const-argument expressions

**Verdict: ready.**

`note_const_arg_expr_ty` (`crates/hir-ty/src/lower.rs:507`) runs from `lower_expr_as_const` while a body is inferred. No inference variables means signature lowering returns immediately (`:516`), which is correct: there is no `type_of_expr` map there. One empty block is unwrapped, matching `create_anon_const`, and both the block expr and the tail are recorded. `record_expr_ty` (`crates/hir-ty/src/infer/diagnostics.rs:88`) does not overwrite a type inference already stored. `TypeLikeConst::Infer` notes the placeholder before `next_const_var` (`crates/hir-ty/src/lower/path.rs:763`). `record_type_placeholder` uses `or_insert` (`diagnostics.rs:94`). Hover reads `type_of_type_placeholder` from the source analyzer; `check_types` / `check_infer` read `type_of_expr`. The new tests hit the expr map, which is what the 38 unknown-type rows in the plan are.

The four snapshot additions are the array-length expressions that previously had no type. They are justified:

| Snapshot | What the span is |
| --- | --- |
| `tests/patterns.rs:474` `31..32 '2': usize` | the `2` in `let arr: [f64; 2]` inside `infer_pattern_match_arr` |
| `tests/regression/new_solver.rs:339` `27..28 '_': usize` | the **length** `_` in `let foo: [_; _] = [false] as _`. Body `{` is at byte 10; the element `_` is at 24 and is still untyped; `as _` is later and still untyped. `foo` stays `[bool; 1]` |
| `tests/simple.rs:1241` `282..283 '0': usize` | the `0` in `let x: [u8; 0] = []` inside `infer_array` (`simple.rs:1185`). The `[]` literal stays `[u8; 0]` |
| `tests/simple.rs:3717` `178..179 '0': usize` | the `0` in `Box::new([]) as Box<[i32; 0]>` inside `castable_to`. The `[]` stays `[i32; 0]` |

No other snapshot lines moved. The new `check_types` tests (`regression.rs:3221`, `:3234`, `:3246`, `:3259`) put the marker on the expression that used to be unknown (`"Send"`, `1`, `_`, `TEXT`), so they fail if the write is removed. `minicore` is correctly absent: none of them need a lang item beyond what the fixture already is.

### Blocking

None.

### Non-blocking

- `note_const_arg_expr_ty` is called even when `create_anon_const` returns `Err` (`lower.rs:495`, before `unwrap_or` at `:497`). A mistyped literal can hover as the expected type. The plan asked to record the expected type and not re-infer; the pre-existing FIXME still does not report the mismatch. Skip the note on `Err`, or record it only for `Ok`.
- The block node `{ TEXT }` is recorded but `const_arg_block_path_has_expected_type` only marks `TEXT`. Mark the block too so a one-sided write fails the test.
- Only one empty block is unwrapped. `{ let x = 1; x }` leaves the tail untyped. `{{ TEXT }}` types the outer block and the inner block, not `TEXT`. Subexpressions of `{ 1 + 2 }` other than the tail stay untyped. That matches `create_anon_const` and is the right scope for this PR; mention it in the commit body (the body already says "one unwrapped block").

### Nits

`record_expr_ty` on the inference ctx is a default method on `TyLoweringInferVarsCtx` (`lower.rs:204` area) overridden in one place. That is the existing extension point. No new public API.

### Test gaps

A failing literal (`"Send"` where the const param is `usize`) is not asserted. Nested blocks and a block with statements are not asserted. Array length `_` versus element `_` is covered by the `new_solver` snapshot; worth a sentence in the PR so nobody "fixes" the element underscore next.

### Risks

Recording only when `type_of_expr` has no entry avoids fighting inference. Signature lowering is unchanged. Conflict with piece 3 is mechanical: both wrap `create_anon_const` in `lower_expr_as_const`. Piece 4 does not depend on pieces 1–3 for correctness; the unknown-type rows in Amaru are this piece alone.

## What a maintainer would ask to split, rename, or drop

- **Drop or split** the feature alias from piece 2. Keep the two `register_builtin!` lines and `identity_expand`.
- **Do not open piece 3 against `master`** while it contains piece 1. Review it as one commit on top of piece 1.
- **Rebase piece 4 onto piece 3** (or the reverse) before the second of those two merges. No functional split inside piece 4; the snapshot updates belong with the write.
- Piece 1 is one logical change. Do not split the three tests out.
- Nobody should be asked to implement `define_opaque` or to flip `Features::generic_const_args` in these PRs.

## Principle objections, and how to frame the upload

- **Identity builtins for an unstable macro.** Reasonable. rustc's builtin is a token pass-through, the attribute name changed under Amaru's nightly, and invisible delimiters are an existing expander trick. Say that. Do not say generic const arguments are implemented.
- **Alias of removed feature gates.** Not reasonable as written. rustc reports `E0557` and does not enable the new gate. External `UnstableFeatures` consumers would observe a lie. See the blocking note and the table above.
- **Permissive generic const item paths.** Reasonable under Roland's decision, if piece 3 refuses parent substitutions it cannot see. Stable rustc rejects `const FOO<T>`; rust-analyzer already parses it. The PR should say the path is lowered whenever the const declares parameters, and that the feature flag is intentionally not consulted because those `Features` booleans are still false.
