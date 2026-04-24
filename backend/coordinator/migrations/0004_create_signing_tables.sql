-- Coordinator DB: Signing requests and round state

CREATE TABLE signing_requests (
    id               UUID                    PRIMARY KEY,
    wallet_index     INTEGER                 NOT NULL REFERENCES wallets (index),
    recipient        TEXT                    NOT NULL,
    amount_lamports  BIGINT                  NOT NULL,
    status           signing_request_status  NOT NULL DEFAULT 'pending',
    tx_message       BYTEA,
    tx_signature     TEXT,
    error_message    TEXT,
    created_at       TIMESTAMPTZ             NOT NULL DEFAULT now(),
    updated_at       TIMESTAMPTZ             NOT NULL DEFAULT now()
);

CREATE INDEX idx_signing_requests_status ON signing_requests (status);
CREATE INDEX idx_signing_requests_created ON signing_requests (created_at DESC);

CREATE TABLE signing_round_state (
    id                  UUID         PRIMARY KEY,
    signing_request_id  UUID         NOT NULL REFERENCES signing_requests (id) ON DELETE CASCADE,
    node_id             TEXT         NOT NULL,
    round               SMALLINT     NOT NULL,
    status              round_status NOT NULL DEFAULT 'pending',
    output_data         JSONB,
    created_at          TIMESTAMPTZ  NOT NULL DEFAULT now(),
    updated_at          TIMESTAMPTZ  NOT NULL DEFAULT now(),

    CONSTRAINT uq_signing_round_state_request_node_round
        UNIQUE (signing_request_id, node_id, round)
);

CREATE TRIGGER trg_signing_requests_updated_at
    BEFORE UPDATE ON signing_requests
    FOR EACH ROW
    EXECUTE FUNCTION update_updated_at_column();

CREATE TRIGGER trg_signing_round_state_updated_at
    BEFORE UPDATE ON signing_round_state
    FOR EACH ROW
    EXECUTE FUNCTION update_updated_at_column();
