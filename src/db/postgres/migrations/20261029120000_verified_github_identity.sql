-- Verified GitHub identity records: numeric GitHub user ID + verification timestamp.
-- Legacy rows keep a login with NULL id/verified_at by droppin non-NULL requirement.
ALTER TABLE user_links
    ALTER COLUMN github_login DROP NOT NULL,
    ADD COLUMN github_user_id     BIGINT,
    ADD COLUMN github_verified_at TIMESTAMPTZ;

-- One GitHub account maps to at most one Discord user in this filtered index.
CREATE UNIQUE INDEX user_links_github_user_id_key
    ON user_links (github_user_id)
    WHERE github_user_id IS NOT NULL;