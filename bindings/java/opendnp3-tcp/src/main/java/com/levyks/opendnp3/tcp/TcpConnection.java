package com.levyks.opendnp3.tcp;

import com.levyks.opendnp3.Dnp3Stream;

import java.io.IOException;
import java.net.InetAddress;
import java.net.Socket;

/** One connected TCP socket and the stream the master or outstation runs on. */
public final class TcpConnection implements AutoCloseable {
    private final Socket socket;
    private final TcpDnp3Stream stream;

    private TcpConnection(Socket socket) throws IOException {
        this.socket = socket;
        this.stream = new TcpDnp3Stream(socket);
    }

    public static TcpConnection connect(String host, int port) throws IOException {
        return new TcpConnection(new Socket(InetAddress.getByName(host), port));
    }

    static TcpConnection accepted(Socket socket) throws IOException {
        return new TcpConnection(socket);
    }

    public Dnp3Stream stream() {
        return stream;
    }

    @Override
    public void close() {
        stream.close();
    }
}
