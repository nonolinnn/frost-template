-- Node DB: Root key material from completed DKG

CREATE TABLE key_shares (
    id                  UUID        PRIMARY KEY,
    session_id          UUID        NOT NULL UNIQUE,
    key_package         JSONB       NOT NULL,
    public_key_package  JSONB       NOT NULL,
    group_public_key    TEXT        NOT NULL,
    created_at          TIMESTAMPTZ NOT NULL DEFAULT now()
);
