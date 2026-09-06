-- Remove the legacy repository column now that owner/project are present.
ALTER TABLE subscriptions DROP COLUMN repository;