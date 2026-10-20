-- Remove the legacy repository column now that owner/project are present.
ALTER TABLE pr_messages DROP COLUMN repository;