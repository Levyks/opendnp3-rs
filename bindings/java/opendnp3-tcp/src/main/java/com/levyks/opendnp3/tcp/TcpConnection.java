package com.levyks.opendnp3.tcp;

import com.levyks.opendnp3.Dnp3Stream;

import java.io.IOException;
import java.net.InetAddress;
import java.net.Socket;
import java.security.NoSuchAlgorithmException;
import java.util.concurrent.Executor;
import javax.net.ssl.SSLContext;
import javax.net.ssl.SSLSocket;

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

    /** Connect over TLS using the JVM default {@link SSLContext}. */
    public static TcpConnection connectTls(String host, int port) throws IOException {
        try {
            return connectTls(host, port, SSLContext.getDefault(), null);
        } catch (NoSuchAlgorithmException e) {
            throw new IOException("Default SSLContext unavailable", e);
        }
    }

    /** Connect over TLS (or mTLS) using a caller-supplied {@link SSLContext}. */
    public static TcpConnection connectTls(String host, int port, SSLContext sslContext) throws IOException {
        return connectTls(host, port, sslContext, null);
    }

    /** Connect over TLS (or mTLS) using a caller-supplied {@link SSLContext} and I/O {@link Executor}. */
    public static TcpConnection connectTls(
            String host,
            int port,
            SSLContext sslContext,
            Executor ioExecutor) throws IOException {
        SSLSocket socket = (SSLSocket) sslContext.getSocketFactory().createSocket(host, port);
        try {
            socket.setTcpNoDelay(true);
            socket.startHandshake();
            return new TcpConnection(socket, ioExecutor);
        } catch (IOException | RuntimeException e) {
            try {
                socket.close();
            } catch (IOException suppressed) {
                e.addSuppressed(suppressed);
            }
            throw e;
        }
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
