CREATE TABLE IF NOT EXISTS requests (
    id              INTEGER PRIMARY KEY AUTOINCREMENT,
    request_id      INTEGER NOT NULL UNIQUE,
    request_type    TEXT NOT NULL,
    requester       TEXT NOT NULL,
    collateral_amount TEXT NOT NULL,
    synthetic_amount TEXT,
    status          TEXT NOT NULL DEFAULT 'detected',
    dinari_order_id TEXT,
    dinari_status   TEXT,
    dinari_fill_price TEXT,
    dinari_fill_shares TEXT,
    onchain_tx_hash TEXT,
    retry_count     INTEGER NOT NULL DEFAULT 0,
    last_error      TEXT,
    created_at      TEXT NOT NULL,
    updated_at      TEXT NOT NULL
);

CREATE TABLE IF NOT EXISTS block_cursor (
    id                    INTEGER PRIMARY KEY DEFAULT 1,
    last_processed_block  INTEGER NOT NULL DEFAULT 0,
    updated_at            TEXT NOT NULL
);

CREATE TABLE IF NOT EXISTS treasury_snapshots (
    id                    INTEGER PRIMARY KEY AUTOINCREMENT,
    dinari_usdc_balance   TEXT NOT NULL,
    dinari_dshares_held   TEXT NOT NULL,
    synthetic_outstanding TEXT NOT NULL,
    backing_ratio         TEXT NOT NULL,
    created_at            TEXT NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_requests_status ON requests(status);
CREATE INDEX IF NOT EXISTS idx_requests_requester ON requests(requester);
CREATE INDEX IF NOT EXISTS idx_requests_type ON requests(request_type);
