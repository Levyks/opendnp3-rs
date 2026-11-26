# opendnp3-rs

A DNP3 (IEEE 1815) stack in Rust, licensed under MIT or Apache-2.0. The protocol core does no I/O. Tokio, smol, compio, Embassy, blocking `std`, and other languages drive it.

This is a from-scratch rewrite of the archived C++ library [opendnp3](https://github.com/dnp3/opendnp3). It is not a port of any other Rust DNP3 library, and it is not affiliated with Step Function I/O LLC.

## Why this exists

[opendnp3](https://github.com/dnp3/opendnp3) was the open reference implementation of DNP3. It is Apache-2.0, and the repository was archived in 2022. The stack its maintainers pointed people to after that is a Rust library from Step Function I/O LLC. That library is not under an OSI-approved license, so it cannot be the base of an Apache-2.0 project.

`opendnp3-rs` is the alternative: original Rust, written against the protocol and against the Apache-2.0 C++ code. Agents working in this repository are instructed not to read the Step Function sources. If you contribute, follow [AGENTS.md](AGENTS.md).

The code was vibe coded. Coding agents wrote it. It was not transcribed from the C++ line by line. The check on that is the test suite below.

## Tests

`cargo test --workspace` passes **533** tests.

**415** of those are the C++ unit suite, ported case by case from `cpp/tests/unit` in [dnp3/opendnp3](https://github.com/dnp3/opendnp3). That directory has 436 `TEST_CASE`s. The 21 that were not ported cover C++ utilities rather than the protocol: `TestList`, `TestLog`, `TestShiftableBuffer`, `TestTimeDuration`, `TestIPEndpointsList`, and `TestCollectionTransform`. The C++ `asiotests/` and `integration/` trees were not copied. The Rust adapters have their own TCP tests instead.

## Layout

The core crate is `no_std + alloc`. Enable `async` and a driver crate supplies the clock and the byte stream. `#![forbid(unsafe_code)]` is set for the workspace.

| Crate | What it does |
|---|---|
| `opendnp3` | Link, transport, application, master, and outstation. No sockets. |
| `opendnp3-tokio` | TCP. Optional `tls` and `serial` features. |
| `opendnp3-smol` | smol |
| `opendnp3-compio` | compio (io_uring / IOCP) |
| `opendnp3-embassy` | Embassy, for `no_std` async |
| `opendnp3-std` | Blocking `std::net` on OS threads. No async runtime. |
| `opendnp3-ffi` | [BoltFFI](https://boltffi.dev) 0.32 bindings. Python, C#, and Java. The Rust side still has no sockets. |

Version `0.1.0`. The Rust crates live in this repository. The language packages below are `0.1.0` on PyPI, NuGet, and Maven Central.

## Rust

A Tokio outstation and master. The same shape is in [`opendnp3-tokio/tests/tcp_integration.rs`](opendnp3-tokio/tests/tcp_integration.rs).

```rust
let listener = TcpListener::bind("127.0.0.1:20000").await?;

let mut outstation_link = LinkConfig::new(false);
outstation_link.local_addr = 10;
outstation_link.remote_addr = 1;

let outstation = spawn_tcp_outstation(
    listener,
    outstation_link,
    OutstationConfig {
        database: DatabaseConfig::all_types(4),
        ..Default::default()
    },
    MyCommandHandler,
    DefaultOutstationApplication,
);

outstation.update_binary(0, Binary::new(true), EventMode::Suppress);

let mut master_link = LinkConfig::new(true);
master_link.local_addr = 1;
master_link.remote_addr = 10;

let (master, events) = spawn_tcp_master(
    "127.0.0.1:20000".parse()?,
    master_link,
    MasterParams::default(),
    MySoeHandler,
    DefaultMasterApplication,
).await?;
```

`opendnp3-std` is the runtime-free path: `run_blocking_master` and `run_blocking_outstation` take a `TcpStream` and block the calling thread. smol, compio, and Embassy each wrap the same core session in that runtime's I/O types. One integration test in each crate opens a loopback TCP pair (Embassy's test runs on the host).

## Python

```text
pip install opendnp3==0.1.0 opendnp3-tcp==0.1.0
```

`opendnp3` is the BoltFFI binding. `opendnp3-tcp` is the asyncio socket adapter. The Rust side still opens no sockets.

```python
import opendnp3 as dnp3
import opendnp3_tcp as tcp

server = await tcp.listen("127.0.0.1", 20000, outstation.run_with_pipe)
task, writer = await tcp.connect("127.0.0.1", 20000, master.run_with_pipe)
```

## .NET

```text
dotnet add package Levyks.OpenDnp3 --version 0.1.0
dotnet add package Levyks.OpenDnp3.Tcp --version 0.1.0
```

```csharp
using Levyks.OpenDnp3;
using Levyks.OpenDnp3.Tcp;

var listener = TcpServer.Listen(IPAddress.Loopback);
var serverTask = Task.Run(async () =>
{
    await using var accepted = await TcpServer.AcceptAsync(listener);
    await outstation.Run(accepted.Stream);
});

await using var client = await TcpConnection.ConnectAsync("127.0.0.1", port);
await master.Run(client.Stream);
```

## Java

```xml
<dependency>
  <groupId>com.levyks</groupId>
  <artifactId>opendnp3</artifactId>
  <version>0.1.0</version>
</dependency>
<dependency>
  <groupId>com.levyks</groupId>
  <artifactId>opendnp3-tcp</artifactId>
  <version>0.1.0</version>
</dependency>
```

```java
try (TcpServer server = TcpServer.listen("127.0.0.1", 20000)) {
    try (TcpConnection conn = TcpConnection.connect("127.0.0.1", server.port())) {
        master.run(conn.stream());
    }
}
```

Full programs that poll, operate a CROB, and scan are in [`opendnp3-ffi/examples`](opendnp3-ffi/examples).

## License

[MIT](LICENSE-MIT) or [Apache-2.0](LICENSE-APACHE), at your option. The SPDX identifier is `MIT OR Apache-2.0`.

The ported unit tests descend from [dnp3/opendnp3](https://github.com/dnp3/opendnp3), which is Apache-2.0. Copyright for that suite is Step Function I/O, LLC, licensed to Green Energy Corp and Step Function I/O LLC. See [NOTICE](NOTICE).
