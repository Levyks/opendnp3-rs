import com.levyks.opendnp3.Analog;
import com.levyks.opendnp3.AnalogOutputStatus;
import com.levyks.opendnp3.Binary;
import com.levyks.opendnp3.BinaryOutputStatus;
import com.levyks.opendnp3.ClassField;
import com.levyks.opendnp3.CommandHandler;
import com.levyks.opendnp3.CommandResult;
import com.levyks.opendnp3.CommandSet;
import com.levyks.opendnp3.CommandStatus;
import com.levyks.opendnp3.Counter;
import com.levyks.opendnp3.Crob;
import com.levyks.opendnp3.DatabaseConfig;
import com.levyks.opendnp3.DnpTime;
import com.levyks.opendnp3.DoubleBitBinary;
import com.levyks.opendnp3.EventMode;
import com.levyks.opendnp3.FrozenCounter;
import com.levyks.opendnp3.LinkConfig;
import com.levyks.opendnp3.Master;
import com.levyks.opendnp3.MasterEvent;
import com.levyks.opendnp3.MasterParams;
import com.levyks.opendnp3.MasterTaskType;
import com.levyks.opendnp3.OctetString;
import com.levyks.opendnp3.OperateType;
import com.levyks.opendnp3.OperationType;
import com.levyks.opendnp3.Outstation;
import com.levyks.opendnp3.OutstationParams;
import com.levyks.opendnp3.SoeHandler;
import com.levyks.opendnp3.TaskCompletion;
import com.levyks.opendnp3.TimeSyncMode;
import com.levyks.opendnp3.TimestampQuality;
import com.levyks.opendnp3.TripCloseCode;

import com.levyks.opendnp3.tcp.TcpConnection;
import com.levyks.opendnp3.tcp.TcpServer;

import java.util.ArrayList;
import java.util.List;
import java.util.Optional;
import java.util.concurrent.CompletableFuture;
import java.util.concurrent.TimeUnit;

/**
 * Real TCP playground for the generated Java bindings.
 *
 * Run from the repo root after {@code boltffi pack java --release} in {@code opendnp3-ffi/}:
 *
 * <pre>
 * mvn -f bindings/java/pom.xml -pl playground -am package
 * java --enable-native-access=ALL-UNNAMED -cp ^
 *   bindings/java/playground/target/opendnp3-playground-0.1.0.jar;^
 *   bindings/java/opendnp3-tcp/target/opendnp3-tcp-0.1.0.jar;^
 *   bindings/java/opendnp3/target/opendnp3-0.1.0.jar Playground
 * </pre>
 */
public final class Playground {
    public static void main(String[] args) throws Exception {
        ClassField allClasses = new ClassField(true, true, true, true);
        ClassField allEvents = new ClassField(false, true, true, true);
        ClassField noClasses = new ClassField(false, false, false, false);
        DnpTime invalidTime = new DnpTime(0, TimestampQuality.INVALID);

        DatabaseConfig db = new DatabaseConfig((short) 3);
        PlaygroundCommandHandler cmdHandler = new PlaygroundCommandHandler();
        Outstation outstation = new Outstation(
                new LinkConfig(false, (short) 10, (short) 1, 1000, 60_000, false),
                new OutstationParams(2048, 2048, false, 5000, 5000, 5000, 10_000, 16, allEvents, false, 10),
                db,
                cmdHandler);
        outstation.updateBinary(
                new Binary((short) 0, true, (byte) 0x81, invalidTime), EventMode.SUPPRESS);
        outstation.updateAnalog(
                new Analog((short) 2, 230.5, (byte) 0x01, invalidTime), EventMode.SUPPRESS);

        try (TcpServer listener = TcpServer.listen("127.0.0.1", 0)) {
            int port = listener.port();
            System.out.println("Outstation TCP server listening on 127.0.0.1:" + port);

            CompletableFuture<TcpConnection> accepted = CompletableFuture.supplyAsync(() -> {
                try {
                    return listener.accept();
                } catch (Exception ex) {
                    throw new IllegalStateException(ex);
                }
            });

            PlaygroundSoeHandler soe = new PlaygroundSoeHandler();
            Master master = new Master(
                    new LinkConfig(true, (short) 1, (short) 10, 1000, 60_000, false),
                    new MasterParams(
                            5000,
                            TimeSyncMode.NONE,
                            false,
                            false,
                            noClasses,
                            allClasses,
                            true,
                            noClasses,
                            2048,
                            2048),
                    soe);

            try (TcpConnection client = TcpConnection.connect("127.0.0.1", port);
                    TcpConnection server = accepted.join()) {
                CompletableFuture<Boolean> serverTask = outstation.run(server.stream());
                CompletableFuture<Boolean> masterTask = master.run(client.stream());
                serverTask.whenComplete((ok, err) -> {
                    if (err != null) {
                        System.out.println("Outstation run failed: " + err);
                    }
                });
                masterTask.whenComplete((ok, err) -> {
                    if (err != null) {
                        System.out.println("Master run failed: " + err);
                    }
                });

                System.out.println("Waiting for startup integrity poll...");
                while (true) {
                    MasterEvent ev = awaitEvent(master);
                    if (ev instanceof MasterEvent.TaskComplete tc
                            && tc.taskType == MasterTaskType.STARTUP_INTEGRITY_POLL) {
                        System.out.println("Startup integrity poll finished: " + tc.result);
                        if (tc.result != TaskCompletion.SUCCESS) {
                            throw new IllegalStateException("Expected SUCCESS, got " + tc.result);
                        }
                        break;
                    }
                }

                System.out.println("Sending SBO CROB command to index 1...");
                try (CommandSet cmds = new CommandSet()) {
                    cmds.addCrob(
                            (short) 1,
                            new Crob(
                                    OperationType.PULSE_ON,
                                    TripCloseCode.CLOSE,
                                    false,
                                    (byte) 1,
                                    100,
                                    100,
                                    CommandStatus.SUCCESS));
                    master.selectAndOperate(cmds);
                }

                while (true) {
                    MasterEvent ev = awaitEvent(master);
                    if (ev instanceof MasterEvent.CommandComplete cc) {
                        CommandResult result = cc.result;
                        System.out.println("Command complete: summary="
                                + result.summary
                                + ", points="
                                + result.points.size());
                        if (result.summary != TaskCompletion.SUCCESS
                                || result.points.get(0).status != CommandStatus.SUCCESS) {
                            throw new IllegalStateException("Command failed");
                        }
                        break;
                    }
                }

                System.out.println("Updating Analog[2] -> 241 and scanning...");
                outstation.updateAnalog(
                        new Analog((short) 2, 241.0, (byte) 0x01, invalidTime), EventMode.FORCE);
                master.scanClasses(allClasses);

                while (true) {
                    MasterEvent ev = awaitEvent(master);
                    System.out.println("  [Master Event] " + ev);
                    if (ev instanceof MasterEvent.TaskComplete tc) {
                        System.out.println("On-demand scan finished: " + tc.taskType + " -> " + tc.result);
                        if (tc.result != TaskCompletion.SUCCESS) {
                            throw new IllegalStateException("Expected SUCCESS, got " + tc.result);
                        }
                        break;
                    }
                }

                if (soe.binaries.stream().noneMatch(b -> b.index == 0 && b.value)) {
                    throw new IllegalStateException("Missing binary[0]=true");
                }
                if (soe.analogs.stream().noneMatch(a -> a.index == 2 && Math.abs(a.value - 230.0) < 1e-6)) {
                    throw new IllegalStateException("Missing analog[2]=230");
                }
                if (soe.analogs.stream().noneMatch(a -> a.index == 2 && Math.abs(a.value - 241.0) < 1e-6)) {
                    throw new IllegalStateException("Missing analog[2]=241");
                }
                if (cmdHandler.selected.size() != 1 || cmdHandler.selected.get(0) != 1) {
                    throw new IllegalStateException("Missing select");
                }
                if (cmdHandler.operated.size() != 1
                        || cmdHandler.operated.get(0).index != 1
                        || cmdHandler.operated.get(0).opType != OperateType.SELECT_BEFORE_OPERATE) {
                    throw new IllegalStateException("Missing operate");
                }

                master.shutdown();
                outstation.shutdown();
                masterTask.get(10, TimeUnit.SECONDS);
                serverTask.get(10, TimeUnit.SECONDS);
                master.close();
                outstation.close();
            }
        } finally {
            db.close();
        }

        System.out.println();
        System.out.println("ALL JAVA DNP3 CHECKS PASSED!");
    }

    private static MasterEvent awaitEvent(Master master) throws Exception {
        Optional<MasterEvent> event = master.nextEvent().get(20, TimeUnit.SECONDS);
        if (event.isEmpty()) {
            throw new IllegalStateException("Master event stream ended");
        }
        return event.get();
    }

    private Playground() {}

    static final class PlaygroundSoeHandler implements SoeHandler {
        final List<Binary> binaries = new ArrayList<>();
        final List<Analog> analogs = new ArrayList<>();

        @Override
        public void onBinary(List<Binary> values) {
            for (Binary value : values) {
                System.out.printf(
                        "  [Master SOE] Binary[%d] = %s (flags=0x%02x)%n",
                        value.index,
                        value.value,
                        value.flags);
            }
            binaries.addAll(values);
        }

        @Override
        public void onDoubleBit(List<DoubleBitBinary> values) {}

        @Override
        public void onBinaryOutputStatus(List<BinaryOutputStatus> values) {}

        @Override
        public void onCounter(List<Counter> values) {}

        @Override
        public void onFrozenCounter(List<FrozenCounter> values) {}

        @Override
        public void onAnalog(List<Analog> values) {
            for (Analog value : values) {
                System.out.printf(
                        "  [Master SOE] Analog[%d] = %s (flags=0x%02x)%n",
                        value.index,
                        value.value,
                        value.flags);
            }
            analogs.addAll(values);
        }

        @Override
        public void onAnalogOutputStatus(List<AnalogOutputStatus> values) {}

        @Override
        public void onOctetString(List<OctetString> values) {}
    }

    static final class Operated {
        final short index;
        final OperateType opType;

        Operated(short index, OperateType opType) {
            this.index = index;
            this.opType = opType;
        }
    }

    static final class PlaygroundCommandHandler implements CommandHandler {
        final List<Short> selected = new ArrayList<>();
        final List<Operated> operated = new ArrayList<>();

        @Override
        public CommandStatus selectCrob(Crob command, short index) {
            System.out.println("  [Outstation CMD] SELECT CROB index=" + index + " op=" + command.opType);
            selected.add(index);
            return CommandStatus.SUCCESS;
        }

        @Override
        public CommandStatus operateCrob(Crob command, short index, OperateType opType) {
            System.out.println(
                    "  [Outstation CMD] OPERATE CROB index=" + index + " op=" + command.opType + " mode=" + opType);
            operated.add(new Operated(index, opType));
            return CommandStatus.SUCCESS;
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
}
