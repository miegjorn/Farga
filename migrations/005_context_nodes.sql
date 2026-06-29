-- Role-scoped context graph: 0=component, 1=architect, 2=org, 3=human.
-- Existing nodes default to 0 (all agents can read).
ALTER TABLE nodes ADD COLUMN read_role_level INTEGER NOT NULL DEFAULT 0;
