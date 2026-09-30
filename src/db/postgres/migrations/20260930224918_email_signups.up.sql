CREATE TABLE email_signups (
   id         BIGINT GENERATED ALWAYS AS IDENTITY PRIMARY KEY,
   email      TEXT        NOT NULL UNIQUE,
   subscribed BOOLEAN     NOT NULL DEFAULT TRUE,
   created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
   updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
   CHECK (email = lower(btrim(email)))
);