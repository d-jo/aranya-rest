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

### Issue 1: QUIC Stream FIN Delivery Problem

**Core Issue:** The server successfully closes after receiving ACK, but the client never receives the data/FIN. This suggests the ACK the server receives is NOT for the final STREAM frame with the FIN flag.

**Possible scenarios:**
1. **FIN lost after data ACKed**: The data was received and ACKed, but a subsequent FIN-bearing frame was lost
2. **Coalesced FIN not processed**: The FIN was coalesced with data but the receiver didn't process the FIN flag
3. **Stream state race**: A race condition in the bidirectional stream split where the send-side close interferes with receive-side processing

**The ~50 packets/second pattern explained:**
- 50/sec ≈ 20ms interval, close to typical local network RTT
- Bidirectional pattern (B sends, A responds twice) suggests:
  - B is sending probe/ACK frames
  - A is retransmitting or sending flow control updates
- This is consistent with a **Probe Timeout (PTO) retransmission loop**

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

**Note:** The s2n-quic client API does not support configuring the congestion controller. This asymmetry is unavoidable with the current API.

**Potential Impact:**
- Server uses BBR which is designed for high-bandwidth, high-latency networks
- Client uses the default (likely CUBIC or NewReno)
- In containerized environments with variable latency, this asymmetry might cause:
  - BBR's aggressive probing on the server side
  - Potential pacing mismatches

**Workaround:** Remove BBR from the server to use default on both sides for testing.

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

### Fix 1: Remove BBR from Server (Test Symmetry)

Since the client cannot configure a congestion controller, test with the default on both:

```rust
// In Server::new(), remove the congestion controller:
let server = QuicServer::builder()
    .with_tls(tls_server_provider)?
    .with_io(addr)?
    // Remove: .with_congestion_controller(Bbr::default())?
    .start()
    .map_err(Error::ServerStart)?;
```

### Fix 2: Add Stream-Level Timeout with Retry

Add explicit timeout and retry logic to detect and recover from the hang:

```rust
async fn receive_sync_response<S, A>(/*...*/) -> SyncResult<usize> {
    debug!("client receiving sync response from QUIC sync server");

    let mut recv_buf = Vec::new();

    // Add timeout to detect hang
    match tokio::time::timeout(Duration::from_secs(30), recv.read_to_end(&mut recv_buf)).await {
        Ok(Ok(_)) => {
            debug!(n = recv_buf.len(), "received sync response");
        }
        Ok(Err(e)) => {
            error!("Stream error during receive: {:?}", e);
            return Err(e.into());
        }
        Err(_) => {
            error!("TIMEOUT: read_to_end hung - this is the flaky issue!");
            // Consider forcing connection close and retry here
            return Err(anyhow::anyhow!("Sync receive timeout").into());
        }
    }
    // ...
}
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

**Most Likely Root Cause:** The STREAM frame with FIN flag is being lost or not processed by the client, causing:
1. Server thinks it successfully closed (it ACKed, but for previous data, not the FIN)
2. Client waits indefinitely for FIN
3. Probe Timeout (PTO) retransmission loop creates the ~50 packets/second pattern

**Contributing Factors:**
1. **Docker networking UDP issues** - Packet loss, buffer issues, or conntrack problems causing selective frame loss
2. **Connection reuse race conditions** - The SharedConnectionMap may have stale connection state issues
3. **Network path changes** - Cross-machine networks have more variability, increasing the failure rate

**The ~50 packets/second Pattern Breakdown:**
- 50/sec ≈ 20ms RTT (typical local/container network)
- Server retransmits FIN-bearing STREAM frame on PTO
- Client ACKs frames it has seen (not the FIN)
- Two responses from A = retransmitted data + ACK for B's packet

## Next Steps (Priority Order)

1. **Capture packets** - Use tcpdump/Wireshark to confirm whether FIN-bearing STREAM frames are being sent and whether they're received at the UDP level
2. **Enable s2n-quic tracing** - Set `RUST_LOG=s2n_quic=trace` to see stream state machine transitions
3. **Test with host networking** - `docker run --network host` to isolate Docker networking as the cause
4. **Remove BBR from server** - Test with default congestion controller on both sides
5. **Add stream timeout** - Add explicit timeout to detect and recover from hangs
6. **Check UDP buffer sizes** - Increase `rmem_max` and `wmem_max` in containers

## Key Questions to Answer

1. Is the FIN-bearing STREAM frame being sent by the server at the UDP level?
2. Is that UDP packet arriving at the client container?
3. Is s2n-quic's stream state machine receiving and processing it?
4. What frame types are in the ~50/second packets?
