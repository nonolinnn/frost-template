-- Node DB: Signing nonces generated during Signing Round 1
-- Consumed (and must be invalidated) during Signing Round 2.
-- UNIQUE constraint prevents nonce reuse, which would compromise the private key.

CREATE TABLE signing_nonces (
    id                  UUID        PRIMARY KEY,
    signing_request_id  UUID        NOT NULL UNIQUE,
    nonces              JSONB       NOT NULL,
    created_at          TIMESTAMPTZ NOT NULL DEFAULT now()
);
