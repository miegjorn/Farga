-- Add expires_at column to nodes for KV store TTL support.
-- NULL means no expiry. Used exclusively by NodeKind = 'KV'.
ALTER TABLE nodes ADD COLUMN expires_at TEXT NULL;
