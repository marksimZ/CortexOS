CREATE TABLE model_cloud_privacy (
  workspace_id TEXT PRIMARY KEY REFERENCES workspaces(id),
  config_json TEXT NOT NULL CHECK(json_valid(config_json))
);
