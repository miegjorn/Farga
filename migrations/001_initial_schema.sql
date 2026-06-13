CREATE TABLE IF NOT EXISTS nodes (
    id          TEXT PRIMARY KEY,
    kind        TEXT NOT NULL,
    address     TEXT,
    project     TEXT,
    component   TEXT,
    title       TEXT,
    content     TEXT,
    created_at  TEXT NOT NULL,
    updated_at  TEXT NOT NULL,
    stale       INTEGER DEFAULT 0 NOT NULL
);

CREATE TABLE IF NOT EXISTS edges (
    from_id     TEXT NOT NULL REFERENCES nodes(id),
    to_id       TEXT NOT NULL REFERENCES nodes(id),
    kind        TEXT NOT NULL,
    weight      REAL DEFAULT 1.0 NOT NULL,
    created_at  TEXT NOT NULL,
    PRIMARY KEY (from_id, to_id, kind)
);
