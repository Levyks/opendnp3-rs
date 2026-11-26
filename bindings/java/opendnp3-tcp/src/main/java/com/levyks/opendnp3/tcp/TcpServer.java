package com.levyks.opendnp3.tcp;

import java.io.IOException;
import java.net.InetAddress;
import java.net.ServerSocket;

/** A listening socket that accepts one DNP3 connection at a time. */
public final class TcpServer implements AutoCloseable {
    private final ServerSocket listener;

    private TcpServer(ServerSocket listener) {
        this.listener = listener;
    }

    public static TcpServer listen(String host, int port) throws IOException {
        return new TcpServer(new ServerSocket(port, 50, InetAddress.getByName(host)));
    }

    public int port() {
        return listener.getLocalPort();
    }

    public TcpConnection accept() throws IOException {
        return TcpConnection.accepted(listener.accept());
    }

    @Override
    public void close() throws IOException {
        listener.close();
    }
}
