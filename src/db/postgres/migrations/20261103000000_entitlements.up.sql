-- Entitlements keyed by numeric GitHub account id, written only by the
-- marketplace_purchase handler.
CREATE TABLE entitlements (
      github_account_id BIGINT PRIMARY KEY,
      plan              TEXT        NOT NULL,
      status            TEXT        NOT NULL,
      effective_until   TIMESTAMPTZ,
      received_at       TIMESTAMPTZ NOT NULL DEFAULT NOW(),
      created_at        TIMESTAMPTZ NOT NULL DEFAULT NOW(),
      updated_at        TIMESTAMPTZ NOT NULL DEFAULT NOW(),
      created_by        TEXT        NOT NULL DEFAULT 'system',
      updated_by        TEXT        NOT NULL DEFAULT 'system'
);

CREATE INDEX entitlements_active_idx
    ON entitlements (github_account_id)
    WHERE status = 'active';