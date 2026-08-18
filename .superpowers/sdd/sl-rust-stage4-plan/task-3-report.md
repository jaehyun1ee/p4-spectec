# Task 3 report: Port the canonical SL AST

Status: DONE

Commit: `feat(rust): port canonical SL AST`

## Result

Added the serialization-independent canonical `lang::sl` AST and its module
export. `sl::ast` preserves the SL-only parameter, holding-condition, guard,
instruction, relation/function, and definition forms, while reusing canonical
IL AST aliases for every shared representation. No codec, parser, pass,
evaluator, equality/free/print helper, runtime behavior, serde dependency, or
wire dependency was added.

## TDD evidence

The focused construction/shape test was added before production code. The
test uses hand-derived exhaustive tag sequences for every SL-only enum family,
constructs all relation/function tuple aliases, and verifies `INote` plus the
boxed recursive `DebugI` child.

RED:

```text
$ rtk cargo test --locked --test canonical_ast sl_ast_represent_every_sl_only_variant_and_tuple_alias
cargo test: 1 errors, 0 warnings (1 crates)
error[E0432]: unresolved import `p4spec_rust::lang::sl`
  --> tests/canonical_ast.rs:14:13
14 |         il, sl,
   |             ^^ no `sl` in `lang`
```

GREEN:

```text
$ rtk cargo test --locked --test canonical_ast sl_ast_represent_every_sl_only_variant_and_tuple_alias
cargo test: 1 passed, 5 filtered out (1 suite, 0.00s)
```

## Source-order and comment audit

- Reviewed all 233 lines of `p4spec/lib/lang/sl/ast.ml`.
- Preserved declaration order: shared aliases, `Param`, type/arguments,
  dangling/holding/case analysis, instructions, hints, relations, functions,
  definitions, and spec.
- Preserved constructor order for `ParamKind` (`ExpP`, `DefP`), `HoldCase`
  (`BothH`, `HoldH`, `NotHoldH`), `Guard` (`BoolG` through `MemG`),
  `InstrKind` (branching through debugging), and `DefKind`.
- Carried meaningful section, grammar, and instruction/definition constructor
  group comments; omitted only OCaml serialization attributes.
- Reused `il::ast` aliases for the shared SL/IL vocabulary. `AtomKind` points
  to the same domain atom representation because IL imports that name privately.
- `Box` is used only for the direct `DebugI -> Instr` recursive edge;
  recursive collection edges remain inline.

## Files

- `p4spec-rust/src/lang/mod.rs`
- `p4spec-rust/src/lang/sl/mod.rs`
- `p4spec-rust/src/lang/sl/ast.rs`
- `p4spec-rust/tests/canonical_ast.rs`
- `.superpowers/sdd/sl-rust-stage4-plan/task-3-report.md`

## Final verification

```text
$ rtk cargo fmt --check
exit 0

$ rtk cargo clippy --locked --all-targets -- -D warnings
exit 0
cargo clippy: No issues found

$ rtk cargo test --locked
exit 0
cargo test: 31 passed (6 suites, 0.01s)

$ rtk rg -n 'serde|serde_json|crate::wire' p4spec-rust/src/lang
exit 1
<no matches>

$ rtk git diff --check
exit 0
```

## Self-review

- Compared every source declaration, alias, record, and constructor against
  `sl/ast.ml`; none is omitted or reordered.
- Confirmed exact phrase boundaries for `Param`, `Instr`, and `Def`, and the
  `i64` mapping for OCaml `iid : int`, consistent with canonical input hints.
- Confirmed the test detects omission of each required SL-only variant family,
  every relation/function tuple alias, and nested instruction note shape.
- Confirmed canonical `src/lang` contains no serde, `serde_json`, or
  `crate::wire` references and no unrelated files were changed.

## Concerns

None.
