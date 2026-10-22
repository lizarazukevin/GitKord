-- GitHub webhook payloads contain review_requested / review_request_removed which is
-- a post-change reviewer list. GitKord reconciles this stored baseline against
-- the incoming list to attribute exactly which reviewer changed.
ALTER TABLE pr_messages
    ADD COLUMN requested_reviewers TEXT[] NOT NULL DEFAULT '{}';