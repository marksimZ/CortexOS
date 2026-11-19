use super::*;
use std::sync::{Arc, Barrier};

#[test]
fn rag_semantic_scan_measures_large_workspace_with_fixed_vectors() {
    let directory = std::env::temp_dir().join(format!("cortex-rag-performance-{}", new_id()));
    let db = Arc::new(Database::open(&directory).unwrap());
    let workspace = db.create_workspace("合成检索性能样本").unwrap().id;
    let body = "合成知识正文。".repeat(300);
    {
        let mut c = db.connection().unwrap();
        let tx = c.transaction().unwrap();
        let date = now();
        for i in 0..1000 {
            let node = format!("node-{i:04}");
            tx.execute("INSERT INTO knowledge_nodes(id,workspace_id,latest_version,effective_version,revision,created_at,updated_at) VALUES(?1,?2,1,1,2,?3,?3)", params![node,workspace,date]).unwrap();
            tx.execute("INSERT INTO knowledge_versions VALUES(?1,1,?2,'knowledge',?3,'user','性能样本',?4)", params![node,format!("样本 {i:04}"),body,date]).unwrap();
            for id in ["a", "b"] {
                tx.execute("INSERT INTO rag_chunks VALUES(?1,?2,1,?3,'测试片段','{}')", params![workspace,node,id]).unwrap();
            }
        }
        tx.commit().unwrap();
    }
    let docs = documents(&db.connection().unwrap(), &workspace).unwrap();
    assert_eq!(docs.len(), 2000);
    let mut query_vector = vec![0.0f32; 1536];
    query_vector[0] = 1.0;
    let mut vectors = vec![query_vector.clone(); docs.len()];
    // The first node has a higher-scoring row whose input fingerprint is invalid.
    vectors[0][0] = 0.5;
    vectors[0][1] = 0.75f32.sqrt();
    db.install_rag_index(&workspace, "fixture", &"c".repeat(64), docs, vectors).unwrap();
    {
        let c = db.connection().unwrap();
        c.execute("UPDATE rag_workspace_settings SET enabled=1 WHERE workspace_id=?1", [&workspace]).unwrap();
        c.execute("UPDATE rag_vectors SET content_hash='invalid' WHERE node_id='node-0000' AND chunk_id='b'", []).unwrap();
    }
    let mut samples = Vec::new();
    let mut lock_waits = Vec::new();
    for round in 0..11 {
        let c = db.connection().unwrap();
        let query = evaluation_query(&c, &workspace, query_vector.clone()).unwrap();
        let barrier = Arc::new(Barrier::new(2));
        let other = db.clone();
        let started = barrier.clone();
        let waiter = std::thread::spawn(move || {
            started.wait();
            let start = Instant::now();
            drop(other.connection().unwrap());
            start.elapsed().as_micros()
        });
        barrier.wait();
        let start = Instant::now();
        let report = search(&c, &workspace, "没有词语索引的固定问题", Some(&query)).unwrap();
        let elapsed = start.elapsed().as_micros();
        drop(c);
        let waited = waiter.join().unwrap();
        if round > 0 {
            samples.push(elapsed);
            lock_waits.push(waited);
        }
        let hits = report["candidates"].as_array().unwrap();
        assert_eq!(hits.len(), 20);
        for (rank, item) in hits.iter().enumerate() {
            assert_eq!(item["nodeId"], format!("node-{:04}", rank + 1));
            assert_eq!(item["chunkId"], "a");
            assert_eq!(item["content"], body);
        }
    }
    println!("fixture=1000 nodes,2000 chunks,1536 dimensions; elapsed_micros={samples:?}; separate_reader_wait_micros={lock_waits:?}");
}
