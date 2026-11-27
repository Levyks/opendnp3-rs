package com.levyks.opendnp3.tcp;

import com.levyks.opendnp3.CommandHandler;
import com.levyks.opendnp3.CommandStatus;
import com.levyks.opendnp3.Crob;
import com.levyks.opendnp3.OperateType;

/** Default-reject base class for {@link CommandHandler}. Override only the commands you support. */
public class CommandHandlerBase implements CommandHandler {
    @Override
    public CommandStatus selectCrob(Crob command, short index) {
        return CommandStatus.NOT_SUPPORTED;
    }

    @Override
    public CommandStatus operateCrob(Crob command, short index, OperateType opType) {
        return CommandStatus.NOT_SUPPORTED;
    }

    @Override
    public CommandStatus selectAoI32(int value, short index) {
        return CommandStatus.NOT_SUPPORTED;
    }

    @Override
    public CommandStatus operateAoI32(int value, short index, OperateType opType) {
        return CommandStatus.NOT_SUPPORTED;
    }

    @Override
    public CommandStatus selectAoI16(short value, short index) {
        return CommandStatus.NOT_SUPPORTED;
    }

    @Override
    public CommandStatus operateAoI16(short value, short index, OperateType opType) {
        return CommandStatus.NOT_SUPPORTED;
    }

    @Override
    public CommandStatus selectAoF32(float value, short index) {
        return CommandStatus.NOT_SUPPORTED;
    }

    @Override
    public CommandStatus operateAoF32(float value, short index, OperateType opType) {
        return CommandStatus.NOT_SUPPORTED;
    }

    @Override
    public CommandStatus selectAoF64(double value, short index) {
        return CommandStatus.NOT_SUPPORTED;
    }

    @Override
    public CommandStatus operateAoF64(double value, short index, OperateType opType) {
        return CommandStatus.NOT_SUPPORTED;
    }
}
