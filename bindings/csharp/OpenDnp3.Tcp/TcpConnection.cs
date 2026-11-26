using System.Diagnostics;
using System.Net;
using System.Net.Sockets;
using Levyks.OpenDnp3;

namespace Levyks.OpenDnp3.Tcp;

/// <summary>
/// One accepted or connected TCP socket plus the <see cref="Dnp3Stream"/> that drives it.
/// </summary>
public sealed class TcpConnection : IAsyncDisposable
{
    private readonly TcpClient _client;

    private TcpConnection(TcpClient client, Dnp3Stream stream)
    {
        _client = client;
        Stream = stream;
    }

    public Dnp3Stream Stream { get; }

    public static async Task<TcpConnection> ConnectAsync(
        string host,
        int port,
        CancellationToken cancellationToken = default)
    {
        var client = new TcpClient();
        try
        {
            await client.ConnectAsync(host, port, cancellationToken);
            return new TcpConnection(client, new TcpDnp3Stream(client.GetStream()));
        }
        catch
        {
            client.Dispose();
            throw;
        }
    }

    public static TcpConnection FromAccepted(TcpClient client) =>
        new(client, new TcpDnp3Stream(client.GetStream()));

    public ValueTask DisposeAsync()
    {
        _client.Dispose();
        return ValueTask.CompletedTask;
    }
}

/// <summary>Loopback and remote TCP accept helpers for an outstation.</summary>
public static class TcpServer
{
    public static TcpListener Listen(IPAddress address, int port = 0)
    {
        var listener = new TcpListener(address, port);
        listener.Start();
        return listener;
    }

    public static async Task<TcpConnection> AcceptAsync(
        TcpListener listener,
        CancellationToken cancellationToken = default)
    {
        var client = await listener.AcceptTcpClientAsync(cancellationToken);
        return TcpConnection.FromAccepted(client);
    }
}

sealed class TcpDnp3Stream(NetworkStream stream) : Dnp3Stream
{
    private readonly Stopwatch _clock = Stopwatch.StartNew();

    public async Task<byte[]> Read(uint maxBytes)
    {
        var buf = new byte[Math.Min(maxBytes, 65536)];
        try
        {
            int n = await stream.ReadAsync(buf);
            if (n == 0)
                throw new FfiStreamErrorException(new FfiStreamError.Closed());
            return buf[..n];
        }
        catch (IOException ex)
        {
            throw new FfiStreamErrorException(new FfiStreamError.Io(ex.Message));
        }
    }

    public async Task Write(byte[] data)
    {
        try
        {
            await stream.WriteAsync(data);
            await stream.FlushAsync();
        }
        catch (IOException ex)
        {
            throw new FfiStreamErrorException(new FfiStreamError.Io(ex.Message));
        }
    }

    public Task SleepMs(ulong durationMs) =>
        Task.Delay(TimeSpan.FromMilliseconds(Math.Min(durationMs, int.MaxValue)));

    public ulong NowMs() => (ulong)_clock.ElapsedMilliseconds;
}
