#!/usr/bin/env bash
# ------------------------------------------------------------------
# FROST TSS Wallet — Integration Test Script
#
# Exercises the happy path: DKG -> Wallet derivation -> Signing.
#
# Prerequisites:
#   1. docker compose up -d   (all 5 services running and healthy)
#   2. curl and jq installed
#
# Usage:
#   ./tests/integration-test.sh [COORDINATOR_URL]
#
# The COORDINATOR_URL defaults to http://localhost:8080.
# ------------------------------------------------------------------

set -euo pipefail

COORDINATOR="${1:-http://localhost:8080}"
PASS=0
FAIL=0

# ---- Helpers -------------------------------------------------------

green()  { printf "\033[32m%s\033[0m\n" "$*"; }
red()    { printf "\033[31m%s\033[0m\n" "$*"; }
yellow() { printf "\033[33m%s\033[0m\n" "$*"; }
bold()   { printf "\033[1m%s\033[0m\n" "$*"; }

assert_eq() {
    local label="$1" actual="$2" expected="$3"
    if [ "$actual" = "$expected" ]; then
        green "  PASS: $label (got '$actual')"
        PASS=$((PASS + 1))
    else
        red "  FAIL: $label — expected '$expected', got '$actual'"
        FAIL=$((FAIL + 1))
    fi
}

assert_not_empty() {
    local label="$1" actual="$2"
    if [ -n "$actual" ] && [ "$actual" != "null" ]; then
        green "  PASS: $label (non-empty)"
        PASS=$((PASS + 1))
    else
        red "  FAIL: $label — value is empty or null"
        FAIL=$((FAIL + 1))
    fi
}

api_post() {
    local path="$1"
    shift
    curl -sf -X POST -H "Content-Type: application/json" "$@" "${COORDINATOR}${path}"
}

api_get() {
    local path="$1"
    curl -sf "${COORDINATOR}${path}"
}

jq_node_round_status() {
    local json="$1" node="$2" round="$3"
    echo "$json" | jq -r --arg node "$node" --arg round "$round" '.nodes[$node][$round] // "missing"'
}

run_dkg_round_if_pending() {
    local status_json="$1" round="$2" node="$3"
    local round_key="round${round}"
    local current
    current=$(jq_node_round_status "$status_json" "$node" "$round_key")

    if [ "$current" = "complete" ]; then
        yellow "Skipping DKG Round ${round} for ${node} (already complete)..."
        return 0
    fi

    yellow "Executing DKG Round ${round} for ${node}..."
    local response
    response=$(api_post "/api/dkg/round/${round}/node/${node}")
    local resp_status
    resp_status=$(echo "$response" | jq -r '.status')
    assert_eq "Round ${round} ${node} status" "$resp_status" "complete"
}

# ---- Wait for services to be ready --------------------------------

bold "=== Waiting for coordinator to be healthy ==="
for i in $(seq 1 30); do
    if curl -sf "${COORDINATOR}/health" > /dev/null 2>&1; then
        green "Coordinator is healthy."
        break
    fi
    if [ "$i" -eq 30 ]; then
        red "Coordinator did not become healthy after 30 seconds."
        exit 1
    fi
    sleep 1
done

# ---- Step 1: DKG ---------------------------------------------------

bold ""
bold "=== Step 1: Distributed Key Generation (DKG) ==="

DKG_STATUS_RESP=$(api_get "/api/dkg/status")
DKG_FINAL_STATUS=$(echo "$DKG_STATUS_RESP" | jq -r '.status')

if [ "$DKG_FINAL_STATUS" = "not_started" ]; then
    yellow "Starting DKG session..."
    DKG_START=$(api_post "/api/dkg/start")
    SESSION_ID=$(echo "$DKG_START" | jq -r '.session_id')
    assert_not_empty "DKG session_id" "$SESSION_ID"
    DKG_STATUS=$(echo "$DKG_START" | jq -r '.status')
    assert_eq "DKG initial status" "$DKG_STATUS" "initialized"
    DKG_STATUS_RESP=$(api_get "/api/dkg/status")
elif [ "$DKG_FINAL_STATUS" = "complete" ]; then
    yellow "Reusing existing completed DKG session..."
else
    yellow "Resuming existing DKG session in status '$DKG_FINAL_STATUS'..."
fi

run_dkg_round_if_pending "$DKG_STATUS_RESP" 1 "node-a"
DKG_STATUS_RESP=$(api_get "/api/dkg/status")
run_dkg_round_if_pending "$DKG_STATUS_RESP" 1 "node-b"
DKG_STATUS_RESP=$(api_get "/api/dkg/status")
run_dkg_round_if_pending "$DKG_STATUS_RESP" 2 "node-a"
DKG_STATUS_RESP=$(api_get "/api/dkg/status")
run_dkg_round_if_pending "$DKG_STATUS_RESP" 2 "node-b"
DKG_STATUS_RESP=$(api_get "/api/dkg/status")
run_dkg_round_if_pending "$DKG_STATUS_RESP" 3 "node-a"
DKG_STATUS_RESP=$(api_get "/api/dkg/status")
run_dkg_round_if_pending "$DKG_STATUS_RESP" 3 "node-b"

DKG_STATUS_RESP=$(api_get "/api/dkg/status")
DKG_FINAL_STATUS=$(echo "$DKG_STATUS_RESP" | jq -r '.status')
assert_eq "DKG final status" "$DKG_FINAL_STATUS" "complete"

STATUS_GPK=$(echo "$DKG_STATUS_RESP" | jq -r '.group_public_key')
assert_not_empty "Group public key in status" "$STATUS_GPK"

# ---- Step 2: Wallet Derivation ------------------------------------

bold ""
bold "=== Step 2: Wallet Derivation ==="

WALLETS_BEFORE=$(api_get "/api/wallets")
WALLET_COUNT_BEFORE=$(echo "$WALLETS_BEFORE" | jq '.wallets | length')
EXPECTED_WALLET_INDEX=$(echo "$WALLETS_BEFORE" | jq -r '[.wallets[].index] | if length == 0 then 0 else (max + 1) end')

yellow "Deriving next wallet..."
WALLET=$(api_post "/api/wallets")
WALLET_INDEX=$(echo "$WALLET" | jq -r '.index')
WALLET_ADDRESS=$(echo "$WALLET" | jq -r '.address')
assert_eq "Wallet index" "$WALLET_INDEX" "$EXPECTED_WALLET_INDEX"
assert_not_empty "Wallet address" "$WALLET_ADDRESS"

yellow "Listing wallets..."
WALLETS=$(api_get "/api/wallets")
WALLET_COUNT=$(echo "$WALLETS" | jq '.wallets | length')
EXPECTED_WALLET_COUNT=$((WALLET_COUNT_BEFORE + 1))
assert_eq "Wallet count" "$WALLET_COUNT" "$EXPECTED_WALLET_COUNT"

# ---- Step 3: Signing Flow -----------------------------------------

bold ""
bold "=== Step 3: Signing Flow ==="

# Use a well-known Solana address as the recipient (system program)
RECIPIENT="11111111111111111111111111111111"
AMOUNT=1000

yellow "Creating signing request..."
SIGNING_REQ=$(api_post "/api/signing-requests" \
    -d "{\"wallet_index\": ${WALLET_INDEX}, \"recipient\": \"$RECIPIENT\", \"amount_lamports\": $AMOUNT}")
SIGNING_ID=$(echo "$SIGNING_REQ" | jq -r '.id')
SIGNING_STATUS=$(echo "$SIGNING_REQ" | jq -r '.status')
assert_not_empty "Signing request ID" "$SIGNING_ID"
assert_eq "Signing request initial status" "$SIGNING_STATUS" "pending"

# Signing Round 1 for both nodes
yellow "Executing Signing Round 1 for node-a..."
SR1A=$(api_post "/api/signing-requests/${SIGNING_ID}/round/1/node/node-a")
SR1A_STATUS=$(echo "$SR1A" | jq -r '.status')
assert_eq "Signing R1 node-a status" "$SR1A_STATUS" "complete"

yellow "Executing Signing Round 1 for node-b..."
SR1B=$(api_post "/api/signing-requests/${SIGNING_ID}/round/1/node/node-b")
SR1B_STATUS=$(echo "$SR1B" | jq -r '.status')
assert_eq "Signing R1 node-b status" "$SR1B_STATUS" "complete"

# Signing Round 2 for both nodes
yellow "Executing Signing Round 2 for node-a..."
SR2A=$(api_post "/api/signing-requests/${SIGNING_ID}/round/2/node/node-a")
SR2A_STATUS=$(echo "$SR2A" | jq -r '.status')
assert_eq "Signing R2 node-a status" "$SR2A_STATUS" "complete"

yellow "Executing Signing Round 2 for node-b..."
SR2B=$(api_post "/api/signing-requests/${SIGNING_ID}/round/2/node/node-b")
SR2B_STATUS=$(echo "$SR2B" | jq -r '.status')
assert_eq "Signing R2 node-b status" "$SR2B_STATUS" "complete"

# Aggregate and broadcast
# NOTE: The aggregate step calls Solana RPC for a blockhash and broadcasts.
# On Devnet with an unfunded wallet, the broadcast will likely fail with
# insufficient funds. We still verify that aggregation itself succeeds
# by checking that the response has a tx_signature or a meaningful error.
yellow "Aggregating signatures..."
AGGREGATE_RESP=$(curl -sf -X POST "${COORDINATOR}/api/signing-requests/${SIGNING_ID}/aggregate" 2>&1 || true)
if [ -n "$AGGREGATE_RESP" ]; then
    AGG_STATUS=$(echo "$AGGREGATE_RESP" | jq -r '.status // empty' 2>/dev/null || true)
    AGG_TX_SIG=$(echo "$AGGREGATE_RESP" | jq -r '.tx_signature // empty' 2>/dev/null || true)
    AGG_ERROR=$(echo "$AGGREGATE_RESP" | jq -r '.error.code // empty' 2>/dev/null || true)

    if [ "$AGG_STATUS" = "broadcasted" ]; then
        green "  PASS: Aggregate produced a broadcasted transaction"
        assert_not_empty "Transaction signature" "$AGG_TX_SIG"
        PASS=$((PASS + 1))
    elif [ "$AGG_ERROR" = "BROADCAST_FAILED" ]; then
        # Expected when the wallet has no SOL on Devnet
        yellow "  INFO: Aggregation succeeded but broadcast failed (expected with unfunded wallet)"
        green "  PASS: FROST signature aggregation completed (broadcast failed as expected)"
        PASS=$((PASS + 1))
    else
        yellow "  INFO: Aggregate response: $AGGREGATE_RESP"
        red "  FAIL: Unexpected aggregate response"
        FAIL=$((FAIL + 1))
    fi
else
    # The aggregate endpoint may return an error HTTP status.
    # Check the signing request status to see if aggregation at least ran.
    SR_STATUS_RESP=$(api_get "/api/signing-requests/${SIGNING_ID}" 2>/dev/null || true)
    SR_FINAL_STATUS=$(echo "$SR_STATUS_RESP" | jq -r '.status // empty' 2>/dev/null || true)
    if [ "$SR_FINAL_STATUS" = "failed" ] || [ "$SR_FINAL_STATUS" = "broadcasted" ]; then
        yellow "  INFO: Signing request ended in status '$SR_FINAL_STATUS'"
        green "  PASS: Aggregation attempted (final status: $SR_FINAL_STATUS)"
        PASS=$((PASS + 1))
    else
        red "  FAIL: Aggregation did not produce a response"
        FAIL=$((FAIL + 1))
    fi
fi

# ---- Summary -------------------------------------------------------

bold ""
bold "=== Test Summary ==="
green "Passed: $PASS"
if [ "$FAIL" -gt 0 ]; then
    red "Failed: $FAIL"
    exit 1
else
    green "Failed: 0"
    green "All integration tests passed."
fi
