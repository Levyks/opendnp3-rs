package com.levyks.opendnp3.tcp;

import com.levyks.opendnp3.Dnp3Stream;
import com.levyks.opendnp3.FfiStreamError;

import java.io.IOException;
import java.io.InputStream;
import java.io.OutputStream;
import java.net.Socket;
import java.util.concurrent.CompletableFuture;
import java.util.concurrent.ExecutorService;
import java.util.concurrent.Executors;

final class TcpDnp3Stream implements Dnp3Stream {
    private final Socket socket;
    private final InputStream in;
    private final OutputStream out;
    private final long startNanos = System.nanoTime();
    private final ExecutorService io = Executors.newCachedThreadPool(runnable -> {
        Thread thread = new Thread(runnable, "opendnp3-tcp");
        thread.setDaemon(true);
        return thread;
    });

    TcpDnp3Stream(Socket socket) throws IOException {
        this.socket = socket;
        this.in = socket.getInputStream();
        this.out = socket.getOutputStream();
    }

    @Override
    public CompletableFuture<byte[]> read(int maxBytes) {
        CompletableFuture<byte[]> future = new CompletableFuture<>();
        io.execute(() -> {
            try {
                byte[] buf = new byte[Math.min(Math.max(maxBytes, 1), 65536)];
                int n = in.read(buf);
                if (n <= 0) {
                    future.completeExceptionally(FfiStreamError.Closed.INSTANCE);
                    return;
                }
                byte[] data = new byte[n];
                System.arraycopy(buf, 0, data, 0, n);
                future.complete(data);
            } catch (IOException ex) {
                future.completeExceptionally(new FfiStreamError.Io(String.valueOf(ex.getMessage())));
            }
        });
        return future;
    }

    @Override
    public CompletableFuture<Void> write(byte[] data) {
        CompletableFuture<Void> future = new CompletableFuture<>();
        io.execute(() -> {
            try {
                out.write(data);
                out.flush();
                future.complete(null);
            } catch (IOException ex) {
                future.completeExceptionally(new FfiStreamError.Io(String.valueOf(ex.getMessage())));
            }
        });
        return future;
    }

    @Override
    public CompletableFuture<Void> sleepMs(long durationMs) {
        CompletableFuture<Void> future = new CompletableFuture<>();
        io.execute(() -> {
            try {
                Thread.sleep(Math.min(Math.max(durationMs, 0), Integer.MAX_VALUE));
                future.complete(null);
            } catch (InterruptedException ex) {
                Thread.currentThread().interrupt();
                future.completeExceptionally(ex);
            }
        });
        return future;
    }

    @Override
    public long nowMs() {
        return (System.nanoTime() - startNanos) / 1_000_000L;
    }

    public void close() {
        io.shutdownNow();
        try {
            socket.close();
        } catch (IOException ignored) {
            // The protocol run has already finished or the peer hung up.
        }
    }
}
