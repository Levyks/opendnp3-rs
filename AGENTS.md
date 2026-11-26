# AGENTS.md

## Project Goal

`opendnp3-rs` is a from-scratch rewrite of [opendnp3](https://github.com/dnp3/opendnp3)
(the archived C++ DNP3 protocol library) in a modern Rust stack.

The intent is a clean-room, idiomatic Rust implementation of the DNP3
(IEEE 1815) protocol: async-first, safe, and built on the standard Rust
networking ecosystem.

## LEGAL — READ THIS FIRST, IT IS NON-NEGOTIABLE

**You must NEVER, under any circumstances, look at, fetch, read, quote,
reference, or derive code from:**

- `https://github.com/stepfunc/dnp3` — the stepfunc `dnp3` Rust library
- Any other software, documentation, examples, generated code, schema files,
  or artifacts produced by Step Function I/O LLC ("stepfunc") that are
  distributed under a proprietary, commercial, source-available, or otherwise
  non-OSI-approved license (e.g., their commercial license, BSL-style terms,
  or "license required" downloads)

This includes indirect access: do not fetch the repo via `webfetch`, do not
search for it, do not read mirrors or copies of it, do not browse its issues,
PRs, or docs, and do not ask the user to paste code from it. Its license terms
impose restrictions that would contaminate this project. Treat it as if it
does not exist.

### Permitted sources

You **may** consult and base the implementation on:

- `https://github.com/dnp3/opendnp3` — the original opendnp3 C++ library
  (Apache-2.0 licensed, now archived)
- The public DNP3 / IEEE 1815 specification material that is freely available
- Any other software under a genuine OSI-approved open source license
  (Apache-2.0, MIT, BSD, MPL-2.0, etc.)
- Public protocol documentation, Wireshark dissectors, test captures, and
  interoperability guides that are freely distributable

When in doubt about whether a source is permissibly licensed, **do not use
it** — ask the user first.

### Clean-room expectations

- Write original Rust code. Do not transliterate the C++ line-by-line; use it
  to understand protocol semantics, state machines, and wire formats, then
  express them idiomatically in Rust.
- Never copy identifiers, module layouts, or doc comments verbatim from any
  non-permissive source (see above — stepfunc sources are entirely off-limits,
  so this should never come up).
- If you need a piece of functionality that exists only in a
  restrictively-licensed codebase, design it independently from the protocol
  spec and the permissively-licensed references instead.

## Tech direction

- Modern Rust (edition 2024)
- Favor small, well-typed protocol layers (link layer, transport, application
  layer) mirroring DNP3's architecture, but with idiomatic Rust APIs
  (`enum`s over error codes, `Result` over exceptions, builder/config structs)
- `#![forbid(unsafe_code)]` mindset: avoid `unsafe` unless explicitly
  approved by the user

## Architecture decisions

### Runtime / transport independence

The library is **runtime-agnostic** — the core never depends on tokio or any
specific executor/transport:

- **`opendnp3` (core crate)**: zero I/O, zero runtime dependencies. Pure
  protocol state machines (link layer, transport layer, application layer,
  master/outstation logic).
- **I/O via traits**: byte streams use `futures-io`'s `AsyncRead`/`AsyncWrite`
  (implemented by tokio, async-std, and smol). A simpler frame-oriented
  transport trait may be defined instead if the link layer API favors it —
  DNP3 also runs over serial, not just TCP.
- **Time via abstraction**: the core uses a `Clock`/`Sleep`-style trait for
  all timeouts, keep-alives, and retry backoff (precedent: `quinn`'s
  `Runtime` trait). Never call `std::time::Instant::now()` or
  `tokio::time::sleep` in core code.
- **`opendnp3-tokio` (companion crate)**: ready-made TCP client/server, TLS,
  and serial channels plus a `TokioClock`. This is where ergonomic,
  batteries-included integration lives; the core stays clean.

Precedents for this pattern: `rustls`, `quinn`, `smoltcp`, `h2`.

### Test strategy — port the opendnp3 unit suite

The C++ repo (cloned at `../opendnp3`, i.e. `C:\Users\Levyks\dev\libs\opendnp3`)
has ~453 Catch2 `TEST_CASE`s. They are almost all *"hex frame in → hex frame
out + assert mock calls"*, which makes them language-independent conformance
vectors.

- **Port `cpp/tests/unit/` faithfully** — APDU parse/write, CRC, link frame,
  transport layer, master/outstation state machines, event storage, database.
  Same test names where practical so coverage can be audited against the
  original.
- **Port the harness first**: `cpp/tests/dnp3mocks/` (mock layers, SOE
  handler, APDU header handler) and `cpp/tests/unit/utils/` (fixtures like
  `LinkLayerTest`, `MasterTestFixture`, `OutstationTestObject`, hex helpers)
  into a Rust test-util module before bulk-translating test cases.
- **Deterministic time**: the C++ tests use `MockExecutor::AdvanceTime`; the
  Rust equivalent is a `ManualClock` implementing the core clock trait +
  mock transports. Never use real sockets or real time in unit tests.
- **Do NOT port** `asiotests/` (tests the C++ networking layer itself) or
  `integration/` verbatim; write fresh Rust equivalents once `opendnp3-tokio`
  exists.
- **Reuse the fuzz corpus** at `cpp/tests/fuzz/corpus/` for `cargo-fuzz`.

## Working conventions

- Keep the crate usable as a library; binaries/examples belong in `examples/`
  or `src/bin/`
- Add tests for protocol encode/decode round-trips; conformance against real
  captures is valued
- Run `cargo fmt`, `cargo clippy`, and `cargo test` before considering work
  done
