CREATE TABLE model_cloud_knowledge (
  workspace_id TEXT NOT NULL,
  connection_id TEXT NOT NULL,
  node_id TEXT NOT NULL,
  version INTEGER NOT NULL,
  granted_at TEXT NOT NULL,
  PRIMARY KEY(workspace_id,connection_id,node_id,version),
  FOREIGN KEY(workspace_id,connection_id) REFERENCES model_cloud_connections(workspace_id,id) ON DELETE CASCADE,
  FOREIGN KEY(workspace_id,node_id) REFERENCES knowledge_nodes(workspace_id,id),
  FOREIGN KEY(node_id,version) REFERENCES knowledge_versions(node_id,version)
);
