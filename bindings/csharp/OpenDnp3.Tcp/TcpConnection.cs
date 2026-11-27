using System;
using System.Diagnostics;
using System.IO;
using System.Net;
using System.Net.Security;
using System.Net.Sockets;
using System.Security.Authentication;
using System.Security.Cryptography.X509Certificates;
using System.Threading;
using System.Threading.Tasks;
using Levyks.OpenDnp3;

namespace Levyks.OpenDnp3.Tcp;

/// <summary>
/// One accepted or connected TCP (or TLS) socket plus the <see cref="Dnp3Stream"/> that drives it.
/// </summary>
public sealed class TcpConnection : IAsyncDisposable, IDisposable
{
    private readonly TcpClient _client;
    private readonly IDisposable? _extraDisposable;

    private TcpConnection(TcpClient client, Dnp3Stream stream, IDisposable? extraDisposable = null)
    {
        _client = client;
        Stream = stream;
        _extraDisposable = extraDisposable;
    }

    public Dnp3Stream Stream { get; }

    public static async Task<TcpConnection> ConnectAsync(
        string host,
        int port,
        CancellationToken cancellationToken = default)
    {
        var client = new TcpClient { NoDelay = true };
        try
        {
            using var reg = cancellationToken.Register(client.Dispose);
            await client.ConnectAsync(host, port).ConfigureAwait(false);
            cancellationToken.ThrowIfCancellationRequested();
            return new TcpConnection(client, new TcpDnp3Stream(client.GetStream()));
        }
        catch
        {
            client.Dispose();
            throw;
        }
    }

    /// <summary>
    /// Connect over TLS (or mutual TLS when <paramref name="clientCertificates"/> is supplied).
    /// </summary>
    public static async Task<TcpConnection> ConnectTlsAsync(
        string host,
        int port,
        string? targetHost = null,
        X509CertificateCollection? clientCertificates = null,
        RemoteCertificateValidationCallback? certificateValidationCallback = null,
        CancellationToken cancellationToken = default)
    {
        var client = new TcpClient { NoDelay = true };
        SslStream? ssl = null;
        try
        {
            using var reg = cancellationToken.Register(client.Dispose);
            await client.ConnectAsync(host, port).ConfigureAwait(false);
            cancellationToken.ThrowIfCancellationRequested();
            ssl = new SslStream(client.GetStream(), leaveInnerStreamOpen: false, certificateValidationCallback);
            await ssl.AuthenticateAsClientAsync(
                targetHost ?? host,
                clientCertificates,
                SslProtocols.None,
                checkCertificateRevocation: false).ConfigureAwait(false);
            cancellationToken.ThrowIfCancellationRequested();
            return new TcpConnection(client, new TcpDnp3Stream(ssl), ssl);
        }
        catch
        {
            ssl?.Dispose();
            client.Dispose();
            throw;
        }
    }

    public static TcpConnection FromAccepted(TcpClient client)
    {
        client.NoDelay = true;
        return new(client, new TcpDnp3Stream(client.GetStream()));
    }

    internal static TcpConnection FromAcceptedTls(TcpClient client, SslStream ssl)
    {
        client.NoDelay = true;
        return new(client, new TcpDnp3Stream(ssl), ssl);
    }

    public void Dispose()
    {
        _extraDisposable?.Dispose();
        _client.Dispose();
    }

    public ValueTask DisposeAsync()
    {
        Dispose();
        return default;
    }
}

/// <summary>Loopback and remote TCP/TLS accept helpers for an outstation.</summary>
public static class TcpServer
{
    public static System.Net.Sockets.TcpListener Listen(IPAddress address, int port = 0)
    {
        var listener = new System.Net.Sockets.TcpListener(address, port);
        listener.Start();
        return listener;
    }

    public static async Task<TcpConnection> AcceptAsync(
        System.Net.Sockets.TcpListener listener,
        CancellationToken cancellationToken = default)
    {
        using var reg = cancellationToken.Register(listener.Stop);
        var client = await listener.AcceptTcpClientAsync().ConfigureAwait(false);
        cancellationToken.ThrowIfCancellationRequested();
        return TcpConnection.FromAccepted(client);
    }

    /// <summary>
    /// Accept one TCP connection and perform a TLS (or mutual TLS) server handshake.
    /// </summary>
    public static async Task<TcpConnection> AcceptTlsAsync(
        System.Net.Sockets.TcpListener listener,
        X509Certificate serverCertificate,
        bool clientCertificateRequired = false,
        RemoteCertificateValidationCallback? certificateValidationCallback = null,
        CancellationToken cancellationToken = default)
    {
        using var reg = cancellationToken.Register(listener.Stop);
        var client = await listener.AcceptTcpClientAsync().ConfigureAwait(false);
        SslStream? ssl = null;
        try
        {
            cancellationToken.ThrowIfCancellationRequested();
            client.NoDelay = true;
            ssl = new SslStream(client.GetStream(), leaveInnerStreamOpen: false, certificateValidationCallback);
            await ssl.AuthenticateAsServerAsync(
                serverCertificate,
                clientCertificateRequired,
                SslProtocols.None,
                checkCertificateRevocation: false).ConfigureAwait(false);
            cancellationToken.ThrowIfCancellationRequested();
            return TcpConnection.FromAcceptedTls(client, ssl);
        }
        catch
        {
            ssl?.Dispose();
            client.Dispose();
            throw;
        }
    }
}

sealed class TcpDnp3Stream(Stream stream) : Dnp3Stream
{
    private readonly Stopwatch _clock = Stopwatch.StartNew();

    public async Task<byte[]> Read(uint maxBytes)
    {
        int size = (int)Math.Min(maxBytes, 65536u);
        var buf = new byte[size];
        try
        {
            int n = await stream.ReadAsync(buf, 0, buf.Length).ConfigureAwait(false);
            if (n == 0)
                throw new StreamErrorException(new StreamError.Closed());
            if (n == buf.Length)
                return buf;
            var result = new byte[n];
            Buffer.BlockCopy(buf, 0, result, 0, n);
            return result;
        }
        catch (IOException ex)
        {
            throw new StreamErrorException(new StreamError.Io(ex.Message));
        }
    }

    public async Task Write(byte[] data)
    {
        try
        {
            await stream.WriteAsync(data, 0, data.Length).ConfigureAwait(false);
            await stream.FlushAsync().ConfigureAwait(false);
        }
        catch (IOException ex)
        {
            throw new StreamErrorException(new StreamError.Io(ex.Message));
        }
    }

    public Task SleepMs(ulong durationMs) =>
        Task.Delay(TimeSpan.FromMilliseconds(Math.Min(durationMs, (ulong)int.MaxValue)));

    public ulong NowMs() => (ulong)_clock.ElapsedMilliseconds;
}
