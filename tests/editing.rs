//! Acceptance criteria 2–7, 9, and 14: reading while someone edits, one
//! editor per page, independent pages, publishing, conflicts, idle release,
//! draft restore, failed publishing, and version history.

mod common;

use std::fs;
use std::time::{Duration, Instant};

use cairn::config::AppConfig;
use cairn::drafts::{Draft, DraftStore, StagedImage};
use cairn::error::CairnError;
use cairn::fsutil::sha256_hex;
use cairn::history;
use cairn::locks::{self, Acquire};
use cairn::publish::{
    FailPoint, PublishOptions, PublishOutcome, PublishRequest, assets_dir_rel, image_link_for,
    publish,
};
use cairn::server::{AppState, EditSession, api::open_at};

const PAGE: &str = "Guides/printer.md";

fn seed(ws: &common::TestWorkspace, text: &str) -> String {
    common::write(&ws.path.join(PAGE), text);
    sha256_hex(text.as_bytes())
}

fn publish_text(
    ws: &common::TestWorkspace,
    who: &locks::Identity,
    base: Option<&str>,
    text: &str,
    staged: Vec<(String, Vec<u8>)>,
    fail_at: Option<FailPoint>,
) -> cairn::error::Result<PublishOutcome> {
    publish(
        PublishRequest {
            root: &ws.root,
            article_rel: PAGE,
            identity: who,
            base_hash: base,
            content: text,
            staged,
        },
        &PublishOptions {
            fail_at,
            ..Default::default()
        },
    )
}

fn temp_leftovers(dir: &std::path::Path) -> Vec<String> {
    fs::read_dir(dir)
        .map(|it| {
            it.flatten()
                .map(|e| e.file_name().to_string_lossy().into_owned())
                .filter(|n| n.contains("cairn-tmp"))
                .collect()
        })
        .unwrap_or_default()
}

fn make_heartbeat_ancient(ws: &common::TestWorkspace) {
    let path = locks::lock_path(&ws.root, PAGE);
    let mut info: locks::LockInfo = serde_json::from_slice(&fs::read(&path).unwrap()).unwrap();
    info.heartbeat_at = 1;
    fs::write(&path, serde_json::to_vec(&info).unwrap()).unwrap();
}

// ------------------------------------------------------------- AC 2 and 3

#[test]
fn readers_are_not_blocked_and_a_second_editor_is_refused_with_the_editors_name() {
    let ws = common::workspace();
    seed(&ws, "# Printer\n");
    let alex = common::identity("Alex");
    let sam = common::identity("Sam");

    assert!(matches!(
        locks::acquire(&ws.root, PAGE, &alex).unwrap(),
        Acquire::Acquired(_)
    ));

    // Reading takes no lock and still works.
    assert_eq!(
        fs::read_to_string(ws.root.resolve(PAGE).unwrap()).unwrap(),
        "# Printer\n"
    );

    match locks::acquire(&ws.root, PAGE, &sam).unwrap() {
        Acquire::HeldBy(view) => {
            assert_eq!(view.info.display_name, "Alex");
            assert!(!view.is_mine && !view.possibly_abandoned);
        }
        Acquire::Acquired(_) => panic!("second editor must be refused"),
    }
    let seen_by_sam = locks::status(&ws.root, PAGE, &sam).unwrap().unwrap();
    assert_eq!(seen_by_sam.info.display_name, "Alex");

    let err = publish_text(&ws, &sam, None, "x", vec![], None).unwrap_err();
    assert!(
        matches!(err, CairnError::Locked(_)),
        "Sam cannot publish either"
    );
}

#[test]
fn locks_are_case_insensitive_like_windows_paths() {
    let ws = common::workspace();
    let alex = common::identity("Alex");
    let sam = common::identity("Sam");
    locks::acquire(&ws.root, "Guides/Printer.md", &alex).unwrap();
    assert!(matches!(
        locks::acquire(&ws.root, "guides/printer.md", &sam).unwrap(),
        Acquire::HeldBy(_)
    ));
}

#[test]
fn two_people_can_edit_different_pages_at_once() {
    let ws = common::workspace();
    let alex = common::identity("Alex");
    let sam = common::identity("Sam");
    assert!(matches!(
        locks::acquire(&ws.root, "Guides/a.md", &alex).unwrap(),
        Acquire::Acquired(_)
    ));
    assert!(matches!(
        locks::acquire(&ws.root, "Guides/b.md", &sam).unwrap(),
        Acquire::Acquired(_)
    ));
}

// ------------------------------------------------------------------ AC 4

#[test]
fn published_change_is_visible_to_a_reader_and_the_lock_is_free_afterwards() {
    let ws = common::workspace();
    let base = seed(&ws, "# Printer\n\nOld steps.\n");
    let alex = common::identity("Alex");
    locks::acquire(&ws.root, PAGE, &alex).unwrap();

    let out = publish_text(
        &ws,
        &alex,
        Some(&base),
        "# Printer\n\nNew steps.\n",
        vec![],
        None,
    )
    .unwrap();
    assert!(matches!(out, PublishOutcome::Published { .. }));
    locks::release(&ws.root, PAGE, &alex).unwrap();

    // A second client re-reads the file, as a browser refresh does.
    let reader = cairn::paths::Root::new(&ws.path).unwrap();
    assert!(
        fs::read_to_string(reader.resolve(PAGE).unwrap())
            .unwrap()
            .contains("New steps.")
    );
    assert!(locks::status(&ws.root, PAGE, &alex).unwrap().is_none());
    assert!(temp_leftovers(&ws.path.join("Guides")).is_empty());
}

// ------------------------------------------------------------------ AC 5

#[test]
fn outside_modification_causes_a_conflict_instead_of_an_overwrite() {
    let ws = common::workspace();
    let base = seed(&ws, "# Printer\n");
    let alex = common::identity("Alex");
    locks::acquire(&ws.root, PAGE, &alex).unwrap();

    // Someone edits the file directly in another program.
    fs::write(ws.path.join(PAGE), "# Printer\n\nEdited in Notepad.\n").unwrap();

    match publish_text(
        &ws,
        &alex,
        Some(&base),
        "# Printer\n\nMy version.\n",
        vec![],
        None,
    )
    .unwrap()
    {
        PublishOutcome::Conflict {
            current_content,
            current_hash,
        } => {
            assert!(current_content.unwrap().contains("Notepad"));
            assert!(current_hash.is_some());
        }
        other => panic!("expected a conflict, got {other:?}"),
    }
    assert!(
        fs::read_to_string(ws.path.join(PAGE))
            .unwrap()
            .contains("Notepad"),
        "not overwritten"
    );
    assert!(
        history::list_versions(&ws.root, PAGE).unwrap().is_empty(),
        "nothing published"
    );
}

#[test]
fn a_page_created_meanwhile_by_someone_else_is_a_conflict() {
    let ws = common::workspace();
    let alex = common::identity("Alex");
    locks::acquire(&ws.root, PAGE, &alex).unwrap();
    fs::write(ws.path.join(PAGE), "someone else's page").unwrap();
    let out = publish_text(&ws, &alex, None, "# Mine\n", vec![], None).unwrap();
    assert!(matches!(out, PublishOutcome::Conflict { .. }));
}

// ------------------------------------------------------------------ AC 6

#[test]
fn idle_timeout_releases_the_lock_but_keeps_the_draft() {
    let ws = common::workspace();
    seed(&ws, "# Printer\n");
    let home = tempfile::tempdir().unwrap();
    let state = AppState::new(home.path().to_path_buf(), AppConfig::default(), None);
    open_at(&state, &ws.path).unwrap();
    let me = state.identity();
    let instance = state.workspace().unwrap().instance_id();
    let key = AppState::session_key(&instance, PAGE);

    locks::acquire(&ws.root, PAGE, &me).unwrap();
    let draft = Draft {
        article: PAGE.into(),
        instance_id: instance.clone(),
        base_hash: None,
        content: "# Printer\n\nUnpublished work.\n".into(),
        updated_at: 0,
        is_new: false,
        staged: vec![],
    };
    state.drafts().save(&draft);
    let start = Instant::now();
    let session = EditSession {
        article: PAGE.into(),
        instance_id: instance.clone(),
        lock_held: true,
        last_activity: start,
        released_reason: None,
    };
    state.edits.lock().unwrap().insert(key.clone(), session);

    // 14 minutes idle: nothing happens.
    state.sweep(start + Duration::from_secs(14 * 60), false);
    assert!(locks::status(&ws.root, PAGE, &me).unwrap().is_some());

    // 20 minutes idle (the default): lock released, draft kept.
    state.sweep(start + Duration::from_secs(20 * 60), false);
    assert!(
        locks::status(&ws.root, PAGE, &me).unwrap().is_none(),
        "lock released"
    );
    let session = state.edits.lock().unwrap().get(&key).cloned().unwrap();
    assert!(!session.lock_held);
    assert_eq!(session.released_reason.as_deref(), Some("idle"));
    assert_eq!(
        state.drafts().load(&instance, PAGE).unwrap().content,
        draft.content
    );

    // Publishing is refused until the lock is taken again; nothing was auto-published.
    let err = publish_text(&ws, &me, None, "x", vec![], None).unwrap_err();
    assert!(matches!(err, CairnError::Locked(_)));
    assert_eq!(
        fs::read_to_string(ws.path.join(PAGE)).unwrap(),
        "# Printer\n"
    );
}

#[test]
fn stale_locks_are_never_taken_over_automatically() {
    let ws = common::workspace();
    let bob = common::identity("Bob");
    let alex = common::identity("Alex");
    locks::acquire(&ws.root, PAGE, &bob).unwrap();

    // A live lock can't be released by a maintainer.
    let err = locks::maintainer_release(&ws.root, PAGE, &bob.session_id).unwrap_err();
    assert!(matches!(err, CairnError::Locked(_)));

    make_heartbeat_ancient(&ws); // as after a crash
    match locks::acquire(&ws.root, PAGE, &alex).unwrap() {
        Acquire::HeldBy(v) => assert!(v.possibly_abandoned && !v.reclaimable_by_me),
        Acquire::Acquired(_) => panic!("age alone must never hand over a lock"),
    }
    assert!(
        locks::reclaim_own_stale(&ws.root, PAGE, &alex).is_err(),
        "not Alex's lock"
    );

    // Maintainer recovery: wrong session refused; right session releases with an audit record.
    assert!(matches!(
        locks::maintainer_release(&ws.root, PAGE, "wrong"),
        Err(CairnError::Conflict(_))
    ));
    let released = locks::maintainer_release(&ws.root, PAGE, &bob.session_id).unwrap();
    assert_eq!(released.display_name, "Bob");
    let audit = ws.root.system_dir().join("locks").join("released");
    assert!(audit.read_dir().unwrap().next().is_some());
    assert!(matches!(
        locks::acquire(&ws.root, PAGE, &alex).unwrap(),
        Acquire::Acquired(_)
    ));
}

#[test]
fn a_user_can_reclaim_their_own_crashed_lock_on_the_same_computer() {
    let ws = common::workspace();
    let before_crash = common::identity("Alex");
    locks::acquire(&ws.root, PAGE, &before_crash).unwrap();
    make_heartbeat_ancient(&ws);

    let after_restart = locks::Identity {
        session_id: uuid::Uuid::new_v4().to_string(),
        ..before_crash
    };
    let reclaimed = locks::reclaim_own_stale(&ws.root, PAGE, &after_restart).unwrap();
    assert_eq!(reclaimed.session_id, after_restart.session_id);
}

// ------------------------------------------------------------------ AC 7

#[test]
fn draft_with_picture_survives_a_restart_when_persistent_drafts_are_on() {
    let dir = tempfile::tempdir().unwrap();
    let instance = uuid::Uuid::new_v4().to_string();
    let bytes = common::png(40, 20, 9);
    {
        let store = DraftStore::new(Some(dir.path().to_path_buf()));
        assert!(store.is_persistent());
        let draft = Draft {
            article: PAGE.into(),
            instance_id: instance.clone(),
            base_hash: Some("abc".into()),
            content: "# Printer\n\n![Shot](printer.assets/shot-11111111.png)\n".into(),
            updated_at: 1,
            is_new: false,
            staged: vec![],
        };
        assert!(store.save(&draft).persisted);
        let img = StagedImage {
            name: "shot-11111111.png".into(),
            target_rel: "Guides/printer.assets/shot-11111111.png".into(),
            mime: "image/png".into(),
            size: bytes.len() as u64,
        };
        assert!(
            store
                .add_image(&instance, PAGE, img, bytes.clone())
                .unwrap()
                .persisted
        );
    }
    // "Restart": a brand-new store over the same folder.
    let store = DraftStore::new(Some(dir.path().to_path_buf()));
    let restored = store.load(&instance, PAGE).expect("draft restored");
    assert!(restored.content.contains("![Shot]"));
    assert_eq!(restored.staged.len(), 1);
    assert_eq!(
        store
            .image_bytes(&instance, PAGE, "shot-11111111.png")
            .unwrap(),
        bytes
    );

    store.discard(&instance, PAGE).unwrap();
    assert!(
        DraftStore::new(Some(dir.path().to_path_buf()))
            .load(&instance, PAGE)
            .is_none()
    );
}

#[test]
fn disabled_persistent_drafts_keep_the_draft_only_in_memory() {
    let store = DraftStore::new(None);
    let instance = uuid::Uuid::new_v4().to_string();
    let draft = Draft {
        article: PAGE.into(),
        instance_id: instance.clone(),
        base_hash: None,
        content: "in memory".into(),
        updated_at: 1,
        is_new: true,
        staged: vec![],
    };
    let out = store.save(&draft);
    assert!(!out.persisted && out.error.is_none());
    assert_eq!(store.load(&instance, PAGE).unwrap().content, "in memory");
    store.clear_memory();
    assert!(
        store.load(&instance, PAGE).is_none(),
        "gone after a restart"
    );
}

// ------------------------------------------------------------------ AC 9

#[test]
fn every_failure_point_keeps_the_previous_article_and_leaves_no_dangling_picture() {
    let points = [
        FailPoint::AfterAssets,
        FailPoint::AfterHistory,
        FailPoint::BeforeReplace,
        FailPoint::AfterReplace,
    ];
    for fail_at in points {
        let ws = common::workspace();
        let original = "# Printer\n\nOriginal.\n";
        let base = seed(&ws, original);
        let alex = common::identity("Alex");
        locks::acquire(&ws.root, PAGE, &alex).unwrap();

        let target = format!("{}/shot-deadbeef.png", assets_dir_rel(PAGE));
        let content = format!(
            "# Printer\n\n![Shot]({})\n",
            image_link_for(PAGE, "shot-deadbeef.png")
        );
        let staged = vec![(target.clone(), common::png(8, 8, 1))];

        let result = publish_text(&ws, &alex, Some(&base), &content, staged, Some(fail_at));
        assert!(result.is_err(), "{fail_at:?} should fail");
        assert_eq!(
            fs::read_to_string(ws.path.join(PAGE)).unwrap(),
            original,
            "{fail_at:?}"
        );
        assert!(
            !ws.path.join(&target).exists(),
            "{fail_at:?}: new picture cleaned up"
        );
        assert!(
            temp_leftovers(&ws.path.join("Guides")).is_empty(),
            "{fail_at:?}: no temp files"
        );
        assert!(
            locks::verify_held(&ws.root, PAGE, &alex).is_ok(),
            "user can retry"
        );
    }
}

#[test]
fn an_older_picture_is_not_deleted_when_its_reference_is_removed() {
    let ws = common::workspace();
    let alex = common::identity("Alex");
    let old_pic = ws.path.join("Guides/printer.assets/old-12345678.png");
    fs::create_dir_all(old_pic.parent().unwrap()).unwrap();
    fs::write(&old_pic, common::png(4, 4, 3)).unwrap();
    let base = seed(
        &ws,
        "# Printer\n\n![Old](printer.assets/old-12345678.png)\n",
    );
    locks::acquire(&ws.root, PAGE, &alex).unwrap();
    publish_text(
        &ws,
        &alex,
        Some(&base),
        "# Printer\n\nNo picture now.\n",
        vec![],
        None,
    )
    .unwrap();
    assert!(old_pic.exists(), "earlier versions may still refer to it");
}

#[test]
fn staged_pictures_removed_from_the_text_are_never_published() {
    let ws = common::workspace();
    let alex = common::identity("Alex");
    let base = seed(&ws, "# Printer\n");
    locks::acquire(&ws.root, PAGE, &alex).unwrap();
    let target = format!("{}/unused-00000000.png", assets_dir_rel(PAGE));
    let staged = vec![(target.clone(), common::png(4, 4, 5))];
    publish_text(
        &ws,
        &alex,
        Some(&base),
        "# Printer\n\nText only.\n",
        staged,
        None,
    )
    .unwrap();
    assert!(!ws.path.join(target).exists());
}

// ----------------------------------------------------------------- AC 14

#[test]
fn previous_versions_can_be_listed_inspected_and_restored() {
    let ws = common::workspace();
    let alex = common::identity("Alex");
    let v1 = "# Printer\n\nVersion one.\n";
    let mut base = seed(&ws, v1);
    locks::acquire(&ws.root, PAGE, &alex).unwrap();
    for text in [
        "# Printer\n\nVersion two.\n",
        "# Printer\n\nVersion three.\n",
    ] {
        match publish_text(&ws, &alex, Some(&base), text, vec![], None).unwrap() {
            PublishOutcome::Published { hash, .. } => base = hash,
            other => panic!("{other:?}"),
        }
        std::thread::sleep(Duration::from_millis(5));
    }
    let versions = history::list_versions(&ws.root, PAGE).unwrap();
    assert_eq!(versions.len(), 2);
    let oldest = versions.last().unwrap();
    let bytes = history::read_version(&ws.root, PAGE, &oldest.id).unwrap();
    assert_eq!(String::from_utf8(bytes.clone()).unwrap(), v1);

    // Restoring is itself a normal, versioned publish.
    let restored = String::from_utf8(bytes).unwrap();
    publish_text(&ws, &alex, Some(&base), &restored, vec![], None).unwrap();
    assert_eq!(fs::read_to_string(ws.path.join(PAGE)).unwrap(), v1);
    assert_eq!(history::list_versions(&ws.root, PAGE).unwrap().len(), 3);
    assert!(history::read_version(&ws.root, PAGE, "../../shared-docs.json").is_err());
}

// ------------------------------------------- opening and closing the editor

mod routes {
    use super::*;
    use axum::body::Body;
    use axum::http::Request;
    use cairn::server::build_router;
    use http_body_util::BodyExt;
    use std::sync::Arc;
    use tower::ServiceExt;

    const PORT: u16 = 4325;

    fn state(ws: &common::TestWorkspace) -> (tempfile::TempDir, Arc<AppState>) {
        let home = tempfile::tempdir().unwrap();
        let state = AppState::new(home.path().to_path_buf(), AppConfig::default(), None);
        state.set_port(PORT);
        open_at(&state, &ws.path).unwrap();
        (home, state)
    }

    async fn call(st: &Arc<AppState>, method: &str, uri: &str, json: &str) -> serde_json::Value {
        let body = if method == "GET" {
            Body::empty()
        } else {
            Body::from(json.to_string())
        };
        let req = Request::builder()
            .method(method)
            .uri(uri)
            .header("host", format!("127.0.0.1:{PORT}"))
            .header("x-cairn-token", &st.token)
            .header("content-type", "application/json")
            .body(body)
            .unwrap();
        let res = build_router(st.clone()).oneshot(req).await.unwrap();
        assert!(res.status().is_success(), "{uri}: {}", res.status());
        let bytes = res.into_body().collect().await.unwrap().to_bytes();
        serde_json::from_slice(&bytes).unwrap()
    }

    async fn has_draft(st: &Arc<AppState>) -> bool {
        call(st, "GET", "/api/page?path=Guides%2Fprinter.md", "").await["has_draft"]
            .as_bool()
            .unwrap()
    }

    #[tokio::test]
    async fn opening_and_closing_the_editor_without_changes_leaves_no_unsaved_changes() {
        let ws = common::workspace();
        seed(&ws, "# Printer\n");
        let (_home, st) = state(&ws);
        let path = format!(r#"{{"path":"{PAGE}"}}"#);

        call(&st, "POST", "/api/edit/start", &path).await;
        assert!(
            !has_draft(&st).await,
            "an untouched copy is not unsaved work"
        );

        call(&st, "POST", "/api/edit/release", &path).await;
        assert!(!has_draft(&st).await);
        let instance = st.workspace().unwrap().instance_id();
        assert!(
            st.drafts().load(&instance, PAGE).is_none(),
            "the unchanged copy is removed on close"
        );
    }

    #[tokio::test]
    async fn unpublished_new_pages_and_changed_pages_are_listed_as_drafts() {
        let ws = common::workspace();
        seed(&ws, "# Printer\n");
        common::write(&ws.path.join("Guides/wifi.md"), "# Wi-Fi\n");
        let (_home, st) = state(&ws);

        // A new page, written and closed without publishing.
        let new = r##"{"path":"Guides/opening-hours.md","is_new":true,"initial_content":"# Opening hours\n"}"##;
        call(&st, "POST", "/api/edit/start", new).await;
        let text = r##"{"path":"Guides/opening-hours.md","content":"# Opening hours\n\nMonday to Friday.\n"}"##;
        call(&st, "POST", "/api/draft/save", text).await;
        call(
            &st,
            "POST",
            "/api/edit/release",
            r#"{"path":"Guides/opening-hours.md"}"#,
        )
        .await;

        // An existing page with real changes, and one opened without changes.
        let path = format!(r#"{{"path":"{PAGE}"}}"#);
        call(&st, "POST", "/api/edit/start", &path).await;
        let edit = format!(r##"{{"path":"{PAGE}","content":"# Printer\n\nNew step.\n"}}"##);
        call(&st, "POST", "/api/draft/save", &edit).await;
        call(&st, "POST", "/api/edit/release", &path).await;
        call(
            &st,
            "POST",
            "/api/edit/start",
            r#"{"path":"Guides/wifi.md"}"#,
        )
        .await;
        call(
            &st,
            "POST",
            "/api/edit/release",
            r#"{"path":"Guides/wifi.md"}"#,
        )
        .await;

        let drafts = call(&st, "GET", "/api/drafts", "").await["drafts"].clone();
        let list = drafts.as_array().unwrap();
        assert_eq!(list.len(), 2, "{drafts}");
        let new = list
            .iter()
            .find(|d| d["path"] == "Guides/opening-hours.md")
            .unwrap();
        assert_eq!(new["is_new"], true);
        assert_eq!(new["title"], "Opening hours");
        assert_eq!(new["folder"], "Guides");
        let changed = list.iter().find(|d| d["path"] == PAGE).unwrap();
        assert_eq!(changed["is_new"], false);

        // Drafts survive a restart (they are read back from disk).
        st.drafts().clear_memory();
        let again = call(&st, "GET", "/api/drafts", "").await;
        assert_eq!(again["drafts"].as_array().unwrap().len(), 2);
    }

    #[tokio::test]
    async fn real_unsaved_changes_are_kept_when_the_editor_closes() {
        let ws = common::workspace();
        seed(&ws, "# Printer\n");
        let (_home, st) = state(&ws);
        let path = format!(r#"{{"path":"{PAGE}"}}"#);

        call(&st, "POST", "/api/edit/start", &path).await;
        let edit = format!(r##"{{"path":"{PAGE}","content":"# Printer\n\nNew step.\n"}}"##);
        call(&st, "POST", "/api/draft/save", &edit).await;
        call(&st, "POST", "/api/edit/release", &path).await;

        assert!(has_draft(&st).await);
        let instance = st.workspace().unwrap().instance_id();
        assert!(
            st.drafts()
                .load(&instance, PAGE)
                .unwrap()
                .content
                .contains("New step.")
        );
    }
}

// ------------------------------------------------------- last edited by

#[test]
fn pages_and_versions_say_who_last_published_them() {
    use cairn::editors::{self, LastEdit};
    let ws = common::workspace();
    let alex = common::identity("Alex");
    let sam = common::identity("Sam");

    locks::acquire(&ws.root, PAGE, &alex).unwrap();
    publish_text(&ws, &alex, None, "# One\n", vec![], None).unwrap();
    locks::release(&ws.root, PAGE, &alex).unwrap();
    locks::acquire(&ws.root, PAGE, &sam).unwrap();
    let base = sha256_hex(b"# One\n");
    publish_text(&ws, &sam, Some(&base), "# Two\n", vec![], None).unwrap();

    let who = |bytes: &[u8]| match editors::last_edit(&ws.root, PAGE, bytes) {
        LastEdit::By(rec) => Some(rec.by),
        _ => None,
    };
    assert_eq!(who(b"# Two\n").as_deref(), Some("Sam"));
    // The kept earlier version remembers that Alex published it.
    let versions = history::list_versions(&ws.root, PAGE).unwrap();
    assert_eq!(versions.len(), 1);
    assert_eq!(versions[0].by.as_deref(), Some("Alex"));
    // Changed in Notepad: nobody is named.
    fs::write(ws.root.resolve(PAGE).unwrap(), "# Three\n").unwrap();
    assert_eq!(
        editors::last_edit(&ws.root, PAGE, b"# Three\n"),
        LastEdit::Outside
    );
}
