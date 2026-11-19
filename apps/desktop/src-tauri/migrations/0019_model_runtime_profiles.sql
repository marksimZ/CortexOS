CREATE TABLE model_runtime_profiles (
  workspace_id TEXT NOT NULL REFERENCES workspaces(id),
  id TEXT NOT NULL,
  profile_json TEXT NOT NULL CHECK(json_valid(profile_json)),
  active INTEGER NOT NULL DEFAULT 0 CHECK(active IN (0,1)),
  PRIMARY KEY(workspace_id,id)
);

CREATE UNIQUE INDEX model_runtime_one_active ON model_runtime_profiles(workspace_id) WHERE active=1;
