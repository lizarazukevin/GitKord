ALTER TABLE pr_messages
    ADD COLUMN owner TEXT,
    ADD COLUMN project TEXT;

UPDATE pr_messages
    SET owner = split_part(repository, '/', 1),
        project = split_part(repository, '/', 2)
    WHERE (owner IS NULL OR project IS NULL)
      AND repository LIKE '%/%';

ALTER TABLE pr_messages
    ALTER COLUMN owner SET NOT NULL,
    ALTER COLUMN project SET NOT NULL;

ALTER TABLE pr_messages DROP CONSTRAINT pr_messages_pkey;
ALTER TABLE pr_messages ADD PRIMARY KEY (owner, project, pr, channel_id);