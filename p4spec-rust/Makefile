# Test targets for the Rust port.
#
# The default targets run the fast, pure-Rust unit tests. The OCaml-differential
# tests are #[ignore]d and run only via the `*-diff` targets: they need the
# pinned OCaml toolchain (opam/dune) and print a progress bar over the corpus.

CARGO := cargo

.PHONY: test test-frontend test-p4-parse test-elab test-algo test-lang test-wire \
        test-runtime test-run-al test-run-al-diff test-frontend-diff test-elab-diff test-algo-diff test-diff

# Fast unit tests (no OCaml toolchain required)

test:
	$(CARGO) test

test-frontend:
	$(CARGO) test --test frontend

# P4 program parser. The oracle sub-tests parse the Pos/Neg corpus under
# `p4c/testdata` and render a progress bar, so they need the `p4c` submodule
# (`git submodule update --init p4c`) and a C preprocessor; `--nocapture` keeps
# the bar visible.
test-p4-parse:
	$(CARGO) test --test interface p4_parse:: -- --nocapture

test-elab:
	$(CARGO) test --test pass elaborate::

# algo tests live in two places: the binding/side-condition unit tests run
# inside the library (`--lib`), the conversion tests in the `pass` binary.
test-algo:
	$(CARGO) test --lib pass::algo
	$(CARGO) test --test pass algo::

test-lang:
	$(CARGO) test --test lang

test-wire:
	$(CARGO) test --test wire

test-runtime:
	$(CARGO) test --test runtime

# Native AL execution and command-line checks
test-run-al:
	$(CARGO) test --locked --test runtime interp::al::
	$(CARGO) test --locked --test cli

# OCaml-differential tests (need opam/dune; render a corpus progress bar)

test-frontend-diff:
	$(CARGO) test --test frontend -- --ignored --nocapture

test-elab-diff:
	$(CARGO) test --test pass elaborate:: -- --ignored --nocapture

test-algo-diff:
	$(CARGO) test --test pass algo:: -- --ignored --nocapture

test-run-al-diff:
	$(CARGO) test --locked --release --test runtime interp::al::compare::test_run_al_corpus_matches_ocaml -- --ignored --exact --nocapture
	$(CARGO) test --locked --release --test runtime interp::al::compare::test_run_al_det_corpus_matches_ocaml -- --ignored --exact --nocapture
	$(CARGO) test --locked --release --test runtime interp::al::compare::test_run_al_cached_corpus_matches_ocaml -- --ignored --exact --nocapture
	$(CARGO) test --locked --release --test runtime interp::al::compare::test_run_al_cached_det_corpus_matches_ocaml -- --ignored --exact --nocapture

test-diff: test-frontend-diff test-elab-diff test-algo-diff test-run-al-diff

# Standalone expected driver; product unit tests remain in tests/
.PHONY: test-expected test-promote test-elab-expected test-algo-expected test-run-al-expected
DRIVER = $(CARGO) run --locked --release --manifest-path test-driver/Cargo.toml --target-dir target --

test-elab-expected:
	env -u UPDATE_EXPECT $(DRIVER) elab

test-algo-expected:
	env -u UPDATE_EXPECT $(DRIVER) algo

test-run-al-expected:
	env -u UPDATE_EXPECT $(DRIVER) run-al

test-expected: test-elab-expected test-algo-expected test-run-al-expected

# Rerun elab/algo and update snapshots; does not approve a previous run
# AL expected and excludes are never promoted
test-promote:
	UPDATE_EXPECT=1 $(DRIVER) elab
	UPDATE_EXPECT=1 $(DRIVER) algo
