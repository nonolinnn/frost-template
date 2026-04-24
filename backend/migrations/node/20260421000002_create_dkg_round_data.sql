-- Node DB: Intermediate DKG round secrets
-- These are the node's private round outputs needed for subsequent rounds.
-- Round 1 secret is needed for Round 2; Round 2 secret is needed for Round 3.

CREATE TABLE dkg_round_data (
    id              UUID        PRIMARY KEY,
    session_id      UUID        NOT NULL,
    round           SMALLINT    NOT NULL,
    secret_package  JSONB       NOT NULL,
    created_at      TIMESTAMPTZ NOT NULL DEFAULT now(),

    CONSTRAINT uq_dkg_round_data_session_round
        UNIQUE (session_id, round)
);
