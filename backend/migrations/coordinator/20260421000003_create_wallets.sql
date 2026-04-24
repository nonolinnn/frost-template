-- Coordinator DB: Derived HD wallets

CREATE TABLE wallets (
    index       INTEGER     PRIMARY KEY,
    address     TEXT        NOT NULL UNIQUE,
    public_key  TEXT        NOT NULL,
    chain_code  BYTEA,
    created_at  TIMESTAMPTZ NOT NULL DEFAULT now()
);
