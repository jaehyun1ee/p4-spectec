# Prose source comparison

The authoritative annotation comparison uses the OCaml implementation from integration base `55bf880ef8edd699b585346dab68a106ecaa3254`. That base contains semantic source revision `e553b3d8275885db8536617feb0dc1ffb62ef28b` and structuring correction `77156dbff063c35428b28222714e084ca0c8e366`.

`make test-prose-diff` exports rule-group-preserving SL and PL from that source, checks these byte-level provenance hashes, decodes both with the Rust OCaml codecs, runs `prose::convert` on the exact source SL, and compares the complete decoded PL values:

| Artifact | SHA-256 |
| --- | --- |
| Source SL JSON | `47cbdceab528370ac99e14bd05c86ce55faa4a3c4f8b0f2c78ceb493e0405e6c` |
| Source PL JSON | `4846b7ca7d5d6319fc54d378fdf4d17fda37fa780e17b143ebf5385452d10d33` |
| Source-derived Rust rendering | `c05dce3157ac9337e6660e169d1d053370cf5d2408dc3ab9ae6b9f9cd16dd7f6` |
| Source-derived structured PL without spans | `da8f59bac255b29730e80b33c5502e28cd6612dff2c6ed3f35f16dccfbe55e55` |

The exact-source comparison includes definition order, dispatch/group tiers, alternatives, hints, binder/use names, source spans and failure destinations. OCaml instruction `iid` is validated while decoding but is not stored by the shared Rust SL/PL models, so it is the only omitted source field.

The separate native acceptance (`make test-prose`) starts from the Rust parser and therefore ignores spans in its structured hash. Span parity is established by the exact-source gate; the native gate checks integration output after inherited frontend and structuring differences.
