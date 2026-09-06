-- Backfill owner/project from the existing repository string.
-- Defensively guarded (only rows with a well-formed `owner/project` repository value are backfilled).
UPDATE subscriptions
SET owner = split_part(repository, '/', 1),
    project = split_part(repository, '/', 2)
WHERE (owner IS NULL OR project IS NULL)
  AND repository LIKE '%/%';

-- Enforce NOT NULL.
ALTER TABLE subscriptions
    ALTER COLUMN owner SET NOT NULL,
    ALTER COLUMN project SET NOT NULL;

-- Replace the primary key to match the new (owner, project) identity.
ALTER TABLE subscriptions DROP CONSTRAINT subscriptions_pkey;
ALTER TABLE subscriptions ADD PRIMARY KEY (owner, project, guild_id, channel_id);