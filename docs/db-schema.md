# Database Schema Design

Schema design for the FROST 2-of-2 TSS Solana Wallet system. A single PostgreSQL 18 instance hosts three databases:

| Database          | Owner Service | Purpose                                    |
| ----------------- | ------------- | ------------------------------------------ |
| `coordinator_db`  | Coordinator   | Orchestration state, wallets, transactions |
| `node_a_db`       | Node A        | Key shares, DKG round secrets, nonces      |
| `node_b_db`       | Node B        | Same schema as Node A                      |

Node A and Node B use identical schemas but separate databases. Each node stores only its own cryptographic material.

---

## Design Principles

1. **UUIDs everywhere**: all primary keys are `UUID` (v4), generated application-side.
2. **JSONB for FROST data**: frost-ed25519 types implement serde `Serialize`/`Deserialize`. Storing as JSONB gives queryability and avoids custom binary encoding.
3. **Strict data separation**: the Coordinator never stores private key shares. Nodes never store other nodes' data or wallet addresses.
4. **Timestamps**: all `_at` columns use `TIMESTAMPTZ` (UTC).
5. **Enums**: PostgreSQL native enums for status fields.

---

## Coordinator Database (`coordinator_db`)

### Enum Types

```sql
CREATE TYPE dkg_status AS ENUM (
    'initialized',
    'in_progress',
    'complete',
    'failed'
);

CREATE TYPE round_status AS ENUM (
    'pending',
    'complete',
    'failed'
);

CREATE TYPE signing_request_status AS ENUM (
    'pending',
    'round1_in_progress',
    'round2_in_progress',
    'aggregating',
    'broadcasted',
    'confirmed',
    'failed'
);
```

### `dkg_sessions`

Tracks the overall DKG session. The system supports exactly one root key, so at most one non-failed session should exist.

| Column             | Type            | Constraints       | Description                                   |
| ------------------ | --------------- | ----------------- | --------------------------------------------- |
| `id`               | `UUID`          | PK                | Session identifier                            |
| `status`           | `dkg_status`    | NOT NULL          | Overall session status                        |
| `group_public_key` | `TEXT`           | NULL              | Base58-encoded group verifying key (set on completion) |
| `created_at`       | `TIMESTAMPTZ`   | NOT NULL, DEFAULT | Session creation time                         |
| `completed_at`     | `TIMESTAMPTZ`   | NULL              | Time DKG completed                            |
| `updated_at`       | `TIMESTAMPTZ`   | NOT NULL, DEFAULT | Last status change                            |

### `dkg_round_state`

Tracks per-node, per-round progress and stores the output packages that the Coordinator needs to forward between nodes.

| Column           | Type           | Constraints                   | Description                                        |
| ---------------- | -------------- | ----------------------------- | -------------------------------------------------- |
| `id`             | `UUID`         | PK                            | Row identifier                                     |
| `session_id`     | `UUID`         | NOT NULL, FK -> dkg_sessions  | Parent DKG session                                 |
| `node_id`        | `TEXT`          | NOT NULL                      | Node identifier (`node-a` or `node-b`)             |
| `round`          | `SMALLINT`     | NOT NULL                      | Round number (1, 2, or 3)                          |
| `status`         | `round_status` | NOT NULL, DEFAULT 'pending'   | This node's round status                           |
| `output_package` | `JSONB`        | NULL                          | The node's output for this round (public data only)|
| `created_at`     | `TIMESTAMPTZ`  | NOT NULL, DEFAULT             | Row creation time                                  |
| `updated_at`     | `TIMESTAMPTZ`  | NOT NULL, DEFAULT             | Last update                                        |

**Unique constraint**: `(session_id, node_id, round)` -- each node executes each round exactly once per session.

Note: `output_package` stores only public FROST packages that are safe to hold on the Coordinator. For DKG Round 1, this is the commitment package. For Round 2, this is the encrypted share package intended for the other node. For Round 3, this stores the verifying share returned by the node (the private key share stays on the node).

### `wallets`

Derived HD wallets. The Coordinator derives child public keys from the group verifying key without contacting nodes.

| Column           | Type          | Constraints      | Description                                    |
| ---------------- | ------------- | ---------------- | ---------------------------------------------- |
| `index`          | `INTEGER`     | PK               | Sequential derivation index (0, 1, 2, ...)     |
| `address`        | `TEXT`         | NOT NULL, UNIQUE | Solana address (Base58)                        |
| `public_key`     | `TEXT`         | NOT NULL         | Derived child public key (Base58)              |
| `chain_code`     | `BYTEA`       | NULL             | HD derivation chain code (if needed for re-derivation) |
| `created_at`     | `TIMESTAMPTZ` | NOT NULL, DEFAULT| Wallet creation time                           |

The `index` column serves as the natural primary key since wallet derivation is strictly sequential.

### `signing_requests`

Each signing request represents a SOL transfer transaction that progresses through the signing lifecycle.

| Column           | Type                       | Constraints       | Description                                           |
| ---------------- | -------------------------- | ----------------- | ----------------------------------------------------- |
| `id`             | `UUID`                     | PK                | Signing request identifier                            |
| `wallet_index`   | `INTEGER`                  | NOT NULL, FK -> wallets | Sender wallet derivation index                  |
| `recipient`      | `TEXT`                     | NOT NULL          | Recipient Solana address (Base58)                     |
| `amount_lamports`| `BIGINT`                   | NOT NULL          | Transfer amount in lamports                           |
| `status`         | `signing_request_status`   | NOT NULL, DEFAULT 'pending' | Current lifecycle status                   |
| `tx_message`     | `BYTEA`                    | NULL              | Serialized Solana transaction message (set before Round 2) |
| `tx_signature`   | `TEXT`                     | NULL              | Solana transaction signature (set after broadcast)    |
| `error_message`  | `TEXT`                     | NULL              | Error details (set on failure)                        |
| `created_at`     | `TIMESTAMPTZ`              | NOT NULL, DEFAULT | Request creation time                                 |
| `updated_at`     | `TIMESTAMPTZ`              | NOT NULL, DEFAULT | Last status change                                    |

### `signing_round_state`

Tracks per-node, per-round signing progress and stores the cryptographic outputs the Coordinator needs for aggregation.

| Column               | Type           | Constraints                       | Description                                          |
| -------------------- | -------------- | --------------------------------- | ---------------------------------------------------- |
| `id`                 | `UUID`         | PK                                | Row identifier                                       |
| `signing_request_id` | `UUID`         | NOT NULL, FK -> signing_requests  | Parent signing request                               |
| `node_id`            | `TEXT`          | NOT NULL                          | Node identifier (`node-a` or `node-b`)               |
| `round`              | `SMALLINT`     | NOT NULL                          | Round number (1 or 2)                                |
| `status`             | `round_status` | NOT NULL, DEFAULT 'pending'       | This node's round status                             |
| `output_data`        | `JSONB`        | NULL                              | Round output: commitments (R1) or signature share (R2)|
| `created_at`         | `TIMESTAMPTZ`  | NOT NULL, DEFAULT                 | Row creation time                                    |
| `updated_at`         | `TIMESTAMPTZ`  | NOT NULL, DEFAULT                 | Last update                                          |

**Unique constraint**: `(signing_request_id, node_id, round)` -- each node executes each round exactly once per signing request.

---

## Node Database (`node_a_db` / `node_b_db`)

Both nodes use an identical schema. Each node's database stores only that node's own cryptographic secrets.

### `key_shares`

Stores the root key material produced by a completed DKG.

| Column              | Type          | Constraints       | Description                                                |
| ------------------- | ------------- | ----------------- | ---------------------------------------------------------- |
| `id`                | `UUID`        | PK                | Row identifier                                             |
| `session_id`        | `UUID`        | NOT NULL, UNIQUE  | DKG session UUID (matches Coordinator's session ID)        |
| `key_package`       | `JSONB`       | NOT NULL          | Serialized `frost_ed25519::keys::KeyPackage` (private share) |
| `public_key_package`| `JSONB`       | NOT NULL          | Serialized `frost_ed25519::keys::PublicKeyPackage`         |
| `group_public_key`  | `TEXT`        | NOT NULL          | Base58 group verifying key (for quick lookup)              |
| `created_at`        | `TIMESTAMPTZ` | NOT NULL, DEFAULT | When the key share was generated                           |

Only one key share should exist per completed DKG session. The `UNIQUE` constraint on `session_id` enforces this.

### `dkg_round_data`

Stores intermediate DKG round secrets that the node needs to carry forward between rounds.

| Column          | Type          | Constraints                   | Description                                          |
| --------------- | ------------- | ----------------------------- | ---------------------------------------------------- |
| `id`            | `UUID`        | PK                            | Row identifier                                       |
| `session_id`    | `UUID`        | NOT NULL                      | DKG session UUID                                     |
| `round`         | `SMALLINT`    | NOT NULL                      | Round number (1 or 2)                                |
| `secret_package`| `JSONB`       | NOT NULL                      | The node's secret data for this round                |
| `created_at`    | `TIMESTAMPTZ` | NOT NULL, DEFAULT             | Row creation time                                    |

**Unique constraint**: `(session_id, round)` -- one secret per round per session.

Contents by round:
- Round 1: The `round1::SecretPackage` -- needed for Round 2 computation.
- Round 2: The `round2::SecretPackage` -- needed for Round 3 finalization.

This data can be cleaned up after DKG completion but is retained for auditability.

### `signing_nonces`

Stores signing nonces generated during Signing Round 1. These are consumed (and should be invalidated) during Signing Round 2.

| Column               | Type          | Constraints       | Description                                            |
| -------------------- | ------------- | ----------------- | ------------------------------------------------------ |
| `id`                 | `UUID`        | PK                | Row identifier                                         |
| `signing_request_id` | `UUID`        | NOT NULL, UNIQUE  | Signing request UUID (matches Coordinator's request ID)|
| `nonces`             | `JSONB`       | NOT NULL          | Serialized `frost_ed25519::round1::SigningNonces`      |
| `created_at`         | `TIMESTAMPTZ` | NOT NULL, DEFAULT | When nonces were generated                             |

The `UNIQUE` constraint on `signing_request_id` ensures nonces are only generated once per signing request per node (nonce reuse would be a security vulnerability).

**Important**: After Signing Round 2 consumes the nonces, the row should be deleted or marked as consumed to prevent accidental reuse.

---

## Entity Relationship Diagram (Text)

### Coordinator DB

```
dkg_sessions (1) ---< dkg_round_state (many)
    |                    [session_id, node_id, round] UNIQUE
    |
wallets (1) ---< signing_requests (many)
                     |
                     +---< signing_round_state (many)
                              [signing_request_id, node_id, round] UNIQUE
```

### Node DB

```
key_shares
    [session_id] UNIQUE

dkg_round_data
    [session_id, round] UNIQUE

signing_nonces
    [signing_request_id] UNIQUE
```

---

## Index Strategy

### Coordinator DB

| Table                 | Index                                              | Purpose                           |
| --------------------- | -------------------------------------------------- | --------------------------------- |
| `dkg_sessions`        | PK on `id`                                         | Lookup by ID                      |
| `dkg_sessions`        | `idx_dkg_sessions_status` on `status`              | Find active/complete session      |
| `dkg_round_state`     | UNIQUE on `(session_id, node_id, round)`           | Enforce one execution per combo   |
| `wallets`             | PK on `index`                                      | Lookup by index                   |
| `wallets`             | UNIQUE on `address`                                | Prevent duplicate addresses       |
| `signing_requests`    | PK on `id`                                         | Lookup by ID                      |
| `signing_requests`    | `idx_signing_requests_status` on `status`          | Filter by status                  |
| `signing_requests`    | `idx_signing_requests_created` on `created_at DESC`| List in order                     |
| `signing_round_state` | UNIQUE on `(signing_request_id, node_id, round)`   | Enforce one execution per combo   |

### Node DB

| Table            | Index                                     | Purpose                         |
| ---------------- | ----------------------------------------- | ------------------------------- |
| `key_shares`     | PK on `id`                                | Lookup by ID                    |
| `key_shares`     | UNIQUE on `session_id`                    | One share per DKG session       |
| `dkg_round_data` | UNIQUE on `(session_id, round)`           | One secret per round            |
| `signing_nonces` | UNIQUE on `signing_request_id`            | One nonce set per signing req   |

---

## Security Considerations

1. **Key share isolation**: The `key_shares` table exists only on node databases. The Coordinator DB has no column or table for private key material.
2. **Nonce single-use**: The `signing_nonces` table has a UNIQUE constraint on `signing_request_id`. Application code must delete or mark nonces as consumed after Round 2 to prevent reuse, which would compromise the private key.
3. **JSONB encryption at rest**: For production, consider PostgreSQL transparent data encryption (TDE) or application-level encryption of JSONB columns containing sensitive cryptographic material on the nodes.
4. **Secret package cleanup**: `dkg_round_data` rows can be deleted after successful DKG completion. Retained here for debugging and auditability.
5. **No cross-node data**: Each node database contains only that node's own secrets. Node A's database never contains Node B's packages and vice versa.
