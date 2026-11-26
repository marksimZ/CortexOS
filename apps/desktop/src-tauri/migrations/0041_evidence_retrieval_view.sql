DROP VIEW IF EXISTS rag_current_knowledge;
CREATE VIEW rag_current_knowledge AS
SELECT n.workspace_id, n.id AS node_id, v.version
FROM knowledge_nodes n
JOIN knowledge_versions v ON v.node_id=n.id AND v.version=n.effective_version
LEFT JOIN knowledge_validity d ON d.node_id=n.id AND d.version=v.version
WHERE v.kind IN ('knowledge','context','decision','evidence')
  AND (d.valid_from IS NULL OR d.valid_from<=date('now','localtime'))
  AND (d.valid_to IS NULL OR d.valid_to>date('now','localtime'));
