package com.levyks.opendnp3.tcp;

import com.levyks.opendnp3.Dnp3Stream;

import java.io.IOException;
import java.net.InetAddress;
import java.net.Socket;
import java.util.concurrent.Executor;

/** One connected TCP socket and the stream the master or outstation runs on. */
public final class TcpConnection implements AutoCloseable {
    private final Socket socket;
    private final TcpDnp3Stream stream;

    private TcpConnection(Socket socket, Executor ioExecutor) throws IOException {
        this.socket = socket;
        this.stream = new TcpDnp3Stream(socket, ioExecutor);
    }

    public static TcpConnection connect(String host, int port) throws IOException {
        return connect(host, port, null);
    }

    /**
     * Connect using a caller-supplied {@link Executor} (for example,
     * {@code Executors.newVirtualThreadPerTaskExecutor()} on Java 21+ or
     * {@link TcpExecutors#virtualThreadsOrCached()}). The caller owns the executor's lifecycle.
     */
    public static TcpConnection connect(String host, int port, Executor ioExecutor) throws IOException {
        return new TcpConnection(new Socket(InetAddress.getByName(host), port), ioExecutor);
    }

    static TcpConnection accepted(Socket socket, Executor ioExecutor) throws IOException {
        return new TcpConnection(socket, ioExecutor);
    }

    public Dnp3Stream stream() {
        return stream;
    }

    @Override
    public void close() {
        stream.close();
    }
}
