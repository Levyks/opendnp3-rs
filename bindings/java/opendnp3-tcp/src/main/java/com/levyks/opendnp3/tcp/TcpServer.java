package com.levyks.opendnp3.tcp;

import java.io.IOException;
import java.net.InetAddress;
import java.net.ServerSocket;
import java.util.concurrent.Executor;
import javax.net.ssl.SSLContext;
import javax.net.ssl.SSLServerSocket;

/** A listening socket that accepts one DNP3 connection at a time. */
public final class TcpServer implements AutoCloseable {
    private final ServerSocket listener;
    private final Executor ioExecutor;

    private TcpServer(ServerSocket listener, Executor ioExecutor) {
        this.listener = listener;
        this.ioExecutor = ioExecutor;
    }

    public static TcpServer listen(String host, int port) throws IOException {
        return listen(host, port, null);
    }

    /**
     * Listen using a caller-supplied {@link Executor} for all accepted connections
     * (for example, {@code Executors.newVirtualThreadPerTaskExecutor()} on Java 21+ or
     * {@link TcpExecutors#virtualThreadsOrCached()}). The caller owns the executor's lifecycle.
     */
    public static TcpServer listen(String host, int port, Executor ioExecutor) throws IOException {
        return new TcpServer(
                new ServerSocket(port, 50, InetAddress.getByName(host)),
                ioExecutor);
    }

    /** Listen for TLS (or mTLS when {@code needClientAuth} is true) connections. */
    public static TcpServer listenTls(
            String host,
            int port,
            SSLContext sslContext,
            boolean needClientAuth) throws IOException {
        return listenTls(host, port, sslContext, needClientAuth, null);
    }

    /** Listen for TLS (or mTLS) connections using a caller-supplied I/O {@link Executor}. */
    public static TcpServer listenTls(
            String host,
            int port,
            SSLContext sslContext,
            boolean needClientAuth,
            Executor ioExecutor) throws IOException {
        SSLServerSocket server = (SSLServerSocket) sslContext
                .getServerSocketFactory()
                .createServerSocket(port, 50, InetAddress.getByName(host));
        server.setNeedClientAuth(needClientAuth);
        return new TcpServer(server, ioExecutor);
    }

    public int port() {
        return listener.getLocalPort();
    }

    public TcpConnection accept() throws IOException {
        return TcpConnection.accepted(listener.accept(), ioExecutor);
    }

    @Override
    public void close() throws IOException {
        listener.close();
    }
}
