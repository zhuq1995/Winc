use super::*;
use crate::app::input::context_menu::{MenuCommand, MenuEntry};
use winc_i18n::t;

fn edit(app: &mut App, root: &Path, name: &str, prefix: &str) -> DocumentId {
    app.open_path(&root.join(name));
    let doc = app.editor.active_mut().unwrap();
    doc.apply(winc_core::edit::replace_selection(
        doc.text().rope().unwrap(),
        winc_core::Selection::point(0),
        prefix,
        winc_core::EditKind::Other,
    ));
    doc.id
}

fn answer(app: &mut App, key: NamedKey) {
    assert!(app.alert_key(&Key::Named(key), false));
}

#[test]
fn staging_waits_for_selected_buffer_edits_and_supports_a_clean_disk_file() {
    let (mut app, root) = fixture("stage-open-buffers", true);
    let outside = edit(&mut app, &root, "outside.txt", "outside buffer\n");
    let dirty = edit(&mut app, &root, "pkg/a.txt", "saved before staging\n");
    app.run_action(Action::GitStageFile);
    assert_eq!(
        git(&root, &["show", ":pkg/a.txt"]),
        "saved before staging\nwork\n"
    );
    assert_eq!(
        read_file(&root, "pkg/a.txt"),
        "saved before staging\nwork\n"
    );
    assert!(!app.editor.document(dirty).unwrap().is_dirty());
    assert!(app.editor.document(outside).unwrap().is_dirty());
    assert_eq!(git(&root, &["show", ":outside.txt"]), "staged\n");
    assert_eq!(read_file(&root, "outside.txt"), "staged\n");

    git(&root, &["add", "pkg-extra/c.txt"]);
    git(
        &root,
        &[
            "commit",
            "-qm",
            "base for clean disk",
            "--",
            "pkg-extra/c.txt",
        ],
    );
    app.git_refresh();
    let dirty = edit(&mut app, &root, "pkg-extra/c.txt", "buffer only\n");
    let file = root.join("pkg-extra/c.txt");
    assert!(
        !app.git
            .status
            .as_ref()
            .unwrap()
            .entries
            .iter()
            .any(|e| e.path == file)
    );
    assert_eq!(
        app.git_change_items(&file),
        [MenuEntry::Item(MenuCommand::CommitCheck(true), true)]
    );
    app.run_action(Action::GitStageFile);
    assert_eq!(
        git(&root, &["show", ":pkg-extra/c.txt"]),
        "buffer only\nwork\n"
    );
    assert!(!app.editor.document(dirty).unwrap().is_dirty());
}

#[test]
fn staging_waits_for_format_on_save_to_write_the_selected_file() {
    let (mut app, root) = fixture("stage-format-before-save", true);
    write(&root, "readme.md", "unformatted\n");
    app.git_refresh();
    app.format.formatter = winc_editor::format::Formatter::with_vars(std::env::vars_os().collect());
    app.config.format_on_save = Some(true);
    app.config.format_commands.insert(
        "markdown".to_owned(),
        "git --no-pager show HEAD:pkg/staged.txt".to_owned(),
    );
    let dirty = edit(&mut app, &root, "readme.md", "editor\n");
    app.save_active();
    assert!(app.format.saving_document(dirty));
    app.run_action(Action::GitStageFile);
    assert!(!app.format.saving_document(dirty));
    assert!(!app.editor.document(dirty).unwrap().is_dirty());
    assert_eq!(read_file(&root, "readme.md"), "base\n");
    assert_eq!(git(&root, &["show", ":readme.md"]), "base\n");
}

#[test]
fn a_save_conflict_during_batch_staging_keeps_the_index_and_buffer() {
    let (mut app, root) = fixture("stage-save-conflict", true);
    let dirty = edit(&mut app, &root, "pkg/a.txt", "editor buffer\n");
    write(&root, "pkg/a.txt", "a different external file\n");
    let index = git(&root, &["ls-files", "--stage"]);
    app.stage_paths(
        &[root.join("pkg/nested/b.txt"), root.join("pkg/a.txt")],
        true,
    );
    assert_eq!(git(&root, &["ls-files", "--stage"]), index);
    assert_eq!(read_file(&root, "pkg/a.txt"), "a different external file\n");
    assert_eq!(read_file(&root, "pkg/nested/b.txt"), "work\n");
    assert!(app.editor.document(dirty).unwrap().is_dirty());
    assert!(app.git.status.as_ref().unwrap().entries.iter().any(|e| {
        e.path.ends_with("pkg/nested/b.txt") && e.staged.is_none() && e.unstaged.is_some()
    }));
}

#[test]
fn empty_index_confirmation_leaves_draft_and_disk_until_confirmed_then_commits_nonignored_files() {
    let (mut app, root) = fixture("confirm-first-commit", false);
    write(&root, ".gitignore", "ignore.txt\n");
    write(&root, "ignore.txt", "ignored disk\n");
    app.git_refresh();
    let dirty = edit(&mut app, &root, "pkg/new.txt", "buffer\n");
    let ignored = edit(&mut app, &root, "ignore.txt", "ignored buffer\n");
    app.git.commit.message.set_text("first commit");
    let status = app.git.status.as_ref().unwrap();
    assert!(status.oid.is_none() && app.commit_blocker(status).is_none());
    app.git_commit();
    assert_eq!(
        app.overlays.alert.as_ref().unwrap().detail,
        t!("git-commit-all-confirm-detail")
    );
    answer(&mut app, NamedKey::Escape);
    assert_eq!(app.git.commit.message.text(), "first commit");
    assert!(app.editor.document(dirty).unwrap().is_dirty());
    assert_eq!(read_file(&root, "pkg/new.txt"), "new\n");
    assert!(git(&root, &["ls-files"]).is_empty());
    assert!(app.git.status.as_ref().unwrap().oid.is_none());

    app.git_commit();
    answer(&mut app, NamedKey::Enter);
    assert_eq!(git(&root, &["show", "HEAD:pkg/new.txt"]), "buffer\nnew\n");
    assert_eq!(git(&root, &["show", ":pkg/new.txt"]), "buffer\nnew\n");
    assert_eq!(git(&root, &["log", "-1", "--format=%s"]), "first commit\n");
    assert!(git(&root, &["ls-files", "--", "ignore.txt"]).is_empty());
    assert_eq!(read_file(&root, "ignore.txt"), "ignored disk\n");
    assert!(app.editor.document(ignored).unwrap().is_dirty());
    assert!(app.git.commit.message.text().is_empty());
}

#[test]
fn a_todo_confirmation_does_not_commit_after_switching_repositories() {
    let (mut app, root) = fixture("todo-original-repo", true);
    let (_, other) = fixture("todo-current-repo", true);
    app.state.commit_check_todo = true;
    write(&root, "pkg/a.txt", "TODO: follow up\n");
    git(&root, &["add", "--", "pkg/a.txt"]);
    app.git_refresh();
    app.git.commit.message.set_text("original draft");
    app.git_commit();
    assert_eq!(
        app.overlays.alert.as_ref().unwrap().detail,
        t!("git-commit-todo-confirm-detail")
    );

    app.git.root = Some(other.clone());
    app.git.commit.message.set_text("other draft");
    answer(&mut app, NamedKey::Enter);
    assert_eq!(git(&root, &["log", "-1", "--format=%s"]), "base\n");
    assert_eq!(git(&other, &["log", "-1", "--format=%s"]), "base\n");
    assert_eq!(git(&other, &["show", ":pkg/a.txt"]), "staged\n");
    assert_eq!(app.git.commit.message.text(), "other draft");
}

#[test]
fn an_undo_confirmation_does_not_apply_to_a_new_head() {
    let (mut app, root) = fixture("undo-stale-head", true);
    let old = app.commit_undoable().unwrap().oid.clone();
    app.press_commit(CommitTarget::Undo);
    git(&root, &["commit", "-qm", "new HEAD"]);
    app.git_refresh();
    assert_ne!(app.commit_undoable().unwrap().oid, old);
    let index = git(&root, &["ls-files", "--stage"]);
    answer(&mut app, NamedKey::Enter);
    assert_eq!(git(&root, &["log", "-1", "--format=%s"]), "new HEAD\n");
    assert_eq!(git(&root, &["ls-files", "--stage"]), index);
    assert_eq!(read_file(&root, "pkg/a.txt"), "work\n");
}

#[test]
fn empty_index_confirmation_rechecks_repository_and_real_index() {
    let (mut app, root) = fixture("confirm-recheck-index", true);
    git(&root, &["reset", "-q"]);
    app.git_refresh();
    app.git.commit.message.set_text("draft");
    app.git_commit();
    app.git.root = Some(sample_folder("confirm-different-root"));
    answer(&mut app, NamedKey::Enter);
    assert!(git(&root, &["diff", "--cached", "--name-only"]).is_empty());
    assert_eq!(git(&root, &["log", "-1", "--format=%s"]), "base\n");
    app.git.root = Some(root.clone());

    app.git_commit();
    git(&root, &["add", "pkg/a.txt"]);
    write(&root, "pkg/a.txt", "changed after terminal staging\n");
    answer(&mut app, NamedKey::Enter);
    assert_eq!(git(&root, &["show", "HEAD:pkg/a.txt"]), "work\n");
    assert_eq!(git(&root, &["show", ":pkg/a.txt"]), "work\n");
    assert_eq!(
        read_file(&root, "pkg/a.txt"),
        "changed after terminal staging\n"
    );
    assert_eq!(git(&root, &["show", "HEAD:pkg/nested/b.txt"]), "base\n");
}

#[test]
fn empty_index_commit_does_not_save_documents_in_an_unlisted_nested_repository() {
    let (mut app, root) = fixture("confirm-nested-repo", true);
    let nested = root.join("unlisted/child");
    std::fs::create_dir_all(&nested).unwrap();
    git(&nested, &["init", "-q", "-b", "nested"]);
    git(&nested, &["config", "core.autocrlf", "false"]);
    git(&nested, &["config", "commit.gpgsign", "false"]);
    write(&nested, "file.txt", "nested disk\n");
    git(&nested, &["add", "--", "file.txt"]);
    git(
        &nested,
        &[
            "-c",
            "user.name=test",
            "-c",
            "user.email=test@example.com",
            "commit",
            "-qm",
            "nested",
        ],
    );
    git(&root, &["reset", "-q"]);
    app.git_refresh();
    assert!(!app.git.repositories.contains(&nested));

    let dirty = edit(&mut app, &nested, "file.txt", "nested buffer\n");
    app.run_action(Action::GitStageFile);
    assert_eq!(read_file(&nested, "file.txt"), "nested disk\n");
    assert_eq!(app.git.root.as_ref(), Some(&root));
    app.git.commit.message.set_text("outer commit");
    app.git_commit();
    answer(&mut app, NamedKey::Enter);
    assert_eq!(git(&root, &["log", "-1", "--format=%s"]), "outer commit\n");
    assert_eq!(git(&nested, &["log", "-1", "--format=%s"]), "nested\n");
    assert_eq!(git(&nested, &["show", ":file.txt"]), "nested disk\n");
    assert_eq!(read_file(&nested, "file.txt"), "nested disk\n");
    assert!(app.editor.document(dirty).unwrap().is_dirty());
}

#[test]
fn stage_all_does_not_readd_an_untracked_recreation_of_a_staged_deletion() {
    let (mut app, root) = fixture("stage-all-staged-deletion", true);
    git(&root, &["rm", "-f", "--", "pkg/staged.txt"]);
    write(&root, "pkg/staged.txt", "recreated\n");
    app.git_refresh();
    app.run_action(Action::GitStageAll);
    assert_eq!(git(&root, &["show", ":pkg/a.txt"]), "work\n");
    assert!(git(&root, &["ls-files", "--", "pkg/staged.txt", "pkg/new.txt"]).is_empty());
    assert_eq!(read_file(&root, "pkg/staged.txt"), "recreated\n");
}

#[test]
fn a_truncated_display_still_commits_the_real_index_version() {
    let (mut app, root) = fixture("commit-staged-outside-display", true);
    let status = app.git.status.as_mut().unwrap();
    status.truncated = true;
    status.entries.retain(|entry| entry.staged.is_none());
    app.git.commit.message.set_text("commit real index");
    app.git_commit();
    assert!(app.overlays.alert.is_none());
    assert_eq!(git(&root, &["show", "HEAD:pkg/a.txt"]), "staged\n");
    assert_eq!(read_file(&root, "pkg/a.txt"), "work\n");
    assert_eq!(git(&root, &["show", "HEAD:pkg/nested/b.txt"]), "base\n");
}

#[test]
fn a_rejected_commit_leaves_its_added_content_and_draft() {
    let (mut app, root) = fixture("confirm-hook-reject", true);
    git(&root, &["reset", "-q"]);
    git(&root, &["config", "core.hooksPath", ".git/hooks"]);
    write(&root, ".git/hooks/pre-commit", "#!/bin/sh\nexit 1\n");
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(
            root.join(".git/hooks/pre-commit"),
            std::fs::Permissions::from_mode(0o755),
        )
        .unwrap();
    }
    app.git_refresh();
    let git_tool = app.git.tool.get().unwrap();
    let fingerprint = git_tool.untrusted(&root).unwrap().fingerprint;
    app.handle_event(UserEvent::GitTrust(root.clone(), fingerprint, true));
    app.git.commit.message.set_text("rejected draft");
    app.git_commit();
    answer(&mut app, NamedKey::Enter);
    assert_eq!(git(&root, &["log", "-1", "--format=%s"]), "base\n");
    assert_eq!(git(&root, &["show", ":pkg/new.txt"]), "new\n");
    assert_eq!(git(&root, &["show", ":pkg/a.txt"]), "work\n");
    assert_eq!(app.git.commit.message.text(), "rejected draft");
}

#[test]
fn confirmed_empty_index_commit_continues_with_push_and_sync() {
    for sync in [false, true] {
        let label = if sync { "sync" } else { "push" };
        let (mut app, root) = fixture(&format!("commit-then-{label}"), true);
        let shell = winc_editor::shell::Shell::with_vars(std::env::vars_os().collect());
        app.git
            .tool
            .keep(winc_editor::git::Git::find(&shell).unwrap());
        let remote_name = format!("remote-for-{label}");
        let remote = sample_folder(&remote_name);
        git(&remote, &["init", "--bare", "-q", "-b", "main"]);
        git(
            &root,
            &["remote", "add", "origin", &format!("../{remote_name}")],
        );
        git(&root, &["push", "-qu", "origin", "main"]);
        git(&root, &["reset", "-q"]);
        app.git_refresh();
        app.git.commit.message.set_text(label);
        if sync {
            app.confirm_commit_and_sync();
        } else {
            app.confirm_commit_and_push();
        }
        assert!(
            app.overlays
                .alert
                .as_ref()
                .unwrap()
                .detail
                .contains(t!("git-commit-all-confirm-detail"))
        );
        answer(&mut app, NamedKey::Enter);
        assert_eq!(
            git(&remote, &["rev-parse", "main"]),
            git(&root, &["rev-parse", "HEAD"]),
            "{label}: {:?}",
            app.notice.text
        );
        assert_eq!(git(&remote, &["show", "main:pkg/new.txt"]), "new\n");
    }
}
