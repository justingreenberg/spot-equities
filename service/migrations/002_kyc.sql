CREATE TABLE IF NOT EXISTS kyc_records (
    id              INTEGER PRIMARY KEY AUTOINCREMENT,
    wallet_address  TEXT NOT NULL UNIQUE,
    dinari_account_id TEXT,
    kyc_url         TEXT,
    status          TEXT NOT NULL DEFAULT 'not_started',
    -- not_started, pending, in_review, approved, rejected
    submitted_at    TEXT,
    approved_at     TEXT,
    rejected_reason TEXT,
    role_granted    INTEGER NOT NULL DEFAULT 0,
    role_granted_at TEXT,
    role_tx_hash    TEXT,
    created_at      TEXT NOT NULL,
    updated_at      TEXT NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_kyc_wallet ON kyc_records(wallet_address);
CREATE INDEX IF NOT EXISTS idx_kyc_status ON kyc_records(status);
