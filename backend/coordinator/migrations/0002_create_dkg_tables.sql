-- Coordinator DB: DKG sessions and round state

CREATE TABLE dkg_sessions (
    id               UUID PRIMARY KEY,
    status           dkg_status    NOT NULL DEFAULT 'initialized',
    group_public_key TEXT,
    created_at       TIMESTAMPTZ   NOT NULL DEFAULT now(),
    completed_at     TIMESTAMPTZ,
    updated_at       TIMESTAMPTZ   NOT NULL DEFAULT now()
);

CREATE INDEX idx_dkg_sessions_status ON dkg_sessions (status);

CREATE TABLE dkg_round_state (
    id             UUID PRIMARY KEY,
    session_id     UUID         NOT NULL REFERENCES dkg_sessions (id) ON DELETE CASCADE,
    node_id        TEXT         NOT NULL,
    round          SMALLINT     NOT NULL,
    status         round_status NOT NULL DEFAULT 'pending',
    output_package JSONB,
    created_at     TIMESTAMPTZ  NOT NULL DEFAULT now(),
    updated_at     TIMESTAMPTZ  NOT NULL DEFAULT now(),

    CONSTRAINT uq_dkg_round_state_session_node_round
        UNIQUE (session_id, node_id, round)
);

-- Trigger to update updated_at on dkg_sessions when status changes
CREATE OR REPLACE FUNCTION update_updated_at_column()
RETURNS TRIGGER AS $$
BEGIN
    NEW.updated_at = now();
    RETURN NEW;
END;
$$ LANGUAGE plpgsql;

CREATE TRIGGER trg_dkg_sessions_updated_at
    BEFORE UPDATE ON dkg_sessions
    FOR EACH ROW
    EXECUTE FUNCTION update_updated_at_column();

CREATE TRIGGER trg_dkg_round_state_updated_at
    BEFORE UPDATE ON dkg_round_state
    FOR EACH ROW
    EXECUTE FUNCTION update_updated_at_column();
