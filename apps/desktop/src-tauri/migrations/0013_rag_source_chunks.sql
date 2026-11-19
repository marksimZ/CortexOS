CREATE TABLE rag_chunks (
 workspace_id TEXT NOT NULL, node_id TEXT NOT NULL, version INTEGER NOT NULL,
 id TEXT NOT NULL, body TEXT NOT NULL, location_json TEXT NOT NULL,
 PRIMARY KEY(workspace_id,node_id,version,id),
 FOREIGN KEY(workspace_id,node_id) REFERENCES knowledge_nodes(workspace_id,id),
 FOREIGN KEY(node_id,version) REFERENCES knowledge_versions(node_id,version)
);
DROP TABLE knowledge_fts;
CREATE VIRTUAL TABLE knowledge_fts USING fts5(
 node_id UNINDEXED, workspace_id UNINDEXED, version UNINDEXED, title_terms, body_terms,
 chunk_id UNINDEXED, tokenize='unicode61'
);

ALTER TABLE rag_vectors RENAME TO rag_vectors_v12;
CREATE TABLE rag_vectors (
 workspace_id TEXT NOT NULL, index_id TEXT NOT NULL, node_id TEXT NOT NULL, version INTEGER NOT NULL,
 content_hash TEXT NOT NULL, vector BLOB NOT NULL CHECK(length(vector) BETWEEN 4 AND 16384 AND length(vector)%4=0),
 chunk_id TEXT NOT NULL DEFAULT '',
 PRIMARY KEY(workspace_id,index_id,node_id,version,chunk_id),
 FOREIGN KEY(workspace_id,index_id) REFERENCES rag_index_profiles(workspace_id,id) ON DELETE CASCADE,
 FOREIGN KEY(workspace_id,node_id) REFERENCES knowledge_nodes(workspace_id,id),
 FOREIGN KEY(node_id,version) REFERENCES knowledge_versions(node_id,version)
);
INSERT INTO rag_vectors(workspace_id,index_id,node_id,version,content_hash,vector)
SELECT workspace_id,index_id,node_id,version,content_hash,vector FROM rag_vectors_v12;
DROP TABLE rag_vectors_v12;
