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

A Tokio outstation and master. `SoeHandler` and `CommandHandler` have default methods, so an empty impl is a valid handler. The same shape is in [`opendnp3-tokio/tests/tcp_integration.rs`](opendnp3-tokio/tests/tcp_integration.rs).

```rust
use opendnp3::app::Binary;
use opendnp3::enums::EventMode;
use opendnp3::link::LinkConfig;
use opendnp3::master::{DefaultMasterApplication, MasterParams, SoeHandler};
use opendnp3::outstation::{
    DatabaseConfig, DefaultCommandHandler, DefaultOutstationApplication, OutstationConfig,
};
use opendnp3_tokio::{spawn_tcp_master, spawn_tcp_outstation};
use tokio::net::TcpListener;

struct Soe;
impl SoeHandler for Soe {}

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
    DefaultCommandHandler,
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
    Soe,
    DefaultMasterApplication,
)
.await?;
```

`opendnp3-std` is the runtime-free path: `run_blocking_master` and `run_blocking_outstation` take a `TcpStream` and block the calling thread. smol, compio, and Embassy each wrap the same core session in that runtime's I/O types. One integration test in each crate opens a loopback TCP pair (Embassy's test runs on the host).

## Python

```text
pip install opendnp3==0.1.0 opendnp3-tcp==0.1.0
```

`opendnp3` is the BoltFFI binding. `opendnp3-tcp` is the asyncio socket adapter. You build `Outstation` and `Master` yourself, then hand each one's `run_with_pipe` to the adapter. The callback classes must implement every method on the trait.

```python
import opendnp3 as dnp3
import opendnp3_tcp as tcp

class Soe:
    def on_binary(self, values): pass
    def on_double_bit(self, values): pass
    def on_binary_output_status(self, values): pass
    def on_counter(self, values): pass
    def on_frozen_counter(self, values): pass
    def on_analog(self, values): pass
    def on_analog_output_status(self, values): pass
    def on_octet_string(self, values): pass

class Commands:
    def select_crob(self, command, index):
        return dnp3.CommandStatus.SUCCESS
    def operate_crob(self, command, index, op_type):
        return dnp3.CommandStatus.SUCCESS
    def select_ao_i32(self, value, index):
        return dnp3.CommandStatus.NOT_SUPPORTED
    def operate_ao_i32(self, value, index, op_type):
        return dnp3.CommandStatus.NOT_SUPPORTED
    def select_ao_i16(self, value, index):
        return dnp3.CommandStatus.NOT_SUPPORTED
    def operate_ao_i16(self, value, index, op_type):
        return dnp3.CommandStatus.NOT_SUPPORTED
    def select_ao_f32(self, value, index):
        return dnp3.CommandStatus.NOT_SUPPORTED
    def operate_ao_f32(self, value, index, op_type):
        return dnp3.CommandStatus.NOT_SUPPORTED
    def select_ao_f64(self, value, index):
        return dnp3.CommandStatus.NOT_SUPPORTED
    def operate_ao_f64(self, value, index, op_type):
        return dnp3.CommandStatus.NOT_SUPPORTED

all_classes = dnp3.ClassField(True, True, True, True)
no_classes = dnp3.ClassField(False, False, False, False)
link = dict(timeout_ms=1000, keep_alive_timeout_ms=60_000, respond_to_any_source=False)

outstation = dnp3.Outstation(
    link_cfg=dnp3.LinkConfig(is_master=False, local_addr=10, remote_addr=1, **link),
    params=dnp3.OutstationParams(
        2048, 2048, False, 5000, 5000, 5000, 10_000, 16,
        dnp3.ClassField(False, True, True, True), False, 10,
    ),
    database=dnp3.DatabaseConfig.all_types(3),
    cmd_handler=Commands(),
)
master = dnp3.Master(
    link_cfg=dnp3.LinkConfig(is_master=True, local_addr=1, remote_addr=10, **link),
    params=dnp3.MasterParams(
        5000, dnp3.TimeSyncMode.NONE, False, False,
        no_classes, all_classes, True, no_classes, 2048, 2048,
    ),
    soe=Soe(),
)

server = await tcp.listen("127.0.0.1", 20000, outstation.run_with_pipe)
task, writer = await tcp.connect("127.0.0.1", 20000, master.run_with_pipe)
```

## .NET

```text
dotnet add package Levyks.OpenDnp3 --version 0.1.0
dotnet add package Levyks.OpenDnp3.Tcp --version 0.1.0
```

```csharp
using System.Net;
using Levyks.OpenDnp3;
using Levyks.OpenDnp3.Tcp;

var allClasses = new ClassField(true, true, true, true);
var noClasses = new ClassField(false, false, false, false);

using var outstation = new Outstation(
    new LinkConfig(false, 10, 1, 1000, 60_000, false),
    new OutstationParams(2048, 2048, false, 5000, 5000, 5000, 10_000, 16,
        new ClassField(false, true, true, true), false, 10),
    DatabaseConfig.AllTypes(3),
    new Commands());

using var master = new Master(
    new LinkConfig(true, 1, 10, 1000, 60_000, false),
    new MasterParams(5000, TimeSyncMode.None, false, false, noClasses, allClasses, true, noClasses, 2048, 2048),
    new Soe());

var listener = TcpServer.Listen(IPAddress.Loopback);
int port = ((IPEndPoint)listener.LocalEndpoint).Port;
var serverTask = Task.Run(async () =>
{
    await using var accepted = await TcpServer.AcceptAsync(listener);
    await outstation.Run(accepted.Stream);
});

await using var client = await TcpConnection.ConnectAsync("127.0.0.1", port);
await master.Run(client.Stream);

sealed class Soe : SoeHandler
{
    public void OnBinary(Binary[] values) { }
    public void OnDoubleBit(DoubleBitBinary[] values) { }
    public void OnBinaryOutputStatus(BinaryOutputStatus[] values) { }
    public void OnCounter(Counter[] values) { }
    public void OnFrozenCounter(FrozenCounter[] values) { }
    public void OnAnalog(Analog[] values) { }
    public void OnAnalogOutputStatus(AnalogOutputStatus[] values) { }
    public void OnOctetString(OctetString[] values) { }
}

sealed class Commands : CommandHandler
{
    public CommandStatus SelectCrob(Crob command, ushort index) => CommandStatus.Success;
    public CommandStatus OperateCrob(Crob command, ushort index, OperateType opType) => CommandStatus.Success;
    public CommandStatus SelectAoI32(int value, ushort index) => CommandStatus.NotSupported;
    public CommandStatus OperateAoI32(int value, ushort index, OperateType opType) => CommandStatus.NotSupported;
    public CommandStatus SelectAoI16(short value, ushort index) => CommandStatus.NotSupported;
    public CommandStatus OperateAoI16(short value, ushort index, OperateType opType) => CommandStatus.NotSupported;
    public CommandStatus SelectAoF32(float value, ushort index) => CommandStatus.NotSupported;
    public CommandStatus OperateAoF32(float value, ushort index, OperateType opType) => CommandStatus.NotSupported;
    public CommandStatus SelectAoF64(double value, ushort index) => CommandStatus.NotSupported;
    public CommandStatus OperateAoF64(double value, ushort index, OperateType opType) => CommandStatus.NotSupported;
}
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
ClassField allClasses = new ClassField(true, true, true, true);
ClassField noClasses = new ClassField(false, false, false, false);

Outstation outstation = new Outstation(
        new LinkConfig(false, (short) 10, (short) 1, 1000, 60_000, false),
        new OutstationParams(2048, 2048, false, 5000, 5000, 5000, 10_000, 16,
                new ClassField(false, true, true, true), false, 10),
        new DatabaseConfig((short) 3),
        new Commands());
Master master = new Master(
        new LinkConfig(true, (short) 1, (short) 10, 1000, 60_000, false),
        new MasterParams(5000, TimeSyncMode.NONE, false, false,
                noClasses, allClasses, true, noClasses, 2048, 2048),
        new Soe());

try (TcpServer server = TcpServer.listen("127.0.0.1", 0)) {
    try (TcpConnection conn = TcpConnection.connect("127.0.0.1", server.port());
            TcpConnection accepted = server.accept()) {
        outstation.run(accepted.stream());
        master.run(conn.stream());
    }
}

class Soe implements SoeHandler {
    public void onBinary(List<Binary> values) {}
    public void onDoubleBit(List<DoubleBitBinary> values) {}
    public void onBinaryOutputStatus(List<BinaryOutputStatus> values) {}
    public void onCounter(List<Counter> values) {}
    public void onFrozenCounter(List<FrozenCounter> values) {}
    public void onAnalog(List<Analog> values) {}
    public void onAnalogOutputStatus(List<AnalogOutputStatus> values) {}
    public void onOctetString(List<OctetString> values) {}
}

class Commands implements CommandHandler {
    public CommandStatus selectCrob(Crob command, short index) { return CommandStatus.SUCCESS; }
    public CommandStatus operateCrob(Crob command, short index, OperateType opType) { return CommandStatus.SUCCESS; }
    public CommandStatus selectAoI32(int value, short index) { return CommandStatus.NOT_SUPPORTED; }
    public CommandStatus operateAoI32(int value, short index, OperateType opType) { return CommandStatus.NOT_SUPPORTED; }
    public CommandStatus selectAoI16(short value, short index) { return CommandStatus.NOT_SUPPORTED; }
    public CommandStatus operateAoI16(short value, short index, OperateType opType) { return CommandStatus.NOT_SUPPORTED; }
    public CommandStatus selectAoF32(float value, short index) { return CommandStatus.NOT_SUPPORTED; }
    public CommandStatus operateAoF32(float value, short index, OperateType opType) { return CommandStatus.NOT_SUPPORTED; }
    public CommandStatus selectAoF64(double value, short index) { return CommandStatus.NOT_SUPPORTED; }
    public CommandStatus operateAoF64(double value, short index, OperateType opType) { return CommandStatus.NOT_SUPPORTED; }
}
```

Full programs that poll, operate a CROB, and scan are in [`opendnp3-ffi/examples`](opendnp3-ffi/examples).

## License

[MIT](LICENSE-MIT) or [Apache-2.0](LICENSE-APACHE), at your option. The SPDX identifier is `MIT OR Apache-2.0`.

The ported unit tests descend from [dnp3/opendnp3](https://github.com/dnp3/opendnp3), which is Apache-2.0. Copyright for that suite is Step Function I/O, LLC, licensed to Green Energy Corp and Step Function I/O LLC. See [NOTICE](NOTICE).
