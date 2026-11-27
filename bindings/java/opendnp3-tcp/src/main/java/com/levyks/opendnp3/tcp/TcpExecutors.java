package com.levyks.opendnp3.tcp;

import java.lang.reflect.Method;
import java.util.concurrent.ExecutorService;
import java.util.concurrent.Executors;

/**
 * Factory helpers for I/O executors passed to {@link TcpConnection} and {@link TcpServer}.
 *
 * <p>Compiles on Java 17 while letting Java 21+ callers opt into virtual threads without
 * hard-coding a Java 21 bytecode dependency.
 */
public final class TcpExecutors {
    private static final Method NEW_VIRTUAL_THREAD_EXECUTOR = findVirtualThreadFactory();

    private TcpExecutors() {}

    private static Method findVirtualThreadFactory() {
        try {
            return Executors.class.getMethod("newVirtualThreadPerTaskExecutor");
        } catch (NoSuchMethodException ignored) {
            return null;
        }
    }

    /** Returns {@code true} if the current JVM supports virtual threads (Java 21+). */
    public static boolean supportsVirtualThreads() {
        return NEW_VIRTUAL_THREAD_EXECUTOR != null;
    }

    /**
     * Returns a virtual-thread-per-task {@link ExecutorService} on Java 21+, or falls back
     * to a daemon cached thread pool on Java 17–20.
     */
    public static ExecutorService virtualThreadsOrCached() {
        if (NEW_VIRTUAL_THREAD_EXECUTOR != null) {
            try {
                return (ExecutorService) NEW_VIRTUAL_THREAD_EXECUTOR.invoke(null);
            } catch (ReflectiveOperationException ignored) {
                // Fall back to cached daemon thread pool.
            }
        }
        return newDaemonCachedPool();
    }

    /** Creates a daemon cached thread pool compatible with Java 17+. */
    public static ExecutorService newDaemonCachedPool() {
        return Executors.newCachedThreadPool(runnable -> {
            Thread thread = new Thread(runnable, "opendnp3-tcp");
            thread.setDaemon(true);
            return thread;
        });
    }
}
