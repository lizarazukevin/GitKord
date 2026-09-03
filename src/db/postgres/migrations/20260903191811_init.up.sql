CREATE TABLE IF NOT EXISTS pr_messages (
   repository  TEXT NOT NULL,
   pr          BIGINT NOT NULL,
   channel_id  BIGINT NOT NULL,
   message_id  BIGINT NOT NULL,
   thread_id   BIGINT NOT NULL,
   PRIMARY KEY (repository, pr, channel_id)
);

CREATE TABLE IF NOT EXISTS subscriptions (
     repository        TEXT NOT NULL,
     guild_id          BIGINT NOT NULL,
     channel_id        BIGINT NOT NULL,
     installation_id   BIGINT NOT NULL,
     PRIMARY KEY (repository, guild_id, channel_id)
);

CREATE TABLE IF NOT EXISTS user_links (
      discord_id   BIGINT PRIMARY KEY,
      github_login TEXT NOT NULL UNIQUE
);