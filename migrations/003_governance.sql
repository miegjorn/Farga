CREATE TABLE IF NOT EXISTS governance_assessments (
    id           TEXT PRIMARY KEY,
    node_id      TEXT NOT NULL REFERENCES nodes(id),
    status       TEXT NOT NULL DEFAULT 'pending',
    reversibility TEXT,
    impact       TEXT,
    routing      TEXT,
    notes        TEXT,
    created_at   TEXT NOT NULL,
    updated_at   TEXT NOT NULL
);
