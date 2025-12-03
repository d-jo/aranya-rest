# S2N-QUIC Syncer Flaky Behavior Diagnosis

## Problem Summary

Communication between Node A (server) and Node B (client) in Docker containers is flaky:
- Initial traffic works
- Server on A sends response, client B's `read_to_end` never processes it
- Pattern observed: B sends ~90 byte packets (~50 bytes payload), gets two small responses from A, ~50 times/second
- These small packets are QUIC internal, not application data
- Server successfully closes stream after receiving ACK
- Problem is intermittent and worsens on cross-machine networks
- Not NAT issues (consistent source/dest IP/port)
- Not simple MTU issues (large packets successfully traverse)

## Code Analysis (Aranya v3.0.0)

### Key Files Analyzed
- `/tmp/aranya/crates/aranya-daemon/src/sync/task/quic.rs` - Main QUIC syncer
- `/tmp/aranya/crates/aranya-daemon/src/sync/task/quic/connections.rs` - Connection management
- `/tmp/aranya/crates/aranya-util/src/s2n_quic.rs` - Utility functions

### Flow Analysis

**Client (B) - Send Request:**
```rust
// quic.rs:304-308
send.send(Bytes::from(send_buf)).await?;
send.close().await?;  // Sends STREAM FIN
```

**Server (A) - Handle Request:**
```rust
// quic.rs:559-562
let (mut recv, mut send) = stream.split();
recv.read_to_end(&mut recv_buf).await?;  // Waits for client FIN
```

**Server (A) - Send Response:**
```rust
// quic.rs:580-585
send.send(Bytes::from(data)).await?;
send.close().await?;  // Sends STREAM FIN
```

**Client (B) - Receive Response (HANGS HERE):**
```rust
// quic.rs:331-334
recv.read_to_end(&mut recv_buf).await?;  // Never returns!
```

## Identified Issues

### Issue 1: `AsyncReadExt::read_to_end` vs Native s2n-quic Methods

**Location:** `quic.rs:44` and `quic.rs:332`

The syncer uses `tokio::io::AsyncReadExt::read_to_end`:
```rust
use tokio::{io::AsyncReadExt, sync::mpsc};
// ...
recv.read_to_end(&mut recv_buf).await?;
```

However, there's a custom implementation in `aranya_util::s2n_quic`:
```rust
// s2n_quic.rs:10-27
pub async fn read_to_end(stream: &mut stream::ReceiveStream) -> Result<Bytes, stream::Error> {
    let Some(first) = stream.receive().await? else {
        return Ok(Bytes::new());
    };
    // Uses native s2n-quic stream.receive() method
    // ...
}
```

**Problem:** The `AsyncRead` trait implementation in s2n-quic translates between tokio I/O semantics and QUIC stream semantics. This translation layer may have subtle issues with:
- FIN detection and propagation
- Buffer management
- Error handling differences

**Recommendation:** Switch to using `aranya_util::s2n_quic::read_to_end` which uses native s2n-quic methods.

### Issue 2: Connection Keep-Alive Behavior

**Location:** `quic.rs:246` and `quic.rs:497`

Both client and server call:
```rust
conn.keep_alive(true)?;
```

**Problem:** When `keep_alive(true)` is set, s2n-quic sends PING frames periodically (default is every ~15 seconds, but can vary). The ~50 packets/second pattern you're seeing is NOT the keep-alive itself, but could be:

1. **QUIC ACK frames** - Acknowledging received data
2. **MAX_DATA / MAX_STREAM_DATA frames** - Flow control updates
3. **Retransmissions** - If the server thinks data wasn't ACKed

The fact that you see ~50 packets/second suggests either:
- Aggressive retransmission due to perceived packet loss
- Flow control deadlock where receiver isn't advertising credit

### Issue 3: Asymmetric Congestion Controller Configuration

**Location:** `quic.rs:170-175` (client) and `quic.rs:436-441` (server)

**Server:**
```rust
QuicServer::builder()
    .with_tls(tls_server_provider)?
    .with_io(addr)
    .with_congestion_controller(Bbr::default())?  // Uses BBR
    .start()
```

**Client:**
```rust
QuicClient::builder()
    .with_tls(provider)?
    .with_io((Ipv4Addr::UNSPECIFIED, 0))  // No congestion controller specified - uses default
    .start()
```

**Problem:** The server uses BBR (Bottleneck Bandwidth and Round-trip propagation time) while the client uses the default (likely CUBIC or NewReno). This asymmetry can cause issues:
- BBR is designed for high-bandwidth, high-latency networks
- In containerized environments with low latency, BBR can be overly aggressive
- Mismatched congestion controllers can lead to unfair bandwidth sharing and potential stalls

**Recommendation:** Use the same congestion controller on both sides, or test with the default on both.

### Issue 4: Connection Pool Race Condition

**Location:** `connections.rs:90-121`

```rust
pub(super) async fn get_or_try_insert_with(/*...*/) -> Result<Handle, super::Error> {
    let (handle, maybe_acceptor) = match self.handles.lock().await.entry(key) {
        Entry::Occupied(mut entry) => {
            if entry.get_mut().ping().is_ok() {
                (entry.get().clone(), None)  // Reuse connection
            } else {
                let (handle, acceptor) = make_conn().await?.split();
                // ...
            }
        }
        // ...
    };
```

**Problem:** The `ping()` check is a synchronous operation that just checks if the connection is still valid at that moment. However:
1. Between the ping check and the actual stream operation, the connection could become invalid
2. In Docker networks with high variability, connections can become stale quickly
3. The connection might be "alive" but have issues with flow control or stream state

### Issue 5: Docker/Container Networking UDP Issues

Docker networking, especially in overlay networks or between hosts, can have issues with UDP traffic:

1. **MTU issues with encapsulation:** Even if you're seeing large packets succeed, VXLAN or other encapsulation adds overhead. Packets near the MTU limit might be fragmented or dropped inconsistently.

2. **Connection tracking state:** Docker's netfilter/iptables rules may have issues with long-lived UDP flows. QUIC's connection migration and keep-alive patterns might confuse conntrack.

3. **UDP buffer sizes:** Docker containers may have different socket buffer sizes than the host. If the receive buffer fills up, packets are silently dropped.

## The ~50 Packets/Second Pattern

Given the symptoms, this pattern is most likely **QUIC flow control frames or ACK-based retransmission**:

1. Server sends response data
2. Server gets ACK at the QUIC transport layer (that's why it thinks the close succeeded)
3. But the STREAM frame with the actual data OR the FIN flag is lost/not processed
4. Client is waiting for more data (specifically the FIN)
5. Server periodically sends MAX_STREAM_DATA or the client sends ACKs for what it has
6. This creates the 50/second pattern

The fact that it's **bidirectional** (B sends, gets 2 responses from A) suggests:
- B is sending ACKs for data it received (maybe partial)
- A is sending MAX_DATA/MAX_STREAM_DATA frames or retrying the FIN

## Recommended Diagnostic Steps

### 1. Enable s2n-quic Debug Logging
```rust
// Add to Cargo.toml
s2n-quic = { ..., features = ["provider-tls-rustls", "provider-address-token-default", "tracing"] }
```

Set `RUST_LOG=s2n_quic=debug` or `s2n_quic=trace` to see:
- Stream state transitions
- Flow control windows
- Frame-level details

### 2. Capture and Analyze QUIC Packets

Use `tcpdump` or `wireshark` with QUIC dissector:
```bash
tcpdump -i any -w quic_capture.pcap udp port <sync_port>
```

Look for:
- STREAM frames with FIN flag
- MAX_STREAM_DATA frames
- ACK frames and what they're acknowledging
- RESET_STREAM frames (if any)

### 3. Add Timeouts and Diagnostics

Modify the client receive code:
```rust
// Instead of just read_to_end, add timeout and diagnostics
let recv_result = tokio::time::timeout(
    Duration::from_secs(30),
    recv.read_to_end(&mut recv_buf)
).await;

match recv_result {
    Ok(Ok(_)) => debug!("Received {} bytes", recv_buf.len()),
    Ok(Err(e)) => error!("Stream error: {:?}", e),
    Err(_) => {
        error!("Timeout waiting for response! This is the flaky issue.");
        // Log connection state if possible
    }
}
```

### 4. Test with Native s2n-quic Methods

Replace the `AsyncReadExt::read_to_end` with the native implementation:
```rust
use aranya_util::s2n_quic::read_to_end;

// Instead of:
// recv.read_to_end(&mut recv_buf).await?;

// Use:
let recv_bytes = read_to_end(&mut recv).await?;
```

### 5. Check Docker Network Configuration

```bash
# Check MTU settings
docker network inspect <network_name> | jq '.[].Options'

# Check UDP buffer sizes in container
cat /proc/sys/net/core/rmem_max
cat /proc/sys/net/core/wmem_max

# Check for conntrack issues
conntrack -L | grep -i udp
```

## Potential Fixes

### Fix 1: Use Native s2n-quic read_to_end

```rust
// In quic.rs, change:
use tokio::{io::AsyncReadExt, sync::mpsc};

// To:
use aranya_util::s2n_quic::read_to_end as quic_read_to_end;
use tokio::sync::mpsc;

// And update receive_sync_response:
async fn receive_sync_response<S, A>(/*...*/) -> SyncResult<usize> {
    debug!("client receiving sync response from QUIC sync server");

    let recv_bytes = quic_read_to_end(recv).await
        .context("failed to read sync response")?;
    debug!(n = recv_bytes.len(), "received sync response");
    // ...
}
```

### Fix 2: Add Consistent Congestion Controller

```rust
// In State::new(), add congestion controller to client:
let client = QuicClient::builder()
    .with_tls(provider)?
    .with_io((Ipv4Addr::UNSPECIFIED, 0))?
    .with_congestion_controller(Bbr::default())?  // Match server
    .start()
    .map_err(Error::ClientStart)?;
```

### Fix 3: Add Connection Health Monitoring

Add a heartbeat or health check mechanism that:
1. Detects stalled streams
2. Forces connection reconnection on persistent issues
3. Logs detailed diagnostics when issues occur

### Fix 4: Adjust Docker Network Settings

```bash
# Increase UDP buffer sizes
sysctl -w net.core.rmem_max=2097152
sysctl -w net.core.wmem_max=2097152

# Consider using host networking for testing
docker run --network host ...
```

## Summary

The most likely root cause is a combination of:

1. **`AsyncReadExt::read_to_end` not properly detecting stream FIN** - The tokio AsyncRead adapter may not correctly translate s2n-quic's stream termination signaling.

2. **Docker networking UDP quirks** - Packet loss, buffer issues, or conntrack problems causing the STREAM FIN or final data frames to be lost.

3. **Asymmetric congestion controller** - BBR on server only may cause suboptimal behavior.

The ~50 packets/second pattern strongly suggests a flow control or retransmission loop where both sides are alive but disagreeing about stream state.

## Next Steps

1. Add comprehensive tracing/logging to the sync path
2. Test with native s2n-quic `read_to_end`
3. Capture packets to confirm the STREAM FIN delivery
4. Test with host networking to isolate Docker issues
5. Test with matching congestion controllers on both sides
