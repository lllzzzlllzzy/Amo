CREATE TABLE IF NOT EXISTS cards (
    code        TEXT PRIMARY KEY,
    credits     BIGINT NOT NULL,
    total       BIGINT NOT NULL,
    created_at  BIGINT NOT NULL,
    expires_at  BIGINT
);
