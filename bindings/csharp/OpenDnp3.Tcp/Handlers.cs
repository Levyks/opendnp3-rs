using Levyks.OpenDnp3;

namespace Levyks.OpenDnp3.Tcp;

/// <summary>
/// No-op base class for <see cref="SoeHandler"/>. Override only the measurement types you care about.
/// </summary>
public class SoeHandlerBase : SoeHandler
{
    public virtual void OnBinary(Binary[] values) { }
    public virtual void OnDoubleBit(DoubleBitBinary[] values) { }
    public virtual void OnBinaryOutputStatus(BinaryOutputStatus[] values) { }
    public virtual void OnCounter(Counter[] values) { }
    public virtual void OnFrozenCounter(FrozenCounter[] values) { }
    public virtual void OnAnalog(Analog[] values) { }
    public virtual void OnAnalogOutputStatus(AnalogOutputStatus[] values) { }
    public virtual void OnOctetString(OctetString[] values) { }
}

/// <summary>
/// Default-reject base class for <see cref="CommandHandler"/>. Override only the commands you support.
/// </summary>
public class CommandHandlerBase : CommandHandler
{
    public virtual CommandStatus SelectCrob(Crob command, ushort index) => CommandStatus.NotSupported;
    public virtual CommandStatus OperateCrob(Crob command, ushort index, OperateType opType) => CommandStatus.NotSupported;
    public virtual CommandStatus SelectAoI32(int value, ushort index) => CommandStatus.NotSupported;
    public virtual CommandStatus OperateAoI32(int value, ushort index, OperateType opType) => CommandStatus.NotSupported;
    public virtual CommandStatus SelectAoI16(short value, ushort index) => CommandStatus.NotSupported;
    public virtual CommandStatus OperateAoI16(short value, ushort index, OperateType opType) => CommandStatus.NotSupported;
    public virtual CommandStatus SelectAoF32(float value, ushort index) => CommandStatus.NotSupported;
    public virtual CommandStatus OperateAoF32(float value, ushort index, OperateType opType) => CommandStatus.NotSupported;
    public virtual CommandStatus SelectAoF64(double value, ushort index) => CommandStatus.NotSupported;
    public virtual CommandStatus OperateAoF64(double value, ushort index, OperateType opType) => CommandStatus.NotSupported;
}
