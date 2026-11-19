CREATE TABLE rag_runtime_experiments (
  workspace_id TEXT NOT NULL REFERENCES workspaces(id) ON DELETE RESTRICT,
  id TEXT NOT NULL,
  created_at TEXT NOT NULL,
  finished_at TEXT,
  status TEXT NOT NULL CHECK(status IN ('running','completed','cancelled','interrupted','failed')),
  report_json TEXT NOT NULL CHECK(json_valid(report_json)),
  report_hash TEXT NOT NULL,
  PRIMARY KEY(workspace_id,id)
);
