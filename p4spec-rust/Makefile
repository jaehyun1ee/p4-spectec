# Test targets for the Rust port.
#
# Default tests include unit tests and checked-in corpus expectations.
# OCaml-differential tests are ignored and need the pinned opam/dune toolchain.

CARGO := cargo

.PHONY: test test-frontend test-p4-parse test-elab test-algo test-lang test-wire \
        test-runtime test-speclang test-frontend-diff test-elab-diff \
        test-algo-diff test-runtime-diff test-diff

# No OCaml toolchain required

test:
	$(CARGO) test

test-frontend:
	$(CARGO) test --test frontend

# P4 program parser. The oracle sub-tests parse the Pos/Neg corpus under
# `p4c/testdata` and render a progress bar, so they need the `p4c` submodule
# (`git submodule update --init p4c`) and a C preprocessor; `--nocapture` keeps
# the bar visible.
test-p4-parse:
	$(CARGO) test --test interface p4_parse:: -- --nocapture --test-threads=1

test-elab:
	$(CARGO) test --test pass elaborate::
	$(CARGO) test --test cli commands::test_elab_command

# algo tests live in two places: the binding/side-condition unit tests run
# inside the library (`--lib`), the conversion tests in the `pass` binary.
test-algo:
	$(CARGO) test --lib pass::algo
	$(CARGO) test --test pass algo::
	$(CARGO) test --test cli commands::test_algo_command

# Full-spec elab/algo output compared byte-for-byte with the OCaml .expected files
test-speclang:
	$(CARGO) test --test cli matches_expected

test-lang:
	$(CARGO) test --test lang

test-wire:
	$(CARGO) test --test wire

test-runtime:
	$(CARGO) test --test runtime

# OCaml-differential tests (need opam/dune)

test-frontend-diff:
	$(CARGO) test --test frontend -- --ignored --nocapture --test-threads=1

test-elab-diff:
	$(CARGO) test --test pass elaborate:: -- --ignored --nocapture

test-algo-diff:
	$(CARGO) test --test pass algo:: -- --ignored --nocapture

test-runtime-diff:
	$(CARGO) test --test runtime -- --ignored --nocapture

test-diff: test-frontend-diff test-elab-diff test-algo-diff test-runtime-diff
