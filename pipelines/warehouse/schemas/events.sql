-- DDL for Audit Ledger Contract Events Warehouse Tables (#523)

CREATE TABLE IF NOT EXISTS raw_contract_events (
    index BIGINT NOT NULL,
    timestamp TIMESTAMP WITH TIME ZONE NOT NULL,
    event_type VARCHAR(64) NOT NULL,
    category VARCHAR(64),
    sub_event_type VARCHAR(64),
    submitter VARCHAR(64) NOT NULL,
    metadata JSONB,
    event_hash VARCHAR(66) NOT NULL PRIMARY KEY,
    prev_hash VARCHAR(66),
    parent_event_id VARCHAR(66),
    ledger_seq BIGINT,
    ingested_at TIMESTAMP WITH TIME ZONE DEFAULT CURRENT_TIMESTAMP
);

CREATE TABLE IF NOT EXISTS daily_event_aggregates (
    aggregation_date DATE NOT NULL,
    category VARCHAR(64) NOT NULL,
    total_events BIGINT NOT NULL,
    unique_submitters BIGINT NOT NULL,
    high_risk_count BIGINT NOT NULL,
    created_at TIMESTAMP WITH TIME ZONE DEFAULT CURRENT_TIMESTAMP,
    PRIMARY KEY (aggregation_date, category)
);

CREATE TABLE IF NOT EXISTS ml_feature_store_records (
    entity_id VARCHAR(64) NOT NULL,
    feature_timestamp TIMESTAMP WITH TIME ZONE NOT NULL,
    feature_name VARCHAR(64) NOT NULL,
    feature_value DOUBLE PRECISION NOT NULL,
    version VARCHAR(16) NOT NULL,
    PRIMARY KEY (entity_id, feature_timestamp, feature_name)
);
