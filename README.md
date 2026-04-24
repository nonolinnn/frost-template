
# FROST 2-of-2 TSS Solana Wallet

A threshold signature scheme (TSS) wallet demo using
[FROST](https://eprint.iacr.org/2020/852) (Flexible Round-Optimized Schnorr Threshold
Signatures) on Solana Devnet. Two signer nodes collaboratively generate keys, derive
wallets, and sign transactions — no single node ever holds the full private key.

## Architecture

```
┌─────────────┐       ┌──────────────────┐       ┌─────────────┐
│  Frontend   │       │   Coordinator    │       │  PostgreSQL │
│  (Next.js)  │──────▶│   (Rust/axum)    │──────▶│   (PG 18)   │
│  :3000      │       │   :8080          │       │   :5432     │
└─────────────┘       └────────┬─────────┘       └─────────────┘
                               │
                    ┌──────────┴──────────┐
                    │                     │
              ┌─────▼─────┐         ┌─────▼─────┐
              │  Node A   │         │  Node B   │
              │  (Rust)   │         │  (Rust)   │
              │  :8081    │         │  :8082    │
              └───────────┘         └───────────┘
```

- **Frontend** — React UI for driving DKG, wallet, and signing flows
- **Coordinator** — Orchestrates protocol rounds, stores public data, broadcasts
transactions
- **Node A / B** — Hold secret key shares, perform FROST signing rounds
- **PostgreSQL** — Shared instance with per-service databases (`coordinator_db`,
`node_a_db`, `node_b_db`)

## Quick Start

### Prerequisites

- [Docker Desktop](https://www.docker.com/products/docker-desktop/) (Docker Compose v2)

### Run

```bash
# Clone and start
git clone <https://github.com/nonolinnn/frost-template>
cd frost-template
docker compose up --build
```

Wait for all services to become healthy, then open **http://localhost:3000**.

> First build takes several minutes (Rust compilation). Subsequent starts are cached.

## Usage Guide

### 1. Distributed Key Generation (DKG)

1. Open the **DKG** tab
2. Execute Round 1 for Node A, then Node B
3. Execute Round 2 for Node A, then Node B
4. Execute Round 3 for Node A, then Node B
5. DKG completes → Master Public Key is displayed

### 2. Derive Wallet

1. Switch to the **Wallets** tab
2. Click **Derive Wallet** — generates a child wallet (Solana address) from the master key
3. The address appears with its balance (initially 0 SOL)

### 3. Fund Wallet (Devnet Airdrop)

```bash
solana airdrop 1 <wallet-address> --url devnet
```

> Requires the [Solana CLI](https://docs.solanalabs.com/cli/install). Refresh the Wallets
tab to see the updated balance.

### 4. Sign & Broadcast Transaction

1. Switch to the **Transactions** tab
2. Select sender wallet, enter recipient address and amount
3. Execute Signing Round 1 for Node A, then Node B
4. Execute Signing Round 2 for Node A, then Node B
5. Click **Aggregate & Broadcast**
6. On success, click the Solana Explorer link to view the transaction

> Without funding, the signing flow completes normally but broadcast fails with insufficient
balance. This is expected — the FROST signature is still valid.

## Environment Variables

Copy and customize if needed:

```bash
cp .env.example .env
```

| Variable | Default | Description |
|----------|---------|-------------|
| `SOLANA_RPC_URL` | `https://api.devnet.solana.com` | Solana RPC endpoint |
| `POSTGRES_USER` | `frost` | PostgreSQL username |
| `POSTGRES_PASSWORD` | `frost` | PostgreSQL password |
| `COORDINATOR_PORT` | `8080` | Coordinator host port |
| `NODE_A_PORT` | `8081` | Node A host port |
| `NODE_B_PORT` | `8082` | Node B host port |
| `FRONTEND_PORT` | `3000` | Frontend host port |
| `NEXT_PUBLIC_API_URL` | `http://localhost:8080` | Coordinator URL for browser |

See `.env.example` for the full list.

## Testing

### Unit Tests

```bash
cd backend
cargo test --workspace
```

15 tests covering DKG round-trip correctness, HD wallet derivation consistency, and
threshold signing verification.

### Integration Tests

With Docker services running:

```bash
chmod +x tests/integration-test.sh
./tests/integration-test.sh
```

Exercises the full happy path via API: DKG → wallet derivation → signing → aggregate.

## Tech Stack

| Layer | Technology | Version |
|-------|-----------|---------|
| Frontend | Next.js + React + TypeScript | 16.1 / 19 |
| Styling | Tailwind CSS | v4 |
| Backend | Rust + axum | 1.94 / 0.8 |
| Cryptography | frost-ed25519 | 2.2 |
| HD Derivation | hd-wallet (Edwards) | 0.6 |
| Database | PostgreSQL + sqlx | 18 / 0.8 |
| Blockchain | Solana Devnet (solana-client) | v3 |
| Container | Docker Compose | v2 |

## Project Structure

```
frost-template/
├── docker-compose.yml          # Production Docker orchestration
├── .env.example                # Environment variable reference
├── frontend/
│   ├── Dockerfile
│   └── app/
│       ├── page.tsx            # Main shell with tab navigation
│       ├── components/
│       │   ├── dkg-panel.tsx       # DKG interface
│       │   ├── wallets-panel.tsx   # Wallet management
│       │   └── transactions-panel.tsx  # Signing flow UI
│       └── lib/api.ts          # Typed API client
├── backend/
│   ├── Cargo.toml              # Workspace root
│   ├── Dockerfile
│   ├── coordinator/
│   │   ├── src/
│   │   │   ├── main.rs         # axum server setup
│   │   │   ├── routes/         # DKG, wallets, signing handlers
│   │   │   ├── derivation.rs   # FROST ↔ hd-wallet bridging
│   │   │   ├── db/             # sqlx queries
│   │   │   └── models/         # Request/response types
│   │   └── migrations/
│   └── tss-node/
│       ├── src/
│       │   ├── main.rs
│       │   ├── routes/         # DKG round, signing round handlers
│       │   ├── derivation.rs   # Child key share derivation
│       │   └── db/
│       └── migrations/
├── tests/
│   └── integration-test.sh     # End-to-end API test script
├── docker/
│   └── postgres/
│       └── init-databases.sql  # Multi-database initialization
    └── ai-journal.md           # AI development journal
```

## Development Setup (without Docker)

### Prerequisites

- Node.js 24.14+ / Rust 1.94+ (use `mise install`)
- PostgreSQL 18 running locally

### Frontend

```bash
cd frontend
npm install
npm run dev           # http://localhost:3000
```

### Backend

```bash
# Create databases
createdb coordinator_db
createdb node_a_db
createdb node_b_db

# Run services (in separate terminals)
cd backend
DATABASE_URL=postgresql://localhost/coordinator_db cargo run -p coordinator
NODE_ID=node-a DATABASE_URL=postgresql://localhost/node_a_db cargo run -p tss-node
NODE_ID=node-b DATABASE_URL=postgresql://localhost/node_b_db PORT=8082 cargo run -p tss-node
```

## AI Development Journal

See [`docs/ai-journal.md`](docs/ai-journal.md) for the complete record of AI-assisted
development, including prompt engineering decisions, review judgments, and course
corrections throughout the build process.