CREATE TABLE model_cloud_connections (
  workspace_id TEXT NOT NULL REFERENCES workspaces(id) ON DELETE RESTRICT,
  id TEXT NOT NULL,
  configuration_json TEXT NOT NULL CHECK(json_valid(configuration_json)),
  PRIMARY KEY(workspace_id,id)
);
