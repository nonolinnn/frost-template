-- Coordinator DB: Create enum types for status tracking

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
