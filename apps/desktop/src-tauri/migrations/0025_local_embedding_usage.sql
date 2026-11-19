CREATE TABLE model_local_embeddings (
  id TEXT PRIMARY KEY,
  workspace_id TEXT NOT NULL REFERENCES workspaces(id),
  operation_id TEXT NOT NULL,
  purpose TEXT NOT NULL,
  model TEXT NOT NULL,
  started_unix INTEGER NOT NULL,
  state TEXT NOT NULL,
  input_tokens INTEGER,
  vector_count INTEGER,
  elapsed_ms INTEGER
);
CREATE INDEX model_local_embeddings_time ON model_local_embeddings(workspace_id,started_unix);
