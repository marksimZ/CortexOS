use super::*;

fn index_count(db: &Database, conversation: &str) -> i64 {
    db.connection()
        .unwrap()
        .query_row(
            "SELECT count(*) FROM chat_messages_fts WHERE conversation_id=?1",
            [conversation],
            |row| row.get(0),
        )
        .unwrap()
}

#[test]
fn chat_search_finds_chinese_and_english_hits_across_conversations_and_archived() {
    let db = Database::rag_evaluation_database().unwrap();
    let workspace = db.create_workspace("Search fixture").unwrap().id;
    let active = db.create_conversation(&workspace).unwrap().id;
    let archived = db.create_conversation(&workspace).unwrap().id;
    db.rename_conversation(&workspace, &archived, "Pricing archive")
        .unwrap();
    let settings = ModelSettings::default();
    let active_run = db
        .prepare_run(
            &new_id(),
            &workspace,
            &active,
            "请看定价方案\n  Roadmap",
            &settings,
        )
        .unwrap()
        .run;
    let user = db.messages(&workspace, &active).unwrap().remove(0);
    let purpose = ExecutionPurpose::Chat;
    db.execution_checkpoint(&active_run, purpose, "checkpointneedle", None)
        .unwrap();
    assert_eq!(
        db.search_chat_messages(&workspace, "checkpointneedle", None)
            .unwrap()
            .len(),
        1
    );
    db.execution_checkpoint(&active_run, purpose, "New draft", None)
        .unwrap();
    assert!(
        db.search_chat_messages(&workspace, "checkpointneedle", None)
            .unwrap()
            .is_empty()
    );
    db.execution_finish(
        &active_run,
        purpose,
        "",
        Some("thinkingonlyneedle"),
        "completed",
        None,
        json!({}),
    )
    .unwrap();
    let archived_run = db
        .prepare_run(
            &new_id(),
            &workspace,
            &archived,
            "Previous question",
            &settings,
        )
        .unwrap()
        .run;
    let content = format!(
        "{}定价方案\n\tROADMAP § {}",
        "前".repeat(140),
        "后".repeat(140)
    );
    db.execution_finish(
        &archived_run,
        purpose,
        &content,
        None,
        "completed",
        None,
        json!({}),
    )
    .unwrap();
    db.archive_conversation(&workspace, &archived, true)
        .unwrap();

    assert_eq!(index_count(&db, &active), 1);
    assert_eq!(index_count(&db, &archived), 2);
    for query in ["定价方案", "\"roadmap\"!", "价"] {
        let hits = db.search_chat_messages(&workspace, query, None).unwrap();
        assert_eq!(hits.len(), 2, "{query}");
        let hit = hits
            .iter()
            .find(|hit| hit.conversation_id == active)
            .unwrap();
        assert_eq!(hit.message_id, user.id);
        assert_eq!(hit.role, "user");
        assert_eq!(hit.created_at, user.created_at);
        assert_eq!(hit.snippet, "请看定价方案 Roadmap");
        assert!(!hit.archived);
        let hit = hits
            .iter()
            .find(|hit| hit.conversation_id == archived)
            .unwrap();
        assert_eq!(hit.message_id, archived_run.assistant_message_id);
        assert_eq!(hit.conversation_title, "Pricing archive");
        assert_eq!(hit.role, "assistant");
        assert!(hit.archived);
        assert!(hit.snippet.starts_with('…') && hit.snippet.ends_with('…'));
        assert!(hit.snippet.contains("定价方案 ROADMAP §"));
        assert!(hit.snippet.chars().count() <= 122);
    }
    let symbol = db.search_chat_messages(&workspace, "§", None).unwrap();
    assert_eq!(symbol.len(), 1);
    assert_eq!(symbol[0].message_id, archived_run.assistant_message_id);
    assert!(
        db.search_chat_messages(&workspace, "thinkingonlyneedle", None)
            .unwrap()
            .is_empty()
    );
    assert!(
        db.search_chat_messages(&workspace, " \n ", None)
            .unwrap()
            .is_empty()
    );
    assert_eq!(
        db.search_chat_messages(&workspace, "roadmap", Some(0))
            .unwrap()
            .len(),
        1
    );
    assert!(db.search_chat_messages("missing", "roadmap", None).is_err());
}

#[test]
fn chat_search_isolates_workspaces_backfills_legacy_rows_and_cleans_deleted_conversations() {
    let db = Database::rag_evaluation_database().unwrap();
    let workspace = db.create_workspace("First workspace").unwrap().id;
    let other_workspace = db.create_workspace("Other workspace").unwrap().id;
    let legacy = db.create_conversation(&workspace).unwrap().id;
    let other = db.create_conversation(&other_workspace).unwrap().id;
    let settings = ModelSettings::default();
    let other_run = db
        .prepare_run(
            &new_id(),
            &other_workspace,
            &other,
            "sharedneedle",
            &settings,
        )
        .unwrap()
        .run;
    db.finish(&other_run, "", "cancelled", None, json!({}))
        .unwrap();
    let legacy_message = new_id();
    let benchmark = new_id();
    {
        let mut c = db.connection().unwrap();
        let tx = c.transaction().unwrap();
        tx.execute(
            "INSERT INTO conversations(id,workspace_id,title,created_at,updated_at,purpose)
             VALUES(?1,?2,'Benchmark','t','t','benchmark')",
            params![benchmark, workspace],
        )
        .unwrap();
        for (conversation, user_id) in [(&legacy, legacy_message.clone()), (&benchmark, new_id())] {
            let run = RunSummary {
                id: new_id(),
                workspace_id: workspace.clone(),
                conversation_id: conversation.clone(),
                assistant_message_id: new_id(),
                model_id: settings.model.clone(),
                status: "running".into(),
                started_at: now(),
                finished_at: None,
                error_message: None,
            };
            insert_execution(
                &tx,
                &run,
                &settings,
                &json!({}),
                &json!({}),
                &user_id,
                "sharedneedle",
            )
            .unwrap();
            event(&tx, &run.id, "fixture", json!({"text": "eventonlyneedle"})).unwrap();
            tx.execute(
                "UPDATE execution_runs SET status='completed' WHERE id=?1",
                [&run.id],
            )
            .unwrap();
        }
        tx.commit().unwrap();
    }
    assert_eq!(index_count(&db, &legacy), 0);
    for _ in 0..2 {
        let hits = db
            .search_chat_messages(&workspace, "sharedneedle", None)
            .unwrap();
        assert_eq!(hits.len(), 1);
        assert_eq!(hits[0].conversation_id, legacy);
        assert_eq!(hits[0].message_id, legacy_message);
        assert_eq!(index_count(&db, &legacy), 1);
    }
    assert_eq!(index_count(&db, &benchmark), 0);
    assert!(
        db.search_chat_messages(&workspace, "eventonlyneedle", None)
            .unwrap()
            .is_empty()
    );
    let substring_hits = db
        .search_chat_messages(&workspace, "sharedneed", None)
        .unwrap();
    assert_eq!(substring_hits.len(), 1);
    assert_eq!(substring_hits[0].message_id, legacy_message);
    let other_hits = db
        .search_chat_messages(&other_workspace, "sharedneedle", None)
        .unwrap();
    assert_eq!(other_hits.len(), 1);
    assert_eq!(other_hits[0].conversation_id, other);

    db.delete_conversation(&workspace, &legacy).unwrap();
    assert_eq!(index_count(&db, &legacy), 0);
    assert!(
        db.search_chat_messages(&workspace, "sharedneedle", None)
            .unwrap()
            .is_empty()
    );
    assert_eq!(index_count(&db, &legacy), 0);
    assert_eq!(index_count(&db, &other), 1);
}
