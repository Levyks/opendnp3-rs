using System.Net;
using System.Net.Sockets;
using Levyks.OpenDnp3;
using Levyks.OpenDnp3.Tcp;

var allClasses = new ClassField(true, true, true, true);
var allEvents = new ClassField(false, true, true, true);
var noClasses = new ClassField(false, false, false, false);
var invalidTime = new DnpTime(0, TimestampQuality.Invalid);

// 1. Outstation + TCP server
using var db = DatabaseConfig.AllTypes(3);
var cmdHandler = new PlaygroundCommandHandler();
using var outstation = new Outstation(
    new LinkConfig(false, 10, 1, 1000, 60_000, false),
    new OutstationParams(2048, 2048, false, 5000, 5000, 5000, 10_000, 16, allEvents, false, 10),
    db,
    cmdHandler
);

outstation.UpdateBinary(new Binary(0, true, 0x81, invalidTime), EventMode.Suppress);
outstation.UpdateAnalog(new Analog(2, 230.5, 0x01, invalidTime), EventMode.Suppress);

var listener = TcpServer.Listen(IPAddress.Loopback);
int port = ((IPEndPoint)listener.LocalEndpoint).Port;
Console.WriteLine($"Outstation TCP server listening on 127.0.0.1:{port}");

var serverTask = Task.Run(async () =>
{
    await using var accepted = await TcpServer.AcceptAsync(listener);
    await outstation.Run(accepted.Stream);
});

// 2. Master + TCP client
var soe = new PlaygroundSoeHandler();
using var master = new Master(
    new LinkConfig(true, 1, 10, 1000, 60_000, false),
    new MasterParams(5000, TimeSyncMode.None, false, false, noClasses, allClasses, true, noClasses, 2048, 2048),
    soe
);

await using var client = await TcpConnection.ConnectAsync("127.0.0.1", port);
var masterTask = master.Run(client.Stream);

// 3. Startup integrity poll
Console.WriteLine("Waiting for startup integrity poll...");
while (true)
{
    var ev = await master.NextEvent();
    if (ev is MasterEvent.TaskComplete { TaskType: MasterTaskType.StartupIntegrityPoll } tc)
    {
        Console.WriteLine($"Startup integrity poll finished: {tc.Result}");
        if (tc.Result != TaskCompletion.Success)
            throw new Exception($"Expected Success, got {tc.Result}");
        break;
    }
}

// 4. Select-Before-Operate CROB command
Console.WriteLine("Sending SBO CROB command to index 1...");
using var cmds = new CommandSet();
cmds.AddCrob(1, new Crob(OperationType.PulseOn, TripCloseCode.Close, false, 1, 100, 100, CommandStatus.Success));
master.SelectAndOperate(cmds);

while (true)
{
    var ev = await master.NextEvent();
    if (ev is MasterEvent.CommandComplete cc)
    {
        Console.WriteLine($"Command complete: summary={cc.Result.Summary}, points={cc.Result.Points.Length}");
        if (cc.Result.Summary != TaskCompletion.Success || cc.Result.Points[0].Status != CommandStatus.Success)
            throw new Exception("Command failed");
        break;
    }
}

// 5. Live update + on-demand scan
Console.WriteLine("Updating Analog[2] -> 241 and scanning...");
outstation.UpdateAnalog(new Analog(2, 241.0, 0x01, invalidTime), EventMode.Force);
master.ScanClasses(allClasses);

while (true)
{
    var ev = await master.NextEvent();
    Console.WriteLine($"  [Master Event] {ev}");
    if (ev is MasterEvent.TaskComplete tc)
    {
        Console.WriteLine($"On-demand scan finished: {tc.TaskType} -> {tc.Result}");
        if (tc.Result != TaskCompletion.Success)
            throw new Exception($"Expected Success, got {tc.Result}");
        break;
    }
}

// Verify results
if (!soe.Binaries.Any(b => b.Index == 0 && b.Value)) throw new Exception("Missing binary[0]=true");
if (!soe.Analogs.Any(a => a.Index == 2 && Math.Abs(a.Value - 230.0) < 1e-6)) throw new Exception("Missing analog[2]=230");
if (!soe.Analogs.Any(a => a.Index == 2 && Math.Abs(a.Value - 241.0) < 1e-6)) throw new Exception("Missing analog[2]=241");
if (cmdHandler.Selected.Count != 1 || cmdHandler.Selected[0] != 1) throw new Exception("Missing select");
if (cmdHandler.Operated.Count != 1 || cmdHandler.Operated[0] != (1, OperateType.SelectBeforeOperate)) throw new Exception("Missing operate");

master.Shutdown();
outstation.Shutdown();
await Task.WhenAll(masterTask, serverTask);
listener.Stop();

Console.WriteLine("\nALL .NET 10 DNP3 CHECKS PASSED!");

sealed class PlaygroundSoeHandler : SoeHandler
{
    public List<Binary> Binaries { get; } = [];
    public List<Analog> Analogs { get; } = [];

    public void OnBinary(Binary[] values)
    {
        foreach (var v in values)
            Console.WriteLine($"  [Master SOE] Binary[{v.Index}] = {v.Value} (flags=0x{v.Flags:x2})");
        Binaries.AddRange(values);
    }

    public void OnDoubleBit(DoubleBitBinary[] values) { }
    public void OnBinaryOutputStatus(BinaryOutputStatus[] values) { }
    public void OnCounter(Counter[] values) { }
    public void OnFrozenCounter(FrozenCounter[] values) { }

    public void OnAnalog(Analog[] values)
    {
        foreach (var v in values)
            Console.WriteLine($"  [Master SOE] Analog[{v.Index}] = {v.Value} (flags=0x{v.Flags:x2})");
        Analogs.AddRange(values);
    }

    public void OnAnalogOutputStatus(AnalogOutputStatus[] values) { }
    public void OnOctetString(OctetString[] values) { }
}

sealed class PlaygroundCommandHandler : CommandHandler
{
    public List<ushort> Selected { get; } = [];
    public List<(ushort Index, OperateType OpType)> Operated { get; } = [];

    public CommandStatus SelectCrob(Crob command, ushort index)
    {
        Console.WriteLine($"  [Outstation CMD] SELECT CROB index={index} op={command.OpType}");
        Selected.Add(index);
        return CommandStatus.Success;
    }

    public CommandStatus OperateCrob(Crob command, ushort index, OperateType opType)
    {
        Console.WriteLine($"  [Outstation CMD] OPERATE CROB index={index} op={command.OpType} mode={opType}");
        Operated.Add((index, opType));
        return CommandStatus.Success;
    }

    public CommandStatus SelectAoI32(int value, ushort index) => CommandStatus.NotSupported;
    public CommandStatus OperateAoI32(int value, ushort index, OperateType opType) => CommandStatus.NotSupported;
    public CommandStatus SelectAoI16(short value, ushort index) => CommandStatus.NotSupported;
    public CommandStatus OperateAoI16(short value, ushort index, OperateType opType) => CommandStatus.NotSupported;
    public CommandStatus SelectAoF32(float value, ushort index) => CommandStatus.NotSupported;
    public CommandStatus OperateAoF32(float value, ushort index, OperateType opType) => CommandStatus.NotSupported;
    public CommandStatus SelectAoF64(double value, ushort index) => CommandStatus.NotSupported;
    public CommandStatus OperateAoF64(double value, ushort index, OperateType opType) => CommandStatus.NotSupported;
}
