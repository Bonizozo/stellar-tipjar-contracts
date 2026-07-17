use sqlx::{Executor, Postgres};

pub const CREATE_SCHEMA_SQL: &str = r#"
CREATE TABLE IF NOT EXISTS indexer_state (
    key TEXT PRIMARY KEY,
    value TEXT NOT NULL,
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE TABLE IF NOT EXISTS contract_events (
    id BIGSERIAL PRIMARY KEY,
    contract_id TEXT NOT NULL,
    event_id TEXT NOT NULL UNIQUE,
    cursor TEXT NOT NULL,
    ledger BIGINT NOT NULL,
    ledger_closed_at TIMESTAMPTZ,
    topic TEXT NOT NULL,
    topic_values JSONB NOT NULL,
    event_data JSONB NOT NULL,
    parsed_data JSONB,
    raw_event JSONB NOT NULL,
    tx_hash TEXT,
    successful_call BOOLEAN,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

ALTER TABLE contract_events ADD COLUMN IF NOT EXISTS raw_event JSONB;
UPDATE contract_events SET raw_event = jsonb_build_object(
    'event_data', event_data,
    'topic_values', topic_values,
    'topic', topic,
    'event_id', event_id
) WHERE raw_event IS NULL;
ALTER TABLE contract_events ALTER COLUMN raw_event SET NOT NULL;

CREATE INDEX IF NOT EXISTS idx_contract_events_ledger ON contract_events (ledger DESC);
CREATE INDEX IF NOT EXISTS idx_contract_events_topic ON contract_events (topic);
CREATE INDEX IF NOT EXISTS idx_contract_events_tx_hash ON contract_events (tx_hash);
CREATE INDEX IF NOT EXISTS idx_contract_events_created_at ON contract_events (created_at DESC);

CREATE TABLE IF NOT EXISTS event_failures (
    event_id TEXT PRIMARY KEY,
    contract_id TEXT NOT NULL,
    reason TEXT NOT NULL,
    retry_count INT NOT NULL DEFAULT 0,
    raw_event JSONB NOT NULL,
    last_error_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);
"#;

pub async fn ensure_schema<'a, E>(executor: E) -> anyhow::Result<()>
where
    E: Executor<'a, Database = Postgres>,
{
    executor.execute(CREATE_SCHEMA_SQL).await?;
    Ok(())
}
