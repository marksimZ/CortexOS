CREATE TABLE knowledge_local_only (
  workspace_id TEXT NOT NULL REFERENCES workspaces(id) ON DELETE CASCADE,
  node_id TEXT NOT NULL REFERENCES knowledge_nodes(id) ON DELETE CASCADE,
  local_only INTEGER NOT NULL DEFAULT 0 CHECK(local_only IN (0, 1)),
  updated_at TEXT NOT NULL,
  PRIMARY KEY (workspace_id, node_id)
);
