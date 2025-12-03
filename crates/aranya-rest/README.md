# Aranya REST Server v4.0.0

A REST API server for the Aranya daemon that provides HTTP endpoints for all daemon operations.

## Overview

This server acts as a REST API gateway to the Aranya daemon, allowing clients to interact with Aranya using standard HTTP requests instead of the native tarpc protocol. It provides JSON-based endpoints covering all operations available in the daemon API.

**v4.0.0 Features:**
- Custom RBAC roles with hierarchical permissions
- AFC (Aranya Fast Channels) - replaces the deprecated AQC system
- Enhanced label management with managing role requirements
- PSK encryption for secure sync operations
- Initial role assignment when adding devices

## Building

```bash
cargo build --bin aranya-rest --release
```

## Usage

```bash
aranya-rest --daemon-socket /path/to/daemon.sock --bind-addr 127.0.0.1:8080
```

The server automatically loads the daemon's API public key from `api.pk` file located next to the daemon socket (same location used by aranya-client).

### Command Line Arguments

- `--daemon-socket`: Path to the daemon's Unix domain socket (default: `/tmp/aranya-daemon.sock`)
- `--bind-addr`: Address to bind the REST server (default: `127.0.0.1:8080`)

### API Key Location

The server expects to find the daemon's public API key in a file named `api.pk` in the same directory as the daemon socket. This matches the behavior of the standard aranya-client.

## API Endpoints

### Version and Device Information

| Method | Endpoint | Description |
|--------|----------|-------------|
| GET | `/api/v1/version` | Get daemon version |
| GET | `/api/v1/local-addr` | Get daemon's local address |
| GET | `/api/v1/key-bundle` | Get device's public key bundle |
| GET | `/api/v1/device-id` | Get device ID |

### Sync Peer Management

| Method | Endpoint | Description |
|--------|----------|-------------|
| POST | `/api/v1/sync/peers` | Add sync peer |
| POST | `/api/v1/sync/now` | Sync with peer immediately |
| DELETE | `/api/v1/sync/peers` | Remove sync peer |
| GET | `/api/v1/teams/{team_id}/sync/peers` | Query sync peers for team |
| GET | `/api/v1/teams/{team_id}/sync/peers/{addr}/config` | Query sync peer config |
| PUT | `/api/v1/sync/peers/config` | Update sync peer config |
| POST | `/api/v1/teams/{team_id}/sync/encrypt-psk-seed` | Encrypt PSK seed for peer (v4.0.0) |

### Team Management

| Method | Endpoint | Description |
|--------|----------|-------------|
| POST | `/api/v1/teams` | Create new team |
| DELETE | `/api/v1/teams/{team_id}` | Close team |
| POST | `/api/v1/teams/{team_id}/add` | Add existing team to local storage |
| DELETE | `/api/v1/teams/{team_id}/remove` | Remove team from local storage |
| GET | `/api/v1/teams/{team_id}/devices` | Query devices on team |

### Device Management

| Method | Endpoint | Description |
|--------|----------|-------------|
| POST | `/api/v1/teams/{team_id}/devices` | Add device to team (supports initial_role) |
| DELETE | `/api/v1/teams/{team_id}/devices/{device_id}` | Remove device from team |
| GET | `/api/v1/teams/{team_id}/devices/{device_id}/keybundle` | Query device key bundle |
| GET | `/api/v1/teams/{team_id}/devices/{device_id}/role` | Query device role |
| GET | `/api/v1/teams/{team_id}/devices/{device_id}/labels` | Query device labels (v4.0.0) |

### Role Management (v4.0.0 Custom RBAC)

| Method | Endpoint | Description |
|--------|----------|-------------|
| POST | `/api/v1/teams/{team_id}/roles/setup-defaults` | Setup default roles for team |
| GET | `/api/v1/teams/{team_id}/roles` | Query all roles on team |
| GET | `/api/v1/teams/{team_id}/roles/{role_id}/owners` | Query roles that own a role |
| GET | `/api/v1/teams/{team_id}/roles/{role_id}/devices` | Query devices by role |
| GET | `/api/v1/teams/{team_id}/device-roles` | Query all device roles |
| POST | `/api/v1/teams/{team_id}/roles/assign` | Assign role to device |
| POST | `/api/v1/teams/{team_id}/roles/revoke` | Revoke role from device |
| POST | `/api/v1/teams/{team_id}/roles/change` | Change device role |
| POST | `/api/v1/teams/{team_id}/roles/bulk-assign` | Bulk assign roles |

### Label Management (v4.0.0)

| Method | Endpoint | Description |
|--------|----------|-------------|
| GET | `/api/v1/teams/{team_id}/labels` | Query labels on team |
| POST | `/api/v1/teams/{team_id}/labels` | Create label (requires managing_role_id) |
| GET | `/api/v1/teams/{team_id}/labels/{label_id}` | Query specific label |
| DELETE | `/api/v1/teams/{team_id}/labels/{label_id}` | Delete label |
| POST | `/api/v1/teams/{team_id}/labels/managing-role` | Add managing role to label |
| POST | `/api/v1/teams/{team_id}/labels/assign` | Assign label to device |
| POST | `/api/v1/teams/{team_id}/labels/revoke` | Revoke label from device |

### AFC (Aranya Fast Channels) - v4.0.0

| Method | Endpoint | Description |
|--------|----------|-------------|
| POST | `/api/v1/teams/{team_id}/afc/channels` | Create send channel |
| POST | `/api/v1/teams/{team_id}/afc/channels/accept` | Accept receive channel |
| DELETE | `/api/v1/afc/channels` | Delete channel |

## Request/Response Format

All requests and responses use JSON format. IDs (team, device, label, role) are represented as hex-encoded 32-byte strings. Binary data (keys, etc.) are base64-encoded.

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

### Example: Add Device to Team with Initial Role (v4.0.0)

Request:
```bash
curl -X POST http://localhost:8080/api/v1/teams/{team_id}/devices \
  -H "Content-Type: application/json" \
  -d '{
    "keys": {
      "identity": "base64-encoded-identity-key",
      "signing": "base64-encoded-signing-key",
      "encoding": "base64-encoded-encoding-key"
    },
    "initial_role": "hex-encoded-role-id"
  }'
```

### Example: Create Label with Managing Role (v4.0.0)

Request:
```bash
curl -X POST http://localhost:8080/api/v1/teams/{team_id}/labels \
  -H "Content-Type: application/json" \
  -d '{
    "name": "my-label",
    "managing_role_id": "hex-encoded-role-id"
  }'
```

Response:
```json
{
  "label_id": "a1b2c3d4e5f6789012345678901234567890abcdef1234567890abcdef123456"
}
```

### Example: Create AFC Channel (v4.0.0)

Request:
```bash
curl -X POST http://localhost:8080/api/v1/teams/{team_id}/afc/channels \
  -H "Content-Type: application/json" \
  -d '{
    "peer_id": "hex-encoded-device-id",
    "label_id": "hex-encoded-label-id"
  }'
```

Response:
```json
{
  "channel_id": "hex-encoded-channel-id",
  "ctrl_msg": "base64-encoded-control-message"
}
```

### Example: Query Team Roles (v4.0.0)

Request:
```bash
curl http://localhost:8080/api/v1/teams/{team_id}/roles
```

Response:
```json
{
  "roles": [
    {
      "id": "hex-encoded-role-id",
      "name": "Admin",
      "author_id": "hex-encoded-device-id",
      "is_default": true
    }
  ]
}
```

## Error Handling

The server returns appropriate HTTP status codes:

| Status | Description |
|--------|-------------|
| 200 OK | Success |
| 400 Bad Request | Invalid request or daemon API error |
| 403 Forbidden | Permission denied |
| 404 Not Found | Resource not found |
| 409 Conflict | Resource already exists |
| 500 Internal Server Error | Server error |
| 503 Service Unavailable | Daemon connection error |

Error responses include details:
```json
{
  "error": "error_type",
  "message": "detailed error message",
  "status": 400
}
```

## Security Notes

- The API key is automatically loaded from the `api.pk` file next to the daemon socket
- The API key is the public API key for the daemon, not a private key
- All communication with the daemon is encrypted using the txp protocol
- Consider running behind a reverse proxy with TLS in production
- Access control should be implemented at the network or proxy level
- Ensure the `api.pk` file has appropriate file permissions for your security requirements

## Testing with Multiple Daemons

### Quick Integration Test

For a complete end-to-end test that validates the REST API:

```bash
# Run full integration test (starts daemons, tests functionality, cleans up)
./scripts/full-test.bash
```

This script will:
- Start 2 daemon instances with REST servers
- Create a team on one daemon
- Add the second daemon's device to the team
- Verify device roles and team membership
- Test all major REST endpoints
- Automatically clean up when finished

### Manual Testing

For manual testing and development:

```bash
# Quick setup of 2 daemons with REST servers
./scripts/setup-multi-daemon.bash

# Setup 3 daemons starting from port 9000
./scripts/setup-multi-daemon.bash 3 9000
```

See `scripts/README.md` for detailed testing instructions and examples.

## Migration from v0.6.x

### Breaking Changes

1. **Role System**: The fixed `Role` enum (Owner, Admin, Operator, Member) is replaced with custom `RoleId`-based roles. Use `setup_default_roles` to initialize standard roles.

2. **Label Creation**: Now requires a `managing_role_id` parameter specifying which role manages the label.

3. **AQC Removed**: The AQC (Aranya QUIC Channels) system is removed and replaced with AFC (Aranya Fast Channels). Bidirectional channels are no longer supported - use two unidirectional channels instead.

4. **Device Addition**: The `add_device_to_team` endpoint now accepts an optional `initial_role` parameter.

5. **Request/Response Changes**:
   - Role queries return `RoleInfo` objects with `id`, `name`, `author_id`, `is_default`
   - Label operations require/return role IDs instead of role names
