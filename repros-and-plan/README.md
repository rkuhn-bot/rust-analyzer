# rust-analyzer repros for amaru typestate

Investigation only. No rust-analyzer or Amaru source changes.

The code under test is `pragma-org/amaru` branch `rk/remove-gce-warnings`, worktree `/home/andrew/work/amaru-gce`, commit `1f895090`, toolchain `nightly-2026-09-04`. rust-analyzer is `/home/andrew/work/rust-analyzer/target/release/rust-analyzer` at `03fcb77246` (2026-09-27).

The write-up is [PLAN.md](PLAN.md).

| Directory | rustc | rust-analyzer | What it isolates |
| --- | --- | --- | --- |
| [repros/01-direct-const-arg](repros/01-direct-const-arg) | accepts | macro-error, E0308, syntax error | `direct_const_arg!` |
| [repros/02-generic-const-item](repros/02-generic-const-item) | accepts | no diagnostic; `{unknown}` params; const-eval fails | `const X<T>: Ty = …` |
| [repros/03-const-arg-expr-types](repros/03-const-arg-expr-types) | accepts | no diagnostic; const-arg expr nodes are `{unknown}` | expression type map, including `[T; _]` and `&'static str` const args |
| [repros/04-define-opaque](repros/04-define-opaque) | accepts | E0308 | `#[define_opaque]` + free TAIT, including the `amaru-protocols` shape |
| [repros/05-const-type-name](repros/05-const-type-name) | accepts | accepts, const-eval succeeds | control: `const_type_name` is not a gap |

`01` and `02` replace `/home/andrew/work/ra-repro2`. `04` replaces `/home/andrew/work/ra-repro` except the array-length case, which is in `03`. Those older trees were left in place.

Every repro pins `nightly-2026-09-04` in its own `rust-toolchain.toml`. Unset `RUSTUP_TOOLCHAIN` before `cargo` or `rust-analyzer`, or the shell override wins over the file. `diagnostics` prints 0-based `LineCol`. `analysis-stats` CSV lines are 1-based. `analysis-stats` enables `#[cfg(test)]`; `diagnostics` does not.
