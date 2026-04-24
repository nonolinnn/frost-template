# API Contract

Complete REST API specification for the FROST 2-of-2 TSS Solana Wallet system.

Two API boundaries exist:

- **Frontend to Coordinator** (port 8080): all user-facing operations
- **Coordinator to Node** (ports 8081/8082): orchestration of cryptographic operations

All request and response bodies use `Content-Type: application/json`.

---

## Conventions

### Node Identifiers

Nodes are identified by string IDs matching the `NODE_ID` environment variable: `node-a` or `node-b`.

The Coordinator maintains a registry mapping node IDs to their base URLs:

| Node ID  | Base URL                    |
| -------- | --------------------------- |
| `node-a` | `http://node-a:8081`        |
| `node-b` | `http://node-b:8082`        |

### UUIDs

All primary keys are UUIDs (v4). Represented as strings in JSON: `"550e8400-e29b-41d4-a716-446655440000"`.

### Timestamps

All timestamps are ISO 8601 strings in UTC: `"2026-04-21T12:00:00Z"`.

### Error Response Format

All error responses use a consistent envelope:

```json
{
  "error": {
    "code": "DKG_NOT_COMPLETE",
    "message": "DKG must be completed before deriving wallets"
  }
}
```

HTTP status codes follow standard semantics:

| Code | Usage                                      |
| ---- | ------------------------------------------ |
| 200  | Success (GET, idempotent POST)             |
| 201  | Resource created                           |
| 400  | Invalid request body or parameters         |
| 404  | Resource not found                         |
| 409  | State conflict (e.g., round out of order)  |
| 500  | Internal server error                      |
| 502  | Upstream node communication failure        |

---

## Health Check

Both the Coordinator and each Node expose a health endpoint.

### `GET /health`

**Response** `200 OK`

Coordinator:
```json
{
  "status": "ok",
  "service": "coordinator"
}
```

Node:
```json
{
  "status": "ok",
  "service": "tss-node",
  "node_id": "node-a"
}
```

---

## Frontend to Coordinator API

### DKG (Distributed Key Generation)

#### `POST /api/dkg/start`

Initialize a new DKG session. Only one active session may exist at a time. Starting a new session when a completed session exists is an error (the system supports a single root key).

**Request Body**: None (empty body or `{}`)

**Response** `201 Created`

```json
{
  "session_id": "uuid",
  "status": "initialized",
  "created_at": "2026-04-21T12:00:00Z",
  "nodes": {
    "node-a": { "round1": "pending", "round2": "pending", "round3": "pending" },
    "node-b": { "round1": "pending", "round2": "pending", "round3": "pending" }
  }
}
```

**Errors**

| Code | Error Code              | Condition                            |
| ---- | ----------------------- | ------------------------------------ |
| 409  | `DKG_ALREADY_EXISTS`    | A completed DKG session exists       |
| 409  | `DKG_IN_PROGRESS`       | An active DKG session is in progress |

---

#### `POST /api/dkg/round/{round}/node/{node_id}`

Trigger a specific node to execute a specific DKG round. The Coordinator forwards the request to the target node with any required input data from previous rounds, then stores the result.

**Path Parameters**

| Parameter | Type    | Description                  |
| --------- | ------- | ---------------------------- |
| `round`   | integer | Round number: 1, 2, or 3     |
| `node_id` | string  | Node identifier: `node-a` or `node-b` |

**Request Body**: None

**Preconditions by Round**

- Round 1: DKG session must be initialized. No prior data needed.
- Round 2: The target node must have completed Round 1. The other node must have completed Round 1 (its Round 1 package is needed as input).
- Round 3: The target node must have completed Round 2. The other node must have completed Round 2 (its Round 2 package is needed as input).

**Response** `200 OK`

```json
{
  "session_id": "uuid",
  "node_id": "node-a",
  "round": 1,
  "status": "complete",
  "nodes": {
    "node-a": { "round1": "complete", "round2": "pending", "round3": "pending" },
    "node-b": { "round1": "pending", "round2": "pending", "round3": "pending" }
  }
}
```

When Round 3 completes for the last node, the overall DKG status becomes `complete` and the response includes the group verifying key:

```json
{
  "session_id": "uuid",
  "node_id": "node-b",
  "round": 3,
  "status": "complete",
  "dkg_complete": true,
  "group_public_key": "Base58EncodedGroupVerifyingKey",
  "nodes": {
    "node-a": { "round1": "complete", "round2": "complete", "round3": "complete" },
    "node-b": { "round1": "complete", "round2": "complete", "round3": "complete" }
  }
}
```

**Errors**

| Code | Error Code               | Condition                                    |
| ---- | ------------------------ | -------------------------------------------- |
| 400  | `INVALID_ROUND`          | Round is not 1, 2, or 3                      |
| 400  | `INVALID_NODE_ID`        | Node ID is not `node-a` or `node-b`          |
| 404  | `DKG_SESSION_NOT_FOUND`  | No active DKG session exists                 |
| 409  | `ROUND_ALREADY_COMPLETE` | This node already completed this round       |
| 409  | `ROUND_PRECONDITION`     | Preconditions not met (details in message)   |
| 502  | `NODE_UNAVAILABLE`       | Could not reach the target node              |
| 502  | `NODE_ERROR`             | Node returned an error during the round      |

---

#### `GET /api/dkg/status`

Get the current DKG session status, including per-node round progress.

**Response** `200 OK`

If a DKG session exists:

```json
{
  "session_id": "uuid",
  "status": "in_progress",
  "created_at": "2026-04-21T12:00:00Z",
  "completed_at": null,
  "group_public_key": null,
  "nodes": {
    "node-a": { "round1": "complete", "round2": "complete", "round3": "pending" },
    "node-b": { "round1": "complete", "round2": "pending", "round3": "pending" }
  }
}
```

When DKG is complete:

```json
{
  "session_id": "uuid",
  "status": "complete",
  "created_at": "2026-04-21T12:00:00Z",
  "completed_at": "2026-04-21T12:05:00Z",
  "group_public_key": "Base58EncodedGroupVerifyingKey",
  "nodes": {
    "node-a": { "round1": "complete", "round2": "complete", "round3": "complete" },
    "node-b": { "round1": "complete", "round2": "complete", "round3": "complete" }
  }
}
```

If no DKG session exists:

```json
{
  "session_id": null,
  "status": "not_started",
  "created_at": null,
  "completed_at": null,
  "group_public_key": null,
  "nodes": {}
}
```

---

### Wallets

#### `POST /api/wallets`

Derive the next wallet using sequential indexing. The Coordinator performs HD derivation from the group verifying key (root public key) without contacting the nodes.

**Request Body**: None (empty body or `{}`)

**Response** `201 Created`

```json
{
  "index": 0,
  "address": "Base58SolanaAddress",
  "public_key": "Base58DerivedChildPublicKey",
  "created_at": "2026-04-21T12:10:00Z"
}
```

**Errors**

| Code | Error Code         | Condition                        |
| ---- | ------------------ | -------------------------------- |
| 409  | `DKG_NOT_COMPLETE` | DKG has not been completed yet   |

---

#### `GET /api/wallets`

List all derived wallets.

**Response** `200 OK`

```json
{
  "wallets": [
    {
      "index": 0,
      "address": "Base58SolanaAddress0",
      "public_key": "Base58DerivedChildPublicKey0",
      "created_at": "2026-04-21T12:10:00Z"
    },
    {
      "index": 1,
      "address": "Base58SolanaAddress1",
      "public_key": "Base58DerivedChildPublicKey1",
      "created_at": "2026-04-21T12:11:00Z"
    }
  ]
}
```

Returns an empty array if no wallets have been derived yet.

---

#### `GET /api/wallets/{index}/balance`

Query the SOL balance of a specific derived wallet from Solana Devnet via RPC.

**Path Parameters**

| Parameter | Type    | Description             |
| --------- | ------- | ----------------------- |
| `index`   | integer | Wallet derivation index |

**Response** `200 OK`

```json
{
  "index": 0,
  "address": "Base58SolanaAddress0",
  "balance_lamports": 1000000000,
  "balance_sol": 1.0
}
```

**Errors**

| Code | Error Code          | Condition                              |
| ---- | ------------------- | -------------------------------------- |
| 404  | `WALLET_NOT_FOUND`  | No wallet exists at the given index    |
| 502  | `SOLANA_RPC_ERROR`  | Failed to query Solana Devnet          |

---

### Signing Requests

#### `POST /api/signing-requests`

Create a new signing request (SOL transfer transaction).

**Request Body**

```json
{
  "wallet_index": 0,
  "recipient": "Base58RecipientAddress",
  "amount_lamports": 500000000
}
```

| Field             | Type    | Required | Description                            |
| ----------------- | ------- | -------- | -------------------------------------- |
| `wallet_index`    | integer | yes      | Index of the sender wallet             |
| `recipient`       | string  | yes      | Base58 Solana address of the recipient |
| `amount_lamports` | integer | yes      | Amount to transfer in lamports         |

**Response** `201 Created`

```json
{
  "id": "uuid",
  "wallet_index": 0,
  "sender_address": "Base58SenderAddress",
  "recipient": "Base58RecipientAddress",
  "amount_lamports": 500000000,
  "status": "pending",
  "created_at": "2026-04-21T12:15:00Z",
  "nodes": {
    "node-a": { "round1": "pending", "round2": "pending" },
    "node-b": { "round1": "pending", "round2": "pending" }
  }
}
```

**Errors**

| Code | Error Code           | Condition                               |
| ---- | -------------------- | --------------------------------------- |
| 400  | `INVALID_RECIPIENT`  | Recipient is not a valid Base58 address |
| 400  | `INVALID_AMOUNT`     | Amount is zero or negative              |
| 404  | `WALLET_NOT_FOUND`   | No wallet at the given index            |
| 409  | `DKG_NOT_COMPLETE`   | DKG has not been completed              |

---

#### `GET /api/signing-requests`

List all signing requests, ordered by creation time (newest first).

**Response** `200 OK`

```json
{
  "signing_requests": [
    {
      "id": "uuid",
      "wallet_index": 0,
      "sender_address": "Base58SenderAddress",
      "recipient": "Base58RecipientAddress",
      "amount_lamports": 500000000,
      "status": "pending",
      "created_at": "2026-04-21T12:15:00Z",
      "tx_signature": null,
      "nodes": {
        "node-a": { "round1": "pending", "round2": "pending" },
        "node-b": { "round1": "pending", "round2": "pending" }
      }
    }
  ]
}
```

---

#### `GET /api/signing-requests/{id}`

Get detailed status of a specific signing request.

**Path Parameters**

| Parameter | Type | Description        |
| --------- | ---- | ------------------ |
| `id`      | uuid | Signing request ID |

**Response** `200 OK`

```json
{
  "id": "uuid",
  "wallet_index": 0,
  "sender_address": "Base58SenderAddress",
  "recipient": "Base58RecipientAddress",
  "amount_lamports": 500000000,
  "status": "round1_in_progress",
  "created_at": "2026-04-21T12:15:00Z",
  "updated_at": "2026-04-21T12:16:00Z",
  "tx_signature": null,
  "explorer_url": null,
  "error_message": null,
  "nodes": {
    "node-a": { "round1": "complete", "round2": "pending" },
    "node-b": { "round1": "pending", "round2": "pending" }
  }
}
```

When broadcasting has succeeded:

```json
{
  "id": "uuid",
  "wallet_index": 0,
  "sender_address": "Base58SenderAddress",
  "recipient": "Base58RecipientAddress",
  "amount_lamports": 500000000,
  "status": "confirmed",
  "created_at": "2026-04-21T12:15:00Z",
  "updated_at": "2026-04-21T12:20:00Z",
  "tx_signature": "SolanaTransactionSignatureBase58",
  "explorer_url": "https://explorer.solana.com/tx/SolanaTransactionSignatureBase58?cluster=devnet",
  "error_message": null,
  "nodes": {
    "node-a": { "round1": "complete", "round2": "complete" },
    "node-b": { "round1": "complete", "round2": "complete" }
  }
}
```

**Errors**

| Code | Error Code                   | Condition                           |
| ---- | ---------------------------- | ----------------------------------- |
| 404  | `SIGNING_REQUEST_NOT_FOUND`  | No signing request with this ID     |

---

#### `POST /api/signing-requests/{id}/round/{round}/node/{node_id}`

Trigger a specific node to execute a specific signing round for a signing request. The Coordinator forwards the request to the target node with required context (transaction message, wallet index, nonce commitments from round 1, etc.), then stores the result.

**Path Parameters**

| Parameter | Type    | Description                            |
| --------- | ------- | -------------------------------------- |
| `id`      | uuid    | Signing request ID                     |
| `round`   | integer | Round number: 1 or 2                   |
| `node_id` | string  | Node identifier: `node-a` or `node-b`  |

**Request Body**: None

**Preconditions by Round**

- Round 1: Signing request must be in `pending` or `round1_in_progress` status.
- Round 2: Both nodes must have completed Round 1. Signing request must be in `round1_in_progress` or `round2_in_progress` status. The Coordinator supplies both nodes' nonce commitments and the transaction message bytes.

**Response** `200 OK`

```json
{
  "signing_request_id": "uuid",
  "node_id": "node-a",
  "round": 1,
  "status": "complete",
  "signing_request_status": "round1_in_progress",
  "nodes": {
    "node-a": { "round1": "complete", "round2": "pending" },
    "node-b": { "round1": "pending", "round2": "pending" }
  }
}
```

Status transitions:
- First node completes Round 1: signing request moves to `round1_in_progress`
- First node completes Round 2: signing request moves to `round2_in_progress`

**Errors**

| Code | Error Code                   | Condition                                              |
| ---- | ---------------------------- | ------------------------------------------------------ |
| 400  | `INVALID_ROUND`              | Round is not 1 or 2                                    |
| 400  | `INVALID_NODE_ID`            | Node ID is not `node-a` or `node-b`                    |
| 404  | `SIGNING_REQUEST_NOT_FOUND`  | No signing request with this ID                        |
| 409  | `ROUND_ALREADY_COMPLETE`     | This node already completed this round for this request|
| 409  | `ROUND_PRECONDITION`         | Preconditions not met (details in message)             |
| 409  | `INVALID_STATUS`             | Signing request is in a terminal state                 |
| 502  | `NODE_UNAVAILABLE`           | Could not reach the target node                        |
| 502  | `NODE_ERROR`                 | Node returned an error during the round                |

---

#### `POST /api/signing-requests/{id}/aggregate`

Aggregate signature shares from both nodes into the final Ed25519 signature, construct the Solana transaction, and broadcast to Devnet. After broadcasting, the Coordinator confirms the transaction.

**Request Body**: None

**Preconditions**

- Both nodes must have completed both Round 1 and Round 2.
- Signing request must be in `round2_in_progress` status.

**Response** `200 OK`

On successful broadcast:

```json
{
  "signing_request_id": "uuid",
  "status": "broadcasted",
  "tx_signature": "SolanaTransactionSignatureBase58",
  "explorer_url": "https://explorer.solana.com/tx/SolanaTransactionSignatureBase58?cluster=devnet"
}
```

The Coordinator updates the status asynchronously from `broadcasted` to `confirmed` once the Solana network confirms the transaction, or to `failed` if confirmation times out or the transaction fails.

**Errors**

| Code | Error Code                   | Condition                                         |
| ---- | ---------------------------- | ------------------------------------------------- |
| 404  | `SIGNING_REQUEST_NOT_FOUND`  | No signing request with this ID                   |
| 409  | `ROUND_PRECONDITION`         | Not all nodes have completed both signing rounds  |
| 409  | `INVALID_STATUS`             | Not in `round2_in_progress` status                |
| 500  | `AGGREGATION_FAILED`         | Signature aggregation failed                      |
| 502  | `BROADCAST_FAILED`           | Failed to broadcast transaction to Solana Devnet  |

---

## Coordinator to Node API

These endpoints are called by the Coordinator. They are not intended for direct frontend access.

### DKG Round Endpoints

#### `POST /api/dkg/round1`

Execute DKG Round 1: generate the node's commitment package (containing the node's public commitment to its secret polynomial).

**Request Body**

```json
{
  "session_id": "uuid"
}
```

| Field        | Type | Required | Description      |
| ------------ | ---- | -------- | ---------------- |
| `session_id` | uuid | yes      | DKG session UUID |

**Response** `200 OK`

```json
{
  "node_id": "node-a",
  "session_id": "uuid",
  "round1_package": { ... }
}
```

The `round1_package` field contains the serialized `frost_ed25519::keys::dkg::round1::Package` (serde JSON). This is opaque to the Coordinator; it stores it and forwards it to the other node in Round 2.

The node also persists its Round 1 secret package internally (never sent to the Coordinator).

**Errors**

| Code | Error Code             | Condition                                  |
| ---- | ---------------------- | ------------------------------------------ |
| 409  | `ROUND_ALREADY_DONE`   | This node already completed Round 1        |
| 500  | `CRYPTO_ERROR`         | Cryptographic operation failed             |

---

#### `POST /api/dkg/round2`

Execute DKG Round 2: using the other node's Round 1 package, generate encrypted share packages.

**Request Body**

```json
{
  "session_id": "uuid",
  "round1_packages": {
    "node-b": { ... }
  }
}
```

| Field              | Type   | Required | Description                                                      |
| ------------------ | ------ | -------- | ---------------------------------------------------------------- |
| `session_id`       | uuid   | yes      | DKG session UUID                                                 |
| `round1_packages`  | object | yes      | Map of other participants' node ID to their Round 1 package JSON |

**Response** `200 OK`

```json
{
  "node_id": "node-a",
  "session_id": "uuid",
  "round2_package": { ... }
}
```

The `round2_package` field contains the serialized `frost_ed25519::keys::dkg::round2::Package` intended for the other node. The node persists its own Round 2 secret data internally.

**Errors**

| Code | Error Code             | Condition                                     |
| ---- | ---------------------- | --------------------------------------------- |
| 409  | `ROUND_ALREADY_DONE`   | This node already completed Round 2           |
| 409  | `ROUND_PRECONDITION`   | Round 1 not completed for this node           |
| 400  | `INVALID_PACKAGES`     | Missing or malformed Round 1 packages         |
| 500  | `CRYPTO_ERROR`         | Cryptographic operation failed                |

---

#### `POST /api/dkg/round3`

Execute DKG Round 3 (finalize): using all Round 1 and Round 2 packages, verify and compute the final key share and group verifying key.

**Request Body**

```json
{
  "session_id": "uuid",
  "round1_packages": {
    "node-b": { ... }
  },
  "round2_packages": {
    "node-b": { ... }
  }
}
```

| Field              | Type   | Required | Description                                            |
| ------------------ | ------ | -------- | ------------------------------------------------------ |
| `session_id`       | uuid   | yes      | DKG session UUID                                       |
| `round1_packages`  | object | yes      | Map of other participants' node ID to Round 1 package  |
| `round2_packages`  | object | yes      | Map of other participants' node ID to Round 2 package  |

**Response** `200 OK`

```json
{
  "node_id": "node-a",
  "session_id": "uuid",
  "group_public_key": "Base58EncodedGroupVerifyingKey",
  "verifying_share": "Base58EncodedVerifyingShare"
}
```

The node persists its `KeyPackage` (containing the private key share) and `PublicKeyPackage` (containing verifying shares for all participants) internally. The private key share never leaves the node.

The `group_public_key` is returned so the Coordinator can verify that both nodes computed the same group key.

**Errors**

| Code | Error Code             | Condition                                     |
| ---- | ---------------------- | --------------------------------------------- |
| 409  | `ROUND_ALREADY_DONE`   | This node already completed Round 3           |
| 409  | `ROUND_PRECONDITION`   | Round 2 not completed for this node           |
| 400  | `INVALID_PACKAGES`     | Missing or malformed packages                 |
| 400  | `VERIFICATION_FAILED`  | Share verification failed (packages tampered) |
| 500  | `CRYPTO_ERROR`         | Cryptographic operation failed                |

---

### Signing Round Endpoints

#### `POST /api/signing/round1`

Execute Signing Round 1: generate nonce commitments for a signing request. The node derives the child key share for the given wallet index from its stored root key share using HD derivation, then produces signing nonces and commitments.

**Request Body**

```json
{
  "signing_request_id": "uuid",
  "wallet_index": 0
}
```

| Field                 | Type    | Required | Description                         |
| --------------------- | ------- | -------- | ----------------------------------- |
| `signing_request_id`  | uuid    | yes      | Signing request UUID                |
| `wallet_index`        | integer | yes      | HD derivation index for the wallet  |

**Response** `200 OK`

```json
{
  "node_id": "node-a",
  "signing_request_id": "uuid",
  "commitments": { ... }
}
```

The `commitments` field contains the serialized `frost_ed25519::round1::SigningCommitments` (serde JSON). The node stores the corresponding `SigningNonces` internally (never sent to the Coordinator).

**Errors**

| Code | Error Code             | Condition                                     |
| ---- | ---------------------- | --------------------------------------------- |
| 409  | `ROUND_ALREADY_DONE`   | This node already completed Signing Round 1   |
| 409  | `DKG_NOT_COMPLETE`     | No completed DKG key share available          |
| 500  | `CRYPTO_ERROR`         | Cryptographic operation failed                |

---

#### `POST /api/signing/round2`

Execute Signing Round 2: compute the signature share. The node uses its stored nonces, the derived child key share, both nodes' commitments, and the transaction message to produce its signature share.

**Request Body**

```json
{
  "signing_request_id": "uuid",
  "wallet_index": 0,
  "message": "Base64EncodedTransactionMessage",
  "commitments": {
    "node-a": { ... },
    "node-b": { ... }
  }
}
```

| Field                 | Type    | Required | Description                                                    |
| --------------------- | ------- | -------- | -------------------------------------------------------------- |
| `signing_request_id`  | uuid    | yes      | Signing request UUID                                           |
| `wallet_index`        | integer | yes      | HD derivation index for the wallet                             |
| `message`             | string  | yes      | Base64-encoded transaction message bytes to sign               |
| `commitments`         | object  | yes      | Map of all participants' node ID to their Round 1 commitments  |

**Response** `200 OK`

```json
{
  "node_id": "node-a",
  "signing_request_id": "uuid",
  "signature_share": { ... }
}
```

The `signature_share` field contains the serialized `frost_ed25519::round2::SignatureShare` (serde JSON).

**Errors**

| Code | Error Code             | Condition                                                    |
| ---- | ---------------------- | ------------------------------------------------------------ |
| 409  | `ROUND_ALREADY_DONE`   | This node already completed Signing Round 2 for this request |
| 409  | `ROUND_PRECONDITION`   | Signing Round 1 not completed for this node                  |
| 400  | `INVALID_COMMITMENTS`  | Missing or malformed commitments                             |
| 400  | `INVALID_MESSAGE`      | Message is not valid base64                                  |
| 500  | `CRYPTO_ERROR`         | Cryptographic operation failed                               |

---

## Signing Request Lifecycle

The signing request progresses through these statuses:

```
pending --> round1_in_progress --> round2_in_progress --> aggregating --> broadcasted --> confirmed
                                                                    \               \
                                                                     --> failed       --> failed
```

| Status               | Description                                               |
| -------------------- | --------------------------------------------------------- |
| `pending`            | Created, no signing rounds started                        |
| `round1_in_progress` | At least one node has started or completed Signing Round 1|
| `round2_in_progress` | Both nodes completed Round 1; at least one started Round 2|
| `aggregating`        | Both nodes completed Round 2; aggregation in progress     |
| `broadcasted`        | Transaction broadcast to Solana Devnet                    |
| `confirmed`          | Transaction confirmed on Solana Devnet                    |
| `failed`             | An error occurred (see `error_message` for details)       |

---

## Data Flow Summary

### DKG Flow

```
Frontend                    Coordinator                     Node A          Node B
  |                              |                            |               |
  |-- POST /api/dkg/start ----->|                            |               |
  |<-- 201 session created -----|                            |               |
  |                              |                            |               |
  |-- POST /api/dkg/round/1    |                            |               |
  |   /node/node-a ----------->|-- POST /api/dkg/round1 -->|               |
  |                              |<-- round1_package --------|               |
  |<-- 200 round complete ------|  (stores package)          |               |
  |                              |                            |               |
  |-- POST /api/dkg/round/1    |                            |               |
  |   /node/node-b ----------->|-- POST /api/dkg/round1 ---|-------------->|
  |                              |<-- round1_package --------|---------------|
  |<-- 200 round complete ------|  (stores package)          |               |
  |                              |                            |               |
  |-- POST /api/dkg/round/2    |                            |               |
  |   /node/node-a ----------->|-- POST /api/dkg/round2 -->|               |
  |                              |   {node-b's round1_pkg}   |               |
  |                              |<-- round2_package --------|               |
  |<-- 200 round complete ------|  (stores package)          |               |
  |                              |                            |               |
  |   (same for node-b round2)  |                            |               |
  |                              |                            |               |
  |-- POST /api/dkg/round/3    |                            |               |
  |   /node/node-a ----------->|-- POST /api/dkg/round3 -->|               |
  |                              |   {round1+round2 pkgs}    |               |
  |                              |<-- group_public_key ------|               |
  |<-- 200 round complete ------|  (stores group key)        |               |
  |                              |                            |               |
  |   (same for node-b round3)  |                            |               |
  |<-- 200 DKG complete --------|                            |               |
```

### Signing Flow

```
Frontend                    Coordinator                     Node A          Node B
  |                              |                            |               |
  |-- POST /api/signing-       |                            |               |
  |   requests --------------->|  (creates request)          |               |
  |<-- 201 request created -----|                            |               |
  |                              |                            |               |
  |-- POST .../round/1/        |                            |               |
  |   node/node-a ------------>|-- POST /api/signing/      |               |
  |                              |   round1 {wallet_index} ->|               |
  |                              |<-- commitments ------------|               |
  |<-- 200 round complete ------|  (stores commitments)      |               |
  |                              |                            |               |
  |   (same for node-b round1)  |                            |               |
  |                              |                            |               |
  |-- POST .../round/2/        |                            |               |
  |   node/node-a ------------>|-- POST /api/signing/      |               |
  |                              |   round2 {msg,            |               |
  |                              |    commitments,            |               |
  |                              |    wallet_index} --------->|               |
  |                              |<-- signature_share --------|               |
  |<-- 200 round complete ------|  (stores share)            |               |
  |                              |                            |               |
  |   (same for node-b round2)  |                            |               |
  |                              |                            |               |
  |-- POST .../aggregate ----->|  (aggregates shares,       |               |
  |                              |   builds tx, broadcasts)   |               |
  |<-- 200 broadcasted ---------|                            |               |
  |                              |  (confirms tx async)       |               |
  |-- GET .../signing-         |                            |               |
  |   requests/{id} ---------->|                            |               |
  |<-- 200 confirmed ----------|                            |               |
```
