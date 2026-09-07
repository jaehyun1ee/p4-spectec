# Test targets for the Rust port.
#
# The default targets run the fast, pure-Rust unit tests. The OCaml-differential
# tests are #[ignore]d and run only via the `*-diff` targets: they need the
# pinned OCaml toolchain (opam/dune) and print a progress bar over the corpus.

CARGO := cargo

.PHONY: test test-frontend test-p4-parse test-elab test-algo test-lang test-wire \
        test-runtime test-frontend-diff test-elab-diff test-algo-diff test-diff

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

# OCaml-differential tests (need opam/dune; render a corpus progress bar)

test-frontend-diff:
	$(CARGO) test --test frontend -- --ignored --nocapture

test-elab-diff:
	$(CARGO) test --test pass elaborate:: -- --ignored --nocapture

test-algo-diff:
	$(CARGO) test --test pass algo:: -- --ignored --nocapture

test-diff: test-frontend-diff test-elab-diff test-algo-diff
