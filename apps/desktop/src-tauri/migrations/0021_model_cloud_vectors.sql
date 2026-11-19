CREATE TABLE model_cloud_indexes (
  workspace_id TEXT NOT NULL,
  connection_id TEXT NOT NULL,
  model TEXT NOT NULL,
  id TEXT NOT NULL,
  provider TEXT NOT NULL,
  dimensions INTEGER NOT NULL CHECK(dimensions BETWEEN 1 AND 4096),
  created_at TEXT NOT NULL,
  metadata_json TEXT NOT NULL CHECK(json_valid(metadata_json)),
  PRIMARY KEY(workspace_id,id),
  UNIQUE(workspace_id,connection_id,model),
  FOREIGN KEY(workspace_id,connection_id) REFERENCES model_cloud_connections(workspace_id,id) ON DELETE CASCADE
);

CREATE TABLE model_cloud_vectors (
  workspace_id TEXT NOT NULL,
  index_id TEXT NOT NULL,
  node_id TEXT NOT NULL,
  version INTEGER NOT NULL,
  chunk_id TEXT NOT NULL,
  content_hash TEXT NOT NULL,
  vector BLOB NOT NULL CHECK(length(vector) BETWEEN 4 AND 16384 AND length(vector)%4=0),
  PRIMARY KEY(workspace_id,index_id,node_id,version,chunk_id),
  FOREIGN KEY(workspace_id,index_id) REFERENCES model_cloud_indexes(workspace_id,id) ON DELETE CASCADE,
  FOREIGN KEY(workspace_id,node_id) REFERENCES knowledge_nodes(workspace_id,id),
  FOREIGN KEY(node_id,version) REFERENCES knowledge_versions(node_id,version)
);
