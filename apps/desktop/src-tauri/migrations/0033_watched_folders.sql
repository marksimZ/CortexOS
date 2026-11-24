CREATE TABLE IF NOT EXISTS watched_folders (
    id TEXT PRIMARY KEY,
    workspace_id TEXT NOT NULL REFERENCES workspaces(id) ON DELETE CASCADE,
    path TEXT NOT NULL,
    last_synced_at TEXT,
    created_at TEXT NOT NULL,
    UNIQUE(workspace_id, path)
);

CREATE INDEX IF NOT EXISTS idx_watched_folders_workspace
    ON watched_folders(workspace_id, created_at DESC);
