DROP TABLE IF EXISTS training_samples;
DROP TABLE IF EXISTS outcomes;
DROP TABLE IF EXISTS market_snapshots;
DROP TABLE IF EXISTS bets;
DROP TABLE IF EXISTS jobs CASCADE;
DROP TABLE IF EXISTS models CASCADE;
DROP TABLE IF EXISTS batches CASCADE;

CREATE TABLE batches (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    status TEXT NOT NULL,
    job_count INT4 NOT NULL,
    aggregated_proof_path TEXT,
    tx_hash TEXT,
    gas_used INT8,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    aggregated_at TIMESTAMPTZ,
    settled_at TIMESTAMPTZ
);

CREATE TABLE models (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    name TEXT NOT NULL,
    version TEXT NOT NULL,
    ipfs_cid TEXT NOT NULL,
    input_shape JSONB NOT NULL,
    on_chain_hash TEXT NOT NULL,
    registered_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE TABLE jobs (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    model_id UUID NOT NULL REFERENCES models(id),
    status TEXT NOT NULL,
    input_hash TEXT NOT NULL,
    proof_path TEXT,
    error TEXT,
    submitted_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    started_at TIMESTAMPTZ,
    completed_at TIMESTAMPTZ,
    settled_at TIMESTAMPTZ,
    tx_hash TEXT,
    batch_id UUID REFERENCES batches(id),
    attestation_hash TEXT,
    proof_bytes BYTEA
);

CREATE TABLE bets (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    job_id UUID REFERENCES jobs(id),
    market_id TEXT NOT NULL,
    question TEXT NOT NULL,
    side TEXT NOT NULL,
    size_usdc FLOAT8 NOT NULL,
    price FLOAT8 NOT NULL,
    paper BOOLEAN NOT NULL,
    confidence FLOAT8 NOT NULL,
    yes_price FLOAT8 NOT NULL,
    no_price FLOAT8 NOT NULL,
    volume_24h FLOAT8 NOT NULL,
    attestation_hash TEXT,
    tx_hash TEXT,
    outcome BOOLEAN,
    pnl_usdc FLOAT8,
    placed_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    resolved_at TIMESTAMPTZ
);

CREATE TABLE market_snapshots (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    market_id TEXT NOT NULL,
    question TEXT NOT NULL,
    yes_price FLOAT8 NOT NULL,
    no_price FLOAT8 NOT NULL,
    volume_24h FLOAT8 NOT NULL,
    end_date TIMESTAMPTZ NOT NULL,
    captured_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE TABLE outcomes (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    market_id TEXT NOT NULL,
    question TEXT NOT NULL,
    resolved_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    outcome BOOLEAN NOT NULL
);

CREATE TABLE training_samples (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    snapshot_id UUID NOT NULL REFERENCES market_snapshots(id),
    outcome_id UUID NOT NULL REFERENCES outcomes(id),
    market_id TEXT NOT NULL,
    yes_price FLOAT8 NOT NULL,
    no_price FLOAT8 NOT NULL,
    volume_24h FLOAT8 NOT NULL,
    time_to_expiry FLOAT8 NOT NULL,
    outcome BOOLEAN NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);
