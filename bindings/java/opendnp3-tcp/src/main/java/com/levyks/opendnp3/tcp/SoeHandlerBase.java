package com.levyks.opendnp3.tcp;

import com.levyks.opendnp3.Analog;
import com.levyks.opendnp3.AnalogOutputStatus;
import com.levyks.opendnp3.Binary;
import com.levyks.opendnp3.BinaryOutputStatus;
import com.levyks.opendnp3.Counter;
import com.levyks.opendnp3.DoubleBitBinary;
import com.levyks.opendnp3.FrozenCounter;
import com.levyks.opendnp3.OctetString;
import com.levyks.opendnp3.SoeHandler;

import java.util.List;

/** No-op base class for {@link SoeHandler}. Override only the measurement types you care about. */
public class SoeHandlerBase implements SoeHandler {
    @Override
    public void onBinary(List<Binary> values) {}

    @Override
    public void onDoubleBit(List<DoubleBitBinary> values) {}

    @Override
    public void onBinaryOutputStatus(List<BinaryOutputStatus> values) {}

    @Override
    public void onCounter(List<Counter> values) {}

    @Override
    public void onFrozenCounter(List<FrozenCounter> values) {}

    @Override
    public void onAnalog(List<Analog> values) {}

    @Override
    public void onAnalogOutputStatus(List<AnalogOutputStatus> values) {}

    @Override
    public void onOctetString(List<OctetString> values) {}
}
