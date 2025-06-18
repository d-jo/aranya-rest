# Aranya REST Server

A REST API server for the Aranya daemon that provides HTTP endpoints for all daemon operations.

## Overview

This server acts as a REST API gateway to the Aranya daemon, allowing clients to interact with Aranya using standard HTTP requests instead of the native tarpc protocol. It provides JSON-based endpoints covering all operations available in the daemon API.

## Building

```bash
cargo build --bin aranya-rest --release
```

## Usage

```bash
aranya-rest --daemon-socket /path/to/daemon.sock --api-key <hex-encoded-key> --bind-addr 127.0.0.1:8080
```

### Command Line Arguments

- `--daemon-socket`: Path to the daemon's Unix domain socket (default: `/tmp/aranya-daemon.sock`)
- `--api-key`: Daemon API public key as hex-encoded string (required)
- `--bind-addr`: Address to bind the REST server (default: `127.0.0.1:8080`)

## API Endpoints

### Version and Device Information

- `GET /api/v1/version` - Get daemon version
- `GET /api/v1/local-addr` - Get daemon's local address
- `GET /api/v1/key-bundle` - Get device's public key bundle
- `GET /api/v1/device-id` - Get device ID

### Sync Peer Management

- `POST /api/v1/sync/peers` - Add sync peer
- `POST /api/v1/sync/now` - Sync with peer immediately
- `DELETE /api/v1/sync/peers` - Remove sync peer

### Team Management

- `POST /api/v1/teams` - Create new team
- `DELETE /api/v1/teams/{team_id}` - Close team
- `GET /api/v1/teams/{team_id}/devices` - Query devices on team

### Device Management

- `POST /api/v1/teams/{team_id}/devices` - Add device to team
- `DELETE /api/v1/teams/{team_id}/devices/{device_id}` - Remove device from team
- `GET /api/v1/teams/{team_id}/devices/{device_id}/role` - Query device role
- `GET /api/v1/teams/{team_id}/devices/{device_id}/keybundle` - Query device key bundle

### Role Management

- `POST /api/v1/teams/{team_id}/roles/assign` - Assign role to device
- `POST /api/v1/teams/{team_id}/roles/revoke` - Revoke role from device

### Network Identifier Management

- `POST /api/v1/teams/{team_id}/net-identifiers/assign` - Assign network identifier
- `POST /api/v1/teams/{team_id}/net-identifiers/remove` - Remove network identifier

### Label Management

- `GET /api/v1/teams/{team_id}/labels` - Query labels on team
- `POST /api/v1/teams/{team_id}/labels` - Create label
- `DELETE /api/v1/teams/{team_id}/labels/{label_id}` - Delete label
- `POST /api/v1/teams/{team_id}/labels/assign` - Assign label to device
- `POST /api/v1/teams/{team_id}/labels/revoke` - Revoke label from device

## Request/Response Format

All requests and responses use JSON format. IDs (team, device, label) are represented as hex-encoded strings. Binary data (keys, etc.) are base64-encoded.

### Example: Create Team

Request:
```bash
curl -X POST http://localhost:8080/api/v1/teams \
  -H "Content-Type: application/json" \
  -d '{"config": {}}'
```

Response:
```json
{
  "team_id": "a1b2c3d4e5f6789012345678901234567890abcdef1234567890abcdef123456"
}
```

### Example: Add Device to Team

Request:
```bash
curl -X POST http://localhost:8080/api/v1/teams/{team_id}/devices \
  -H "Content-Type: application/json" \
  -d '{
    "keys": {
      "identity": "base64-encoded-identity-key",
      "signing": "base64-encoded-signing-key", 
      "encoding": "base64-encoded-encoding-key"
    }
  }'
```

## Error Handling

The server returns appropriate HTTP status codes:

- `200 OK` - Success
- `400 Bad Request` - Invalid request or daemon API error
- `500 Internal Server Error` - Server error
- `503 Service Unavailable` - Daemon connection error

Error responses include details:
```json
{
  "error": "error category",
  "details": "detailed error message"
}
```

## Security Notes

- The API key is the public API key for the daemon, not a private key
- All communication with the daemon is encrypted using the txp protocol
- Consider running behind a reverse proxy with TLS in production
- Access control should be implemented at the network or proxy level