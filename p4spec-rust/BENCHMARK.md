# SL interpreter benchmark

## Result

The Rust SL interpreter is faster on the complete positive `Program_inst`
corpus. The geometric-mean speedup is **1.711x**, with a paired bootstrap 95%
confidence interval of **[1.630x, 1.795x]**. The entire interval is above 1,
so the predeclared conclusion is **Rust improvement**.

| Metric | OCaml | Rust | OCaml / Rust |
| --- | ---: | ---: | ---: |
| Programs | 1,267 | 1,267 | - |
| Passing measured evaluations | 3,801 | 3,801 | - |
| Sum of per-program medians | 93.646 s | 62.333 s | 1.502x |
| Median per-program time | 68.243 ms | 44.885 ms | - |
| Maximum per-program median | 8.452 s | 5.417 s | - |
| Paired median speedup | - | - | 1.596x |
| Paired geometric-mean speedup | - | - | 1.711x |

Rust was faster on 1,219 programs and OCaml was faster on 48. There were no
status mismatches.

## Boundary and method

- Corpus: all 1,267 `Run success` entries in
  `p4spec/test/run/run_pos_sl.expected`, with no additional exclusions.
- Relation: `Program_inst`. It invokes `Program_ok` internally.
- Options: `cache=true`, `deterministic=false`, `guard=false`.
- Both runners initialize the structured SL specification and parse or decode
  each P4 program outside the timer.
- Each already-parsed program is evaluated once as warm-up and then three
  times. The reported program time is the median of those three measurements.
- Every top-level evaluation clears the interpreter call and subtype caches at
  the normal OCaml/Rust entry-point boundary.
- JSON output and result formatting are outside the timer.
- The speedup for a program is `OCaml median / Rust median`. The confidence
  interval uses 10,000 paired bootstrap resamples with seed `20260819`.

The raw, untracked measurement files are:

- `target/corpus/ocaml-interpreter-only-1267-3x.jsonl`
- `target/corpus/rust-interpreter-only-fat-lto-1267-3x.jsonl`

## Environment

- Date: 2026-08-19
- Machine: Apple M1, 8 logical CPUs, 16 GiB RAM
- OS: macOS 26.5 (Darwin 25.5.0, arm64)
- Rust: `rustc 1.97.1`, `cargo 1.97.1`
- OCaml: `5.1.0`
- Dune: `3.16.1`
- Rust allocator: `mimalloc 0.1.52`
- Rust release profile: fat LTO, one codegen unit
- Source checkpoint before the benchmark-driver change: `b1634674`

Both binaries were native optimized builds:

```sh
make release
cd p4spec-rust
cargo build --locked --release --bin p4spec-rust-corpus
```

The OCaml measurement command used the official positive manifest directly:

```sh
programs=("${(@f)$(sed -n 's#^Run success: ../../../##p' \
  p4spec/test/run/run_pos_sl.expected)}")
args=()
for program in "${programs[@]}"; do args+=(-p "$program"); done
_build/default/p4spec/bin/main.exe benchmark-sl spec \
  -i p4c/p4include -rel Program_inst -warmup 1 -repeat 3 \
  "${args[@]}" \
  > p4spec-rust/target/corpus/ocaml-interpreter-only-1267-3x.jsonl
```

The Rust command used the corresponding versioned value envelopes exported by
`p4spectec export-p4-json`:

```sh
cd p4spec-rust
target/release/p4spec-rust-corpus \
  --spec target/corpus/spec.json --expect pass --warmup 1 --repeat 3 \
  -o target/corpus/rust-interpreter-only-fat-lto-1267-3x.jsonl \
  "${json_programs[@]}"
```

## Optimization checkpoint

Before the final two accepted hot-path changes, the same one-pass Rust corpus
took 86.207 seconds. In-place variant-argument traversal reduced it to 72.740
seconds, and indexing global variant cases by notation shape reduced it to
69.267 seconds. Fat LTO with one codegen unit then reduced the one-pass sum to
61.521 seconds. The final three-run median sum is 62.333 seconds.

Rejected experiments were restored completely when the full-corpus sum
regressed, even if a representative program improved. These included a custom
shape hasher, direct fallible notation mapping, and allocation-free `RuleI`
argument partitioning.
