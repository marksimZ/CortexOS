CREATE TABLE assistant_versions (
    workspace_id TEXT NOT NULL REFERENCES workspaces(id) ON DELETE CASCADE,
    version INTEGER NOT NULL CHECK (version > 0),
    name TEXT NOT NULL,
    instructions TEXT NOT NULL,
    PRIMARY KEY (workspace_id, version)
);

CREATE TABLE assistant_drafts (
    workspace_id TEXT PRIMARY KEY NOT NULL REFERENCES workspaces(id) ON DELETE CASCADE,
    name TEXT NOT NULL,
    instructions TEXT NOT NULL,
    active_version INTEGER,
    FOREIGN KEY (workspace_id, active_version) REFERENCES assistant_versions(workspace_id, version)
);

CREATE TRIGGER immutable_assistant_version BEFORE UPDATE ON assistant_versions
BEGIN SELECT RAISE(ABORT, 'Published assistant versions are immutable'); END;
