-- Support counting a user's subscriptions for the basic plan cap without a full scan.
CREATE INDEX IF NOT EXISTS subscriptions_created_by_idx
    ON subscriptions (created_by)
    WHERE created_by IS NOT NULL;