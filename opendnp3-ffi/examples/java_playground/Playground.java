import com.levyks.opendnp3.FfiAnalog;
import com.levyks.opendnp3.FfiAnalogOutputStatus;
import com.levyks.opendnp3.FfiBinary;
import com.levyks.opendnp3.FfiBinaryOutputStatus;
import com.levyks.opendnp3.FfiClassField;
import com.levyks.opendnp3.FfiCommandHandler;
import com.levyks.opendnp3.FfiCommandResult;
import com.levyks.opendnp3.FfiCommandSet;
import com.levyks.opendnp3.FfiCommandStatus;
import com.levyks.opendnp3.FfiCounter;
import com.levyks.opendnp3.FfiCrob;
import com.levyks.opendnp3.FfiDatabaseConfig;
import com.levyks.opendnp3.FfiDnpTime;
import com.levyks.opendnp3.FfiDoubleBitBinary;
import com.levyks.opendnp3.FfiEventMode;
import com.levyks.opendnp3.FfiFrozenCounter;
import com.levyks.opendnp3.FfiLinkConfig;
import com.levyks.opendnp3.FfiMaster;
import com.levyks.opendnp3.FfiMasterEvent;
import com.levyks.opendnp3.FfiMasterParams;
import com.levyks.opendnp3.FfiMasterTaskType;
import com.levyks.opendnp3.FfiOctetString;
import com.levyks.opendnp3.FfiOperateType;
import com.levyks.opendnp3.FfiOperationType;
import com.levyks.opendnp3.FfiOutstation;
import com.levyks.opendnp3.FfiOutstationParams;
import com.levyks.opendnp3.FfiSoeHandler;
import com.levyks.opendnp3.FfiTaskCompletion;
import com.levyks.opendnp3.FfiTimeSyncMode;
import com.levyks.opendnp3.FfiTimestampQuality;
import com.levyks.opendnp3.FfiTripCloseCode;

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
        FfiClassField allClasses = new FfiClassField(true, true, true, true);
        FfiClassField allEvents = new FfiClassField(false, true, true, true);
        FfiClassField noClasses = new FfiClassField(false, false, false, false);
        FfiDnpTime invalidTime = new FfiDnpTime(0, FfiTimestampQuality.INVALID);

        FfiDatabaseConfig db = new FfiDatabaseConfig((short) 3);
        PlaygroundCommandHandler cmdHandler = new PlaygroundCommandHandler();
        FfiOutstation outstation = new FfiOutstation(
                new FfiLinkConfig(false, (short) 10, (short) 1, 1000, 60_000, false),
                new FfiOutstationParams(2048, 2048, false, 5000, 5000, 5000, 10_000, 16, allEvents, false, 10),
                db,
                cmdHandler);
        outstation.updateBinary(
                new FfiBinary((short) 0, true, (byte) 0x81, invalidTime), FfiEventMode.SUPPRESS);
        outstation.updateAnalog(
                new FfiAnalog((short) 2, 230.5, (byte) 0x01, invalidTime), FfiEventMode.SUPPRESS);

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
            FfiMaster master = new FfiMaster(
                    new FfiLinkConfig(true, (short) 1, (short) 10, 1000, 60_000, false),
                    new FfiMasterParams(
                            5000,
                            FfiTimeSyncMode.NONE,
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
                    FfiMasterEvent ev = awaitEvent(master);
                    if (ev instanceof FfiMasterEvent.TaskComplete tc
                            && tc.taskType == FfiMasterTaskType.STARTUP_INTEGRITY_POLL) {
                        System.out.println("Startup integrity poll finished: " + tc.result);
                        if (tc.result != FfiTaskCompletion.SUCCESS) {
                            throw new IllegalStateException("Expected SUCCESS, got " + tc.result);
                        }
                        break;
                    }
                }

                System.out.println("Sending SBO CROB command to index 1...");
                try (FfiCommandSet cmds = new FfiCommandSet()) {
                    cmds.addCrob(
                            (short) 1,
                            new FfiCrob(
                                    FfiOperationType.PULSE_ON,
                                    FfiTripCloseCode.CLOSE,
                                    false,
                                    (byte) 1,
                                    100,
                                    100,
                                    FfiCommandStatus.SUCCESS));
                    master.selectAndOperate(cmds);
                }

                while (true) {
                    FfiMasterEvent ev = awaitEvent(master);
                    if (ev instanceof FfiMasterEvent.CommandComplete cc) {
                        FfiCommandResult result = cc.result;
                        System.out.println("Command complete: summary="
                                + result.summary
                                + ", points="
                                + result.points.size());
                        if (result.summary != FfiTaskCompletion.SUCCESS
                                || result.points.get(0).status != FfiCommandStatus.SUCCESS) {
                            throw new IllegalStateException("Command failed");
                        }
                        break;
                    }
                }

                System.out.println("Updating Analog[2] -> 241 and scanning...");
                outstation.updateAnalog(
                        new FfiAnalog((short) 2, 241.0, (byte) 0x01, invalidTime), FfiEventMode.FORCE);
                master.scanClasses(allClasses);

                while (true) {
                    FfiMasterEvent ev = awaitEvent(master);
                    System.out.println("  [Master Event] " + ev);
                    if (ev instanceof FfiMasterEvent.TaskComplete tc) {
                        System.out.println("On-demand scan finished: " + tc.taskType + " -> " + tc.result);
                        if (tc.result != FfiTaskCompletion.SUCCESS) {
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
                        || cmdHandler.operated.get(0).opType != FfiOperateType.SELECT_BEFORE_OPERATE) {
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

    private static FfiMasterEvent awaitEvent(FfiMaster master) throws Exception {
        Optional<FfiMasterEvent> event = master.nextEvent().get(20, TimeUnit.SECONDS);
        if (event.isEmpty()) {
            throw new IllegalStateException("Master event stream ended");
        }
        return event.get();
    }

    private Playground() {}

    static final class PlaygroundSoeHandler implements FfiSoeHandler {
        final List<FfiBinary> binaries = new ArrayList<>();
        final List<FfiAnalog> analogs = new ArrayList<>();

        @Override
        public void onBinary(List<FfiBinary> values) {
            for (FfiBinary value : values) {
                System.out.printf(
                        "  [Master SOE] Binary[%d] = %s (flags=0x%02x)%n",
                        value.index,
                        value.value,
                        value.flags);
            }
            binaries.addAll(values);
        }

        @Override
        public void onDoubleBit(List<FfiDoubleBitBinary> values) {}

        @Override
        public void onBinaryOutputStatus(List<FfiBinaryOutputStatus> values) {}

        @Override
        public void onCounter(List<FfiCounter> values) {}

        @Override
        public void onFrozenCounter(List<FfiFrozenCounter> values) {}

        @Override
        public void onAnalog(List<FfiAnalog> values) {
            for (FfiAnalog value : values) {
                System.out.printf(
                        "  [Master SOE] Analog[%d] = %s (flags=0x%02x)%n",
                        value.index,
                        value.value,
                        value.flags);
            }
            analogs.addAll(values);
        }

        @Override
        public void onAnalogOutputStatus(List<FfiAnalogOutputStatus> values) {}

        @Override
        public void onOctetString(List<FfiOctetString> values) {}
    }

    static final class Operated {
        final short index;
        final FfiOperateType opType;

        Operated(short index, FfiOperateType opType) {
            this.index = index;
            this.opType = opType;
        }
    }

    static final class PlaygroundCommandHandler implements FfiCommandHandler {
        final List<Short> selected = new ArrayList<>();
        final List<Operated> operated = new ArrayList<>();

        @Override
        public FfiCommandStatus selectCrob(FfiCrob command, short index) {
            System.out.println("  [Outstation CMD] SELECT CROB index=" + index + " op=" + command.opType);
            selected.add(index);
            return FfiCommandStatus.SUCCESS;
        }

        @Override
        public FfiCommandStatus operateCrob(FfiCrob command, short index, FfiOperateType opType) {
            System.out.println(
                    "  [Outstation CMD] OPERATE CROB index=" + index + " op=" + command.opType + " mode=" + opType);
            operated.add(new Operated(index, opType));
            return FfiCommandStatus.SUCCESS;
        }

        @Override
        public FfiCommandStatus selectAoI32(int value, short index) {
            return FfiCommandStatus.NOT_SUPPORTED;
        }

        @Override
        public FfiCommandStatus operateAoI32(int value, short index, FfiOperateType opType) {
            return FfiCommandStatus.NOT_SUPPORTED;
        }

        @Override
        public FfiCommandStatus selectAoI16(short value, short index) {
            return FfiCommandStatus.NOT_SUPPORTED;
        }

        @Override
        public FfiCommandStatus operateAoI16(short value, short index, FfiOperateType opType) {
            return FfiCommandStatus.NOT_SUPPORTED;
        }

        @Override
        public FfiCommandStatus selectAoF32(float value, short index) {
            return FfiCommandStatus.NOT_SUPPORTED;
        }

        @Override
        public FfiCommandStatus operateAoF32(float value, short index, FfiOperateType opType) {
            return FfiCommandStatus.NOT_SUPPORTED;
        }

        @Override
        public FfiCommandStatus selectAoF64(double value, short index) {
            return FfiCommandStatus.NOT_SUPPORTED;
        }

        @Override
        public FfiCommandStatus operateAoF64(double value, short index, FfiOperateType opType) {
            return FfiCommandStatus.NOT_SUPPORTED;
        }
    }
}
