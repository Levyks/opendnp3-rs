using System.Net;
using System.Net.Sockets;
using Levyks.OpenDnp3;
using Levyks.OpenDnp3.Tcp;

var allClasses = new FfiClassField(true, true, true, true);
var allEvents = new FfiClassField(false, true, true, true);
var noClasses = new FfiClassField(false, false, false, false);
var invalidTime = new FfiDnpTime(0, FfiTimestampQuality.Invalid);

// 1. Outstation + TCP server
using var db = FfiDatabaseConfig.AllTypes(3);
var cmdHandler = new PlaygroundCommandHandler();
using var outstation = new FfiOutstation(
    new FfiLinkConfig(false, 10, 1, 1000, 60_000, false),
    new FfiOutstationParams(2048, 2048, false, 5000, 5000, 5000, 10_000, 16, allEvents, false, 10),
    db,
    cmdHandler
);

outstation.UpdateBinary(new FfiBinary(0, true, 0x81, invalidTime), FfiEventMode.Suppress);
outstation.UpdateAnalog(new FfiAnalog(2, 230.5, 0x01, invalidTime), FfiEventMode.Suppress);

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
using var master = new FfiMaster(
    new FfiLinkConfig(true, 1, 10, 1000, 60_000, false),
    new FfiMasterParams(5000, FfiTimeSyncMode.None, false, false, noClasses, allClasses, true, noClasses, 2048, 2048),
    soe
);

await using var client = await TcpConnection.ConnectAsync("127.0.0.1", port);
var masterTask = master.Run(client.Stream);

// 3. Startup integrity poll
Console.WriteLine("Waiting for startup integrity poll...");
while (true)
{
    var ev = await master.NextEvent();
    if (ev is FfiMasterEvent.TaskComplete { TaskType: FfiMasterTaskType.StartupIntegrityPoll } tc)
    {
        Console.WriteLine($"Startup integrity poll finished: {tc.Result}");
        if (tc.Result != FfiTaskCompletion.Success)
            throw new Exception($"Expected Success, got {tc.Result}");
        break;
    }
}

// 4. Select-Before-Operate CROB command
Console.WriteLine("Sending SBO CROB command to index 1...");
using var cmds = new FfiCommandSet();
cmds.AddCrob(1, new FfiCrob(FfiOperationType.PulseOn, FfiTripCloseCode.Close, false, 1, 100, 100, FfiCommandStatus.Success));
master.SelectAndOperate(cmds);

while (true)
{
    var ev = await master.NextEvent();
    if (ev is FfiMasterEvent.CommandComplete cc)
    {
        Console.WriteLine($"Command complete: summary={cc.Result.Summary}, points={cc.Result.Points.Length}");
        if (cc.Result.Summary != FfiTaskCompletion.Success || cc.Result.Points[0].Status != FfiCommandStatus.Success)
            throw new Exception("Command failed");
        break;
    }
}

// 5. Live update + on-demand scan
Console.WriteLine("Updating Analog[2] -> 241 and scanning...");
outstation.UpdateAnalog(new FfiAnalog(2, 241.0, 0x01, invalidTime), FfiEventMode.Force);
master.ScanClasses(allClasses);

while (true)
{
    var ev = await master.NextEvent();
    Console.WriteLine($"  [Master Event] {ev}");
    if (ev is FfiMasterEvent.TaskComplete tc)
    {
        Console.WriteLine($"On-demand scan finished: {tc.TaskType} -> {tc.Result}");
        if (tc.Result != FfiTaskCompletion.Success)
            throw new Exception($"Expected Success, got {tc.Result}");
        break;
    }
}

// Verify results
if (!soe.Binaries.Any(b => b.Index == 0 && b.Value)) throw new Exception("Missing binary[0]=true");
if (!soe.Analogs.Any(a => a.Index == 2 && Math.Abs(a.Value - 230.0) < 1e-6)) throw new Exception("Missing analog[2]=230");
if (!soe.Analogs.Any(a => a.Index == 2 && Math.Abs(a.Value - 241.0) < 1e-6)) throw new Exception("Missing analog[2]=241");
if (cmdHandler.Selected.Count != 1 || cmdHandler.Selected[0] != 1) throw new Exception("Missing select");
if (cmdHandler.Operated.Count != 1 || cmdHandler.Operated[0] != (1, FfiOperateType.SelectBeforeOperate)) throw new Exception("Missing operate");

master.Shutdown();
outstation.Shutdown();
await Task.WhenAll(masterTask, serverTask);
listener.Stop();

Console.WriteLine("\nALL .NET 10 DNP3 CHECKS PASSED!");

sealed class PlaygroundSoeHandler : FfiSoeHandler
{
    public List<FfiBinary> Binaries { get; } = [];
    public List<FfiAnalog> Analogs { get; } = [];

    public void OnBinary(FfiBinary[] values)
    {
        foreach (var v in values)
            Console.WriteLine($"  [Master SOE] Binary[{v.Index}] = {v.Value} (flags=0x{v.Flags:x2})");
        Binaries.AddRange(values);
    }

    public void OnDoubleBit(FfiDoubleBitBinary[] values) { }
    public void OnBinaryOutputStatus(FfiBinaryOutputStatus[] values) { }
    public void OnCounter(FfiCounter[] values) { }
    public void OnFrozenCounter(FfiFrozenCounter[] values) { }

    public void OnAnalog(FfiAnalog[] values)
    {
        foreach (var v in values)
            Console.WriteLine($"  [Master SOE] Analog[{v.Index}] = {v.Value} (flags=0x{v.Flags:x2})");
        Analogs.AddRange(values);
    }

    public void OnAnalogOutputStatus(FfiAnalogOutputStatus[] values) { }
    public void OnOctetString(FfiOctetString[] values) { }
}

sealed class PlaygroundCommandHandler : FfiCommandHandler
{
    public List<ushort> Selected { get; } = [];
    public List<(ushort Index, FfiOperateType OpType)> Operated { get; } = [];

    public FfiCommandStatus SelectCrob(FfiCrob command, ushort index)
    {
        Console.WriteLine($"  [Outstation CMD] SELECT CROB index={index} op={command.OpType}");
        Selected.Add(index);
        return FfiCommandStatus.Success;
    }

    public FfiCommandStatus OperateCrob(FfiCrob command, ushort index, FfiOperateType opType)
    {
        Console.WriteLine($"  [Outstation CMD] OPERATE CROB index={index} op={command.OpType} mode={opType}");
        Operated.Add((index, opType));
        return FfiCommandStatus.Success;
    }

    public FfiCommandStatus SelectAoI32(int value, ushort index) => FfiCommandStatus.NotSupported;
    public FfiCommandStatus OperateAoI32(int value, ushort index, FfiOperateType opType) => FfiCommandStatus.NotSupported;
    public FfiCommandStatus SelectAoI16(short value, ushort index) => FfiCommandStatus.NotSupported;
    public FfiCommandStatus OperateAoI16(short value, ushort index, FfiOperateType opType) => FfiCommandStatus.NotSupported;
    public FfiCommandStatus SelectAoF32(float value, ushort index) => FfiCommandStatus.NotSupported;
    public FfiCommandStatus OperateAoF32(float value, ushort index, FfiOperateType opType) => FfiCommandStatus.NotSupported;
    public FfiCommandStatus SelectAoF64(double value, ushort index) => FfiCommandStatus.NotSupported;
    public FfiCommandStatus OperateAoF64(double value, ushort index, FfiOperateType opType) => FfiCommandStatus.NotSupported;
}
