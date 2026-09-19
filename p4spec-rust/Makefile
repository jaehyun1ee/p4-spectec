# Test targets for the Rust port.
#
# The default targets run the fast, pure-Rust unit tests. The OCaml-differential
# tests are #[ignore]d and run only via the `*-diff` targets: they need the
# pinned OCaml toolchain (opam/dune).

CARGO := cargo

.PHONY: test test-frontend test-p4parse-unit test-elab-unit test-algo-unit test-structure-unit test-prose-unit test-lang test-wire \
        test-runtime test-run-al-unit test-run-sl-unit test-frontend-diff test-algo-diff test-prose-diff test-diff

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

test-structure-unit:
	$(CARGO) test --locked --lib pass::structure::
	$(CARGO) test --locked --test pass structure::
	$(CARGO) test --locked --test lang al::fresh::

test-prose-unit:
	$(CARGO) test --locked --test lang hints::
	$(CARGO) test --locked --test lang pl::
	$(CARGO) test --locked --test pass prose::

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

test-run-sl-unit:
	$(CARGO) test --locked --test runtime interp::sl::
	$(CARGO) test --locked --test runtime sim_plugin::ebpf::test_sl_counter_state
	$(CARGO) test --locked --test cli

# OCaml investigation checks (need opam/dune)
# Algo compares rejected-input diagnostics; full spec uses test-expected

test-frontend-diff:
	$(CARGO) test --test frontend -- --ignored --nocapture

test-algo-diff:
	$(CARGO) test --test pass algo:: -- --ignored --nocapture

PROSE_SOURCE_DIR := $(abspath target/prose-source)
PROSE_SOURCE_SL := $(PROSE_SOURCE_DIR)/source-sl.json
PROSE_SOURCE_PL := $(PROSE_SOURCE_DIR)/source-pl.json

test-prose-diff:
	mkdir -p "$(PROSE_SOURCE_DIR)"
	trap 'rm -f "$(PROSE_SOURCE_SL)" "$(PROSE_SOURCE_PL)"' EXIT; \
		cd "$(abspath ..)" && \
		opam exec -- dune exec ./p4spec/test/prose-export/export_prose_source.exe -- sl spec > "$(PROSE_SOURCE_SL)" && \
		opam exec -- dune exec ./p4spec/test/prose-export/export_prose_source.exe -- pl spec > "$(PROSE_SOURCE_PL)" && \
		cd "$(CURDIR)" && \
		env -u UPDATE_EXPECT $(DRIVER) prose-source --sl "$(PROSE_SOURCE_SL)" --pl "$(PROSE_SOURCE_PL)"

test-diff: test-frontend-diff test-algo-diff test-prose-diff

# Standalone expected driver; product unit tests remain in tests/
.PHONY: test-expected test-promote test-p4parse test-elab test-algo test-structure test-prose test-run-al test-run-sl
DRIVER_TARGET_DIR ?= $(if $(CARGO_TARGET_DIR),$(CARGO_TARGET_DIR),target)
DRIVER = $(CARGO) run --locked --release --manifest-path test-driver/Cargo.toml --target-dir $(DRIVER_TARGET_DIR) --

test-p4parse:
	env -u UPDATE_EXPECT $(DRIVER) p4parse

test-elab:
	env -u UPDATE_EXPECT $(DRIVER) elab

test-algo:
	env -u UPDATE_EXPECT $(DRIVER) algo

test-structure:
	env -u UPDATE_EXPECT $(DRIVER) structure

test-prose:
	env -u UPDATE_EXPECT $(DRIVER) prose

test-run-al:
	env -u UPDATE_EXPECT $(DRIVER) run-al

test-run-sl:
	env -u UPDATE_EXPECT $(DRIVER) run-sl
	env -u UPDATE_EXPECT $(DRIVER) run-sl --det

test-expected: test-p4parse test-elab test-algo test-structure test-prose test-run-al test-run-sl

# Rerun elab/algo and update snapshots; does not approve a previous run
# P4 parser/structure/AL expected and excludes are never promoted
test-promote:
	UPDATE_EXPECT=1 $(DRIVER) elab
	UPDATE_EXPECT=1 $(DRIVER) algo

# Native simulation uses source-derived expectations in both cached modes
.PHONY: test-sim-al test-sim-sl

test-sim-al:
	env -u UPDATE_EXPECT $(DRIVER) sim-al
	env -u UPDATE_EXPECT $(DRIVER) sim-al --det

test-sim-sl:
	env -u UPDATE_EXPECT $(DRIVER) sim-sl
	env -u UPDATE_EXPECT $(DRIVER) sim-sl --det
