# metasq-example: mini P4 and p4st-in-X specifications

Small specifications that run end to end: a mini P4 specification, and mini p4st interpreters written in p4st, PLT Redex, and K that run it.
Nothing here is part of the main implementation: `driver/` is a crate of its own that uses `p4spectec` as a library, and no file outside `metasq-example/` changes.

| directory | contents | tower |
|---|---|---|
| `p4/` | P4 syntax, domains, `$find`, statement and expression rules, without P4 IR | `p4 ◁ p4st ◀ rust` |
| `p4/program/` | `ebpf.p4` and `filter.p4` as P4 source | full P4 spec |
| `st-in-st/` | AL syntax, domains, `Rel_call`, `Ruls_eval`, `Det_check`, `Rul_eval`, `Prem_eval`, `ExtRel_call`, `RunRel` | `p4 ◁ p4st ◁ p4st ◀ rust` |
| `st-in-redex/` | AL syntax, machine terms, frames, and reduction rules | `p4st ◁ redex ◀ racket` |
| `st-in-k/` | AL syntax, configuration, boundaries, and rewrite rules | `p4st ◁ k ◀ llvm` |
| `driver/` | Packet Engine, `ExtRel_call` relay, AL encoder | Rust |

## Running

```sh
make metameta       # filter.p4 on three packets, meta and meta-meta, compared
make redex          # the walk-through on Redex, printed as the outer ruls-eval changes
make k              # the walk-through on K
make k-trace        # <k> around the UNMATCH of rule if_true (about a minute)
make p4-typecheck   # filter.p4 through the full P4 specification (Program_ok)
```

Requires cargo, racket with redex, and K (kompile, krun).

## What runs

- `metasq-example meta` builds the AL of `p4/p4.watsup` and `p4/filter.watsup`, takes the program from `$filter_p4`, and runs the eBPF pipeline: `P4Block_call` on `prs` with a packet, then on `flt` with the parsed header. `Parse_pkt` is served by `PacketEngine`.
- `metasq-example metameta` runs the same packets through `RunRel` of `st-in-st/`, with the AL of `p4/p4.watsup` encoded as a `spec`. `ExtRel_call` is served by `Relay`, which decodes the call, answers it with the same Packet Engine, and encodes the reply. Both towers must give drop, pass, pass.
- `metasq-example emit out` writes the walk-through call `P4Stmt_eval (E, if (hdr.dst == 0x42) ...)` with `hdr.dst = 0x43`, the encoded specification, and the meta-execution result, as `out/walkthrough.rktd` for Redex and `out/walkthrough.k` for K. Both runs check their result against the meta-execution result.

The encoder covers only the AL forms the mini P4 specification produces (`encode.rs`); an iterated variable `x*{x <- x*}` is `EVAR "x" [LIST]`.
The Redex and K minis omit the determinism check; the p4st one does it (`Det_check`).
