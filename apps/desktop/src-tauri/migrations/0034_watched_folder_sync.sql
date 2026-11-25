ALTER TABLE watched_folders ADD COLUMN scan_cursor TEXT;

CREATE TABLE source_managed_knowledge (
    source_id TEXT PRIMARY KEY REFERENCES source_documents(id),
    node_id TEXT NOT NULL UNIQUE,
    managed_version INTEGER NOT NULL,
    FOREIGN KEY(node_id, managed_version) REFERENCES knowledge_versions(node_id, version)
);

-- Recover ownership from the original import version and its matching snapshot.
-- An intervening manual revision disqualifies later versions from adoption.
WITH original_imports AS (
    SELECT l.source_id, n.id, n.revision, n.latest_version, n.effective_version,
           row_number() OVER (PARTITION BY l.source_id ORDER BY n.created_at, n.id) AS position
    FROM knowledge_source_links l
    JOIN knowledge_nodes n ON n.id = l.node_id
    JOIN knowledge_versions k ON k.node_id = n.id AND k.version = 1
    JOIN source_versions s ON s.source_id = l.source_id AND s.version = l.source_version
    WHERE l.knowledge_version = 1 AND l.start_line = 1
          AND n.created_at = s.created_at AND k.created_at = s.created_at
          AND k.change_reason = '导入资料自动生效' AND k.kind = 'knowledge'
          AND k.content = substr(s.content, 1, 20000) AND k.content = l.excerpt
)
INSERT INTO source_managed_knowledge(source_id, node_id, managed_version)
SELECT i.source_id, i.id,
       CASE WHEN i.revision = i.latest_version + 1
                 AND i.effective_version = i.latest_version
                 AND NOT EXISTS (
                     SELECT 1 FROM knowledge_versions newer
                     WHERE newer.node_id = i.id AND newer.change_reason <> '导入资料自动生效'
                 )
            THEN i.latest_version ELSE 1 END
FROM original_imports i
WHERE i.position = 1;
