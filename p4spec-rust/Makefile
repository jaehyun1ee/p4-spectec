# Test targets for the Rust port.
#
# The default targets run the fast, pure-Rust unit tests. The OCaml-differential
# tests are #[ignore]d and run only via the `*-diff` targets: they need the
# pinned OCaml toolchain (opam/dune).

CARGO := cargo

.PHONY: test test-frontend test-p4parse-unit test-elab-unit test-algo-unit test-lang test-wire \
        test-runtime test-run-al-unit test-run-al-diff test-frontend-diff test-elab-diff test-algo-diff test-diff

# Fast unit tests (no OCaml toolchain required)

test:
	$(CARGO) test

test-frontend:
	$(CARGO) test --test frontend

# P4 parser regressions; full roundtrip corpus lives in test-p4parse
test-p4parse-unit:
	$(CARGO) test --test interface p4_parse:: -- --nocapture

test-elab-unit:
	$(CARGO) test --test pass elaborate::

# algo tests live in two places: the binding/side-condition unit tests run
# inside the library (`--lib`), the conversion tests in the `pass` binary.
test-algo-unit:
	$(CARGO) test --lib pass::algo
	$(CARGO) test --test pass algo::

test-lang:
	$(CARGO) test --test lang

test-wire:
	$(CARGO) test --test wire

test-runtime:
	$(CARGO) test --test runtime

# Native AL execution and command-line checks
test-run-al-unit:
	$(CARGO) test --locked --test runtime interp::al::
	$(CARGO) test --locked --test cli

# OCaml investigation checks (need opam/dune)
# Elab/algo compare rejected-input diagnostics; full spec uses test-expected

test-frontend-diff:
	$(CARGO) test --test frontend -- --ignored --nocapture

test-elab-diff:
	$(CARGO) test --test pass elaborate:: -- --ignored --nocapture

test-algo-diff:
	$(CARGO) test --test pass algo:: -- --ignored --nocapture

# Focused choice, guard and host-reentry oracle checks; full AL uses test-expected
test-run-al-diff:
	$(CARGO) test --locked --release --test runtime interp::al::compare:: -- --ignored --nocapture

test-diff: test-frontend-diff test-elab-diff test-algo-diff test-run-al-diff

# Standalone expected driver; product unit tests remain in tests/
.PHONY: test-expected test-promote test-p4parse test-elab test-algo test-run-al
DRIVER_TARGET_DIR ?= $(if $(CARGO_TARGET_DIR),$(CARGO_TARGET_DIR),target)
DRIVER = $(CARGO) run --locked --release --manifest-path test-driver/Cargo.toml --target-dir $(DRIVER_TARGET_DIR) --

test-p4parse:
	env -u UPDATE_EXPECT $(DRIVER) p4parse

test-elab:
	env -u UPDATE_EXPECT $(DRIVER) elab

test-algo:
	env -u UPDATE_EXPECT $(DRIVER) algo

test-run-al:
	env -u UPDATE_EXPECT $(DRIVER) run-al

test-expected: test-p4parse test-elab test-algo test-run-al

# Rerun elab/algo and update snapshots; does not approve a previous run
# P4 parser/AL expected and excludes are never promoted
test-promote:
	UPDATE_EXPECT=1 $(DRIVER) elab
	UPDATE_EXPECT=1 $(DRIVER) algo

# Native simulation uses source-derived expectations in both cached modes
.PHONY: test-sim-al test-sim-al-diff

test-sim-al:
	env -u UPDATE_EXPECT $(DRIVER) sim-al
	env -u UPDATE_EXPECT $(DRIVER) sim-al --det

# Explicit development oracle; native simulation has no OCaml dependency
SIM_ORACLE = ../_build/default/p4spec/test/sim-oracle/sim_oracle.exe

test-sim-al-diff:
	cd .. && opam exec -- dune build p4spec/test/sim-oracle/sim_oracle.exe p4spec/test/sim-oracle/control_oracle.exe
	../_build/default/p4spec/test/sim-oracle/control_oracle.exe
	env -u UPDATE_EXPECT $(DRIVER) sim-al --oracle $(abspath $(SIM_ORACLE))
	env -u UPDATE_EXPECT $(DRIVER) sim-al --det --oracle $(abspath $(SIM_ORACLE))
