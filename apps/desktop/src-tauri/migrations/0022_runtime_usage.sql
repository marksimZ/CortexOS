CREATE TABLE model_runtime_policies (
  workspace_id TEXT PRIMARY KEY REFERENCES workspaces(id),
  policy_json TEXT NOT NULL CHECK(json_valid(policy_json))
);

CREATE TABLE model_cloud_calls (
  id TEXT PRIMARY KEY,
  workspace_id TEXT NOT NULL REFERENCES workspaces(id),
  operation_id TEXT NOT NULL,
  purpose TEXT NOT NULL,
  connection_id TEXT NOT NULL,
  provider TEXT NOT NULL,
  model TEXT NOT NULL,
  capability TEXT NOT NULL,
  data_types_json TEXT NOT NULL CHECK(json_valid(data_types_json)),
  started_unix INTEGER NOT NULL,
  state TEXT NOT NULL,
  estimate_micros INTEGER,
  settled_micros INTEGER,
  input_tokens INTEGER,
  output_tokens INTEGER,
  vector_count INTEGER,
  elapsed_ms INTEGER,
  failure TEXT
);

CREATE INDEX model_cloud_calls_time ON model_cloud_calls(workspace_id,started_unix);
CREATE INDEX model_cloud_calls_connection ON model_cloud_calls(workspace_id,connection_id,started_unix);
CREATE INDEX model_cloud_calls_operation ON model_cloud_calls(workspace_id,operation_id);
