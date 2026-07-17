-- Migration: create contract_events table for the TipJar indexer.
-- Naming follows analytics/migrations convention (sequential prefix).

CREATE TABLE IF NOT EXISTS contract_events (
    id            BIGSERIAL    PRIMARY KEY,
    event_type    VARCHAR(50)  NOT NULL,          -- 'tip' | 'withdraw' | other topic strings
    contract_id   VARCHAR(100) NOT NULL,
    tx_hash       VARCHAR(100) NOT NULL UNIQUE,
    sender        VARCHAR(100),
    recipient     VARCHAR(100),
    amount        BIGINT,
    raw_data      JSONB        NOT NULL,
    processed_at  TIMESTAMPTZ  NOT NULL DEFAULT NOW()
);

CREATE INDEX IF NOT EXISTS idx_contract_events_type      ON contract_events(event_type);
CREATE INDEX IF NOT EXISTS idx_contract_events_recipient ON contract_events(recipient);
CREATE INDEX IF NOT EXISTS idx_contract_events_processed ON contract_events(processed_at);
