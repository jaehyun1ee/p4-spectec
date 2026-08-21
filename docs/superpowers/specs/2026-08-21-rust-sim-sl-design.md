# Rust `sim-sl` PoC Design

## Goal

Extend `p4spec-rust` with a native Rust P4 simulator that can run the seven
non-deterministic suites covered by the OCaml `@sim-sl` alias:

- eBPF P4C
- eBPF P4Testgen
- PSA P4C
- v1model P4C
- v1model P4Testgen
- v1model custom
- v1model regression

The Rust suites must produce output that matches the existing OCaml
`.expected` files exactly. The work is a feasibility PoC. It does not replace
the OCaml simulator, change the existing `@sim-sl` alias, or establish the Rust
implementation as the reference implementation.

The implementation order is eBPF, then PSA, then v1model. This starts with the
smallest architecture, exercises the shared simulator boundary before the
largest port, and leaves the broadest v1model corpus as the final validation.

## Non-goals

- Port the P4 parser or STF parser to Rust
- Support `@sim-sl-det`
- Replace or modify the behavior of the OCaml `@sim-sl` suites
- Optimize simulator performance during the semantic port
- Redesign the P4 architecture models or move their behavior into SL
- Add an OCaml/Rust RPC or long-lived streaming protocol

## System Boundary

OCaml remains responsible for inputs that already have authoritative parsers
and corpus-discovery logic:

1. Parse and elaborate the SL specification and export the existing versioned
   SL JSON envelope.
2. Discover P4/STF test pairs, patches, and exclusions using the current test
   utilities.
3. Parse each included P4 program into its runtime `Value` representation.
4. Parse each included STF file into the STF AST.
5. Export one versioned simulation-suite JSON document.

Rust is responsible for everything after that boundary:

1. Decode the SL specification and simulation suite.
2. Initialize the selected architecture from the parsed P4 program value.
3. Execute STF statements, architecture pipelines, externs, tables, and packet
   matching.
4. Emit the same user-visible log and summary format as the OCaml simulator.

No callback from Rust to OCaml is permitted during simulation. Passing all
parsed inputs in one document keeps the PoC boundary explicit and avoids a
stateful transport protocol.

## Wire Format

Add one schema, `p4spectec.sim-suite.v1`, represented by three concepts:

- `SimSuite`: architecture name and ordered entries
- `SimEntry`: either an included run or an exclusion
- `StfStmt`: the existing STF statement variants and their fields

A suite has the following logical shape:

```json
{
  "schema": "p4spectec.sim-suite.v1",
  "kind": "sim-suite",
  "payload": {
    "arch": "ebpf",
    "entries": [
      {
        "kind": "run",
        "p4_path": "../../../p4c/testdata/p4_16_samples/example.p4",
        "stf_path": "../../../p4c/testdata/p4_16_samples/example.stf",
        "patched": false,
        "program": {
          "schema": "p4spectec.value.v1",
          "kind": "value",
          "payload": {}
        },
        "stf": []
      },
      {
        "kind": "exclude",
        "p4_path": "../../../p4c/testdata/p4_16_samples/excluded.p4",
        "stf_path": "../../../p4c/testdata/p4_16_samples/excluded.stf",
        "patched": false,
        "group": "dynamic/p4c-specific"
      }
    ]
  }
}
```

The nested `program` uses the existing runtime-value envelope without defining
a second P4 AST encoding. STF uses an explicitly tagged encoding rather than
the implementation-specific representation produced by a deriving library.
The OCaml and Rust codecs share fixture tests for every STF variant used by the
seven suites. Excluded entries carry metadata only and do not parse or embed a
P4 program or STF AST.

The first implementation reads a complete suite document into memory. This is
deliberate: the PoC favors a small, stateless interface over streaming state,
temporary artifact directories, or request/response framing. A measured memory
problem may later justify streaming the `entries` array without changing the
schema or simulator API.

## OCaml Exporter and Test Integration

Add an `export-sim-suite` command alongside the existing SL and P4 JSON export
commands. It accepts the same architecture, include, exclude, P4 directory,
STF directory, and patch directory inputs as the current simulation test
driver. It reuses `collect_test_pairs` and the existing exclusion grouping so
entry order and displayed paths remain identical.

Add a new `@sim-sl-rust` aggregate alias and seven architecture/corpus-specific
Rust aliases. Each rule:

1. Exports the SL specification.
2. Exports one simulation suite.
3. Runs the Rust simulator binary.
4. Captures stdout into a Rust-specific `.actual` file.
5. Diffs that file against the existing OCaml `.expected` file.

The Rust `.actual` files have distinct names. The existing OCaml targets,
actual files, and aliases are unchanged.

## Rust Simulator Architecture

The Rust code follows the current OCaml simulator boundaries without copying
the OCaml functor structure literally.

### Common simulator

The common simulator owns:

- packet input/output types and comparison
- outstanding transmitted-packet and expectation queues
- generic STF statement dispatch
- table entry and default-action updates
- mirror, multicast, and register STF operations exposed by an architecture
- per-case logging and per-suite statistics
- interpreter clearing between cases

The output queue matching behavior and wildcard/exact packet comparison are
ported directly from `Runtime.Sim.Io` and `Backend_sim.Make`.

### Specification bridge

Architecture code must call back into SL functions and relations. Extend the
existing Rust `SpecCall` interface with relation evaluation. The bridge exposes
typed helpers corresponding to the OCaml `Spec.Func`, `Spec.Rel`, `Spec.Pgm`,
`Spec.Pack`, and `Spec.Unpack` modules.

The parsed P4 program is already a runtime value, so architecture initialization
invokes `EBPF_init`, `PSA_init`, or `V1Model_init` directly with that value. No
P4 parser callback or filename-based program evaluation is needed in Rust.

### Architecture interface

Define one architecture trait covering only the operations used by the generic
STF runner:

- transform an STF statement
- initialize context and architecture values from a program
- drive one input packet through the pipeline
- update mirror, multicast, and register state
- evaluate architecture extern functions, methods, and relations

Architecture state and extern-object state remain encoded in runtime values,
matching the OCaml implementation. Rust architecture implementations must not
add hidden mutable state whose lifetime differs from a simulation case.

Common packet-in, packet-out, P4 value packing/unpacking, object lookup, and
table behavior live under a shared core module. Code moves into that module
only when at least two architecture ports use the same behavior.

### Architecture ports

Port architectures in this order:

1. **eBPF**: establish the simulator, extern, object, and packet pipeline
   boundaries with the smallest architecture and pass both eBPF suites.
2. **PSA**: add the broader object set, architecture state, register operations,
   multicast behavior, and the PSA pipeline; pass the PSA P4C suite.
3. **v1model**: add v1model-specific metadata, checksums, clone/resubmit/
   recirculate behavior, counters, meters, registers, mirroring, multicast, and
   the full pipeline; pass regression first, then custom, P4C, and P4Testgen.

Each port is a structural translation of the corresponding OCaml modules.
Semantic cleanup and performance optimization are deferred until all expected
outputs match.

## State and Reentrancy

The SL interpreter owns its interface, extern dispatcher, call caches, and
global context. Add an explicit per-case clear operation that clears interface
and extern state and interpreter caches while retaining the decoded immutable
specification. Local evaluation frames must already be unwound when a public
evaluation method returns.

Extern evaluation receives the existing `SpecCall` callback, extended for
relations, so an extern can invoke SL without borrowing the outer interpreter
again. Pipeline code outside an active interpreter call uses the public
function and relation evaluation methods. This preserves one call path and
avoids `Rc<RefCell<_>>`, global trampolines, or unsafe reentrant access.

## Errors

There are two error levels:

- Fatal setup errors: unreadable files, invalid JSON, schema mismatch, invalid
  architecture, or invalid suite structure terminate the Rust command with a
  non-zero status.
- Case errors: SL runtime failures, invalid runtime values, unsupported STF
  statements, unsupported externs, packet mismatches, and unmatched outputs or
  expectations produce the existing per-case failure output and allow later
  cases to run.

All ported helpers return structured errors with the P4/STF path and source
region when available. Input-dependent failures must not use `panic!`,
`unwrap`, or unchecked indexing. Internal invariants may be asserted only when
the corresponding invariant is established by typed construction in the same
module.

## Verification Strategy

Implementation is correctness-first and test-driven. Tests are added at the
responsible layer before or with each behavior:

1. OCaml/Rust fixture agreement for the suite envelope and every STF variant
   exercised by the target suites.
2. Rust unit tests for packet wildcard/exact comparison, output/expect queue
   ordering, unmatched queue failures, and architecture-independent STF
   handling.
3. Focused tests for P4 packing/unpacking, specification callbacks, object
   state round-trips, table updates, and packet-in/packet-out primitives.
4. eBPF differential milestones: a minimal representative case, then P4C and
   P4Testgen exact outputs.
5. PSA differential milestones: representative packet and stateful extern
   cases, then the P4C exact output.
6. v1model differential milestones: regression, custom, P4C, then P4Testgen
   exact outputs.
7. Final `cargo test` and `dune build @sim-sl-rust --profile=release`.

The final acceptance condition is that all seven Rust-specific actual files
diff cleanly against the existing expected files. Existing OCaml tests must
continue to pass. The current uncommitted Rust optimization work is user-owned
and must be preserved; simulator changes that overlap those files are applied
on top of the existing diff rather than replacing it.

## Commit Boundaries

Implementation tasks are divided so each commit builds and passes all tests
introduced up to that point:

1. Simulation suite and STF wire codecs plus OCaml exporter fixtures
2. Generic Rust STF runner, packet matching, output formatting, and CLI shell
3. Specification bridge and shared P4 packing, object, packet, and table core
4. eBPF architecture with both eBPF suite aliases passing
5. PSA architecture with the PSA suite alias passing
6. v1model core and regression/custom suite aliases passing
7. Remaining v1model P4C/P4Testgen aliases and aggregate `@sim-sl-rust`

No commit hard-codes a test filename or adds case-specific semantic behavior.
