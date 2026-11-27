package com.levyks.opendnp3.tcp;

import com.levyks.opendnp3.Dnp3Stream;
import com.levyks.opendnp3.StreamError;

import java.io.IOException;
import java.io.InputStream;
import java.io.OutputStream;
import java.net.Socket;
import java.util.concurrent.CompletableFuture;
import java.util.concurrent.Executor;
import java.util.concurrent.ExecutorService;
import java.util.concurrent.TimeUnit;

final class TcpDnp3Stream implements Dnp3Stream {
    private final Socket socket;
    private final InputStream in;
    private final OutputStream out;
    private final long startNanos = System.nanoTime();
    private final Executor io;
    private final ExecutorService ownedExecutor;

    TcpDnp3Stream(Socket socket) throws IOException {
        this(socket, null);
    }

    TcpDnp3Stream(Socket socket, Executor ioExecutor) throws IOException {
        socket.setTcpNoDelay(true);
        this.socket = socket;
        this.in = socket.getInputStream();
        this.out = socket.getOutputStream();
        if (ioExecutor != null) {
            this.io = ioExecutor;
            this.ownedExecutor = null;
        } else {
            this.ownedExecutor = TcpExecutors.newDaemonCachedPool();
            this.io = this.ownedExecutor;
        }
    }

    @Override
    public CompletableFuture<byte[]> read(int maxBytes) {
        CompletableFuture<byte[]> future = new CompletableFuture<>();
        io.execute(() -> {
            try {
                byte[] buf = new byte[Math.min(Math.max(maxBytes, 1), 65536)];
                int n = in.read(buf);
                if (n <= 0) {
                    future.completeExceptionally(StreamError.Closed.INSTANCE);
                    return;
                }
                byte[] data = new byte[n];
                System.arraycopy(buf, 0, data, 0, n);
                future.complete(data);
            } catch (IOException ex) {
                future.completeExceptionally(new StreamError.Io(String.valueOf(ex.getMessage())));
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
                future.completeExceptionally(new StreamError.Io(String.valueOf(ex.getMessage())));
            }
        });
        return future;
    }

    @Override
    public CompletableFuture<Void> sleepMs(long durationMs) {
        long clamped = Math.max(durationMs, 0L);
        return CompletableFuture.runAsync(
                () -> {},
                CompletableFuture.delayedExecutor(clamped, TimeUnit.MILLISECONDS));
    }

    @Override
    public long nowMs() {
        return (System.nanoTime() - startNanos) / 1_000_000L;
    }

    public void close() {
        if (ownedExecutor != null) {
            ownedExecutor.shutdownNow();
        }
        try {
            socket.close();
        } catch (IOException ignored) {
            // The protocol run has already finished or the peer hung up.
        }
    }
}
