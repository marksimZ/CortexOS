CREATE TABLE rag_index_profiles (
 workspace_id TEXT NOT NULL REFERENCES workspaces(id),
 id TEXT NOT NULL,
 provider TEXT NOT NULL CHECK(provider='ollama'), model TEXT NOT NULL, model_digest TEXT NOT NULL,
 dimensions INTEGER NOT NULL CHECK(dimensions BETWEEN 1 AND 4096),
 normalization TEXT NOT NULL CHECK(normalization='l2'), index_version TEXT NOT NULL,
 created_at TEXT NOT NULL,
 PRIMARY KEY(workspace_id,id)
);
CREATE TABLE rag_vectors (
 workspace_id TEXT NOT NULL, index_id TEXT NOT NULL, node_id TEXT NOT NULL, version INTEGER NOT NULL,
 content_hash TEXT NOT NULL, vector BLOB NOT NULL CHECK(length(vector) BETWEEN 4 AND 16384 AND length(vector)%4=0),
 PRIMARY KEY(workspace_id,index_id,node_id,version),
 FOREIGN KEY(workspace_id,index_id) REFERENCES rag_index_profiles(workspace_id,id) ON DELETE CASCADE,
 FOREIGN KEY(workspace_id,node_id) REFERENCES knowledge_nodes(workspace_id,id),
 FOREIGN KEY(node_id,version) REFERENCES knowledge_versions(node_id,version)
);
CREATE TABLE rag_workspace_settings (
 workspace_id TEXT PRIMARY KEY REFERENCES workspaces(id), index_id TEXT NOT NULL,
 enabled INTEGER NOT NULL CHECK(enabled IN (0,1)), rerank INTEGER NOT NULL CHECK(rerank IN (0,1)),
 FOREIGN KEY(workspace_id,index_id) REFERENCES rag_index_profiles(workspace_id,id)
);
