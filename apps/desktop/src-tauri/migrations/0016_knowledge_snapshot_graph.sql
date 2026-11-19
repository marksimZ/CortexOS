ALTER TABLE knowledge_snapshot_conflicts RENAME TO knowledge_snapshot_relations;

WITH RECURSIVE roots(workspace_id,snapshot_id,root_id) AS (
 SELECT s.workspace_id,s.id,s.id FROM knowledge_snapshots s
 WHERE NOT EXISTS(SELECT 1 FROM knowledge_experimental_branches b
                  WHERE b.workspace_id=s.workspace_id AND b.snapshot_id=s.id)
 UNION ALL
 SELECT b.workspace_id,b.snapshot_id,r.root_id FROM roots r
 JOIN knowledge_experimental_branches b ON b.workspace_id=r.workspace_id AND b.base_snapshot_id=r.snapshot_id
)
INSERT INTO knowledge_snapshot_relations
SELECT r.workspace_id,r.snapshot_id,v.edge_id,v.version
FROM roots r JOIN knowledge_snapshots base ON base.workspace_id=r.workspace_id AND base.id=r.root_id
JOIN knowledge_edges e ON e.workspace_id=r.workspace_id
JOIN knowledge_edge_versions v ON v.edge_id=e.id AND v.workspace_id=e.workspace_id
 AND v.version=(SELECT MAX(old.version) FROM knowledge_edge_versions old
                WHERE old.edge_id=e.id AND old.created_at<base.created_at)
JOIN knowledge_snapshot_items a ON a.workspace_id=r.workspace_id AND a.snapshot_id=r.snapshot_id AND a.node_id=v.from_node_id
JOIN knowledge_snapshot_items b ON b.workspace_id=r.workspace_id AND b.snapshot_id=r.snapshot_id AND b.node_id=v.to_node_id
WHERE v.active=1 AND v.relation<>'contradicts';
