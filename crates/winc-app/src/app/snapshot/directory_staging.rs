//! 目录行暂存按钮：离屏鼠标命中与真实 Git 索引的跨平台测试。

use std::process::Command;

use super::*;
use crate::app::git::commit::CommitGroup;
use crate::keymap::Action;
use winit::keyboard::{Key, NamedKey};

mod save_and_commit;

fn git(root: &Path, args: &[&str]) -> String {
    let mut command = Command::new("git");
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        command.creation_flags(0x0800_0000);
    }
    let output = command.arg("-C").arg(root).args(args).output().unwrap();
    assert!(
        output.status.success(),
        "{args:?}: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    String::from_utf8(output.stdout).unwrap()
}

fn write(root: &Path, path: &str, text: &str) {
    let path = root.join(path);
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    std::fs::write(path, text).unwrap();
}

fn fixture(name: &str, committed: bool) -> (App, PathBuf) {
    let dir = sample_folder(name);
    git(&dir, &["init", "-q", "-b", "main"]);
    for (key, value) in [
        ("core.autocrlf", "false"),
        ("user.name", "test"),
        ("user.email", "test@example.com"),
        ("commit.gpgsign", "false"),
    ] {
        git(&dir, &["config", key, value]);
    }
    if committed {
        for path in [
            "pkg/a.txt",
            "pkg/nested/b.txt",
            "pkg/staged.txt",
            "pkg-extra/c.txt",
            "outside.txt",
        ] {
            write(&dir, path, "base\n");
        }
        git(&dir, &["add", "."]);
        git(&dir, &["commit", "-q", "-m", "base"]);
        for path in ["pkg/a.txt", "pkg/staged.txt", "outside.txt"] {
            write(&dir, path, "staged\n");
        }
        git(&dir, &["add", "pkg/a.txt", "pkg/staged.txt", "outside.txt"]);
        for path in ["pkg/a.txt", "pkg/nested/b.txt", "pkg-extra/c.txt"] {
            write(&dir, path, "work\n");
        }
    }
    write(&dir, "pkg/new.txt", "new\n");
    let mut app = app_with(&dir, false);
    app.git.commit.set_staging(true);
    app.git.commit.set_flat(false);
    app.settings_toggle(Setting::GitModule);
    assert!(app.git.problem.is_none(), "{:?}", app.git.problem);
    let root = app.git.root.clone().unwrap();
    app.run_action(Action::ShowCommit);
    paint(&mut app);
    (app, root)
}

fn dir_row(app: &App, group: CommitGroup) -> usize {
    dir_row_named(app, group, Path::new("pkg"))
}

fn dir_row_named(app: &App, group: CommitGroup, path: &Path) -> usize {
    app.commit_rows()
        .iter()
        .position(
            |r| matches!(r, CommitRow::Dir { group: g, dir, .. } if *g == group && dir == path),
        )
        .unwrap()
}

fn hit(app: &App, target: CommitTarget) -> Option<Rect> {
    app.hits
        .commit
        .iter()
        .find(|(_, t)| *t == target)
        .map(|(r, _)| *r)
}

fn hover_dir(app: &mut App, group: CommitGroup) -> usize {
    hover_dir_named(app, group, Path::new("pkg"))
}

fn hover_dir_named(app: &mut App, group: CommitGroup, path: &Path) -> usize {
    paint(app);
    let row = dir_row_named(app, group, path);
    let rect = app
        .hits
        .commit
        .iter()
        .filter_map(|(rect, target)| {
            matches!(target, CommitTarget::Dir(r) | CommitTarget::Sticky(r, _) if *r == row)
                .then_some(*rect)
        })
        .min_by(|a, b| a.x.total_cmp(&b.x))
        .unwrap();
    app.on_mouse_move(rect.x + 60.0, rect.y + rect.h / 2.0);
    paint(app);
    row
}

fn press_dir_button(app: &mut App, group: CommitGroup, on: bool) {
    let row = hover_dir(app, group);
    let target = CommitTarget::DirStage(row, on);
    let rect = hit(app, target).expect("目录悬停时显示暂存按钮");
    assert!(hit(app, CommitTarget::DirStage(row, !on)).is_none());
    app.on_mouse_move(rect.x + rect.w / 2.0, rect.y + rect.h / 2.0);
    paint(app);
    assert_eq!(app.pointer.hover, Hover::Commit(target));
    assert!(hit(app, target).is_some(), "鼠标移入按钮后保持命中");
    click(app, (rect.x + rect.w / 2.0, rect.y + rect.h / 2.0));
    assert!(!app.git.commit.staging_active());
}

#[test]
fn directory_buttons_stage_and_unstage_only_their_group_including_collapsed_children() {
    let (mut app, root) = fixture("directory-staging-groups", true);
    let row = hover_dir(&mut app, CommitGroup::Changes);
    app.press_commit(CommitTarget::Dir(row));
    assert!(
        app.git
            .commit
            .dir_collapsed(CommitGroup::Changes, Path::new("pkg"))
    );
    press_dir_button(&mut app, CommitGroup::Changes, true);
    assert_eq!(git(&root, &["show", ":pkg/a.txt"]), "work\n");
    assert_eq!(git(&root, &["show", ":pkg/nested/b.txt"]), "work\n");
    assert_eq!(git(&root, &["show", ":pkg/staged.txt"]), "staged\n");
    assert_eq!(git(&root, &["show", ":pkg-extra/c.txt"]), "base\n");
    assert_eq!(git(&root, &["show", ":outside.txt"]), "staged\n");
    assert!(git(&root, &["ls-files", "--", "pkg/new.txt"]).is_empty());

    // 同一路径再次修改后同时出现在两组；取消暂存保留磁盘上的新内容。
    write(&root, "pkg/a.txt", "later\n");
    app.git_refresh();
    let row = hover_dir(&mut app, CommitGroup::Staged);
    app.press_commit(CommitTarget::Dir(row));
    press_dir_button(&mut app, CommitGroup::Staged, false);
    assert_eq!(
        git(&root, &["diff", "--cached", "--name-only"]),
        "outside.txt\n"
    );
    assert_eq!(git(&root, &["show", ":pkg/a.txt"]), "base\n");
    assert_eq!(
        std::fs::read_to_string(root.join("pkg/a.txt")).unwrap(),
        "later\n"
    );
    assert_eq!(
        std::fs::read_to_string(root.join("pkg/nested/b.txt")).unwrap(),
        "work\n"
    );
    assert_eq!(
        std::fs::read_to_string(root.join("pkg/new.txt")).unwrap(),
        "new\n"
    );
}

#[test]
fn an_untracked_directory_can_be_staged_and_unstaged_before_the_first_commit() {
    let (mut app, root) = fixture("directory-staging-unborn", false);
    write(&root, "pkg/nested/second.txt", "second\n");
    app.git_refresh();
    press_dir_button(&mut app, CommitGroup::Untracked, true);
    assert_eq!(
        git(&root, &["diff", "--cached", "--name-only"]),
        "pkg/nested/second.txt\npkg/new.txt\n"
    );
    press_dir_button(&mut app, CommitGroup::Staged, false);
    assert!(git(&root, &["ls-files", "--", "pkg"]).is_empty());
    assert_eq!(
        std::fs::read_to_string(root.join("pkg/new.txt")).unwrap(),
        "new\n"
    );
}

#[test]
fn pinned_directory_buttons_remain_clickable() {
    let (mut app, root) = fixture("directory-staging-pinned", false);
    let name = "directory-name-pinned-beyond-the-sidebar";
    app.panels.sidebar.width = 200.0;
    for i in 0..40 {
        write(&root, &format!("pkg/nested/{i}.txt"), "inside\n");
        write(&root, &format!("zz/{i}.txt"), "outside\n");
    }
    std::fs::rename(root.join("pkg"), root.join(name)).unwrap();
    app.git_refresh();
    paint(&mut app);
    let row = dir_row_named(&app, CommitGroup::Untracked, Path::new(name));
    app.git.commit.scroll = (row + 2) as f32 * COMMIT_ROW;
    paint(&mut app);
    let rows = app.commit_rows();
    let pinned = app
        .commit_sticky(&rows)
        .into_iter()
        .find(|s| s.row == row)
        .unwrap();
    app.on_mouse_move(pinned.rect.x + 60.0, pinned.rect.y + pinned.rect.h / 2.0);
    assert_eq!(
        app.pointer.hover,
        Hover::Commit(CommitTarget::Sticky(row, pinned.level))
    );
    paint(&mut app);
    let target = CommitTarget::DirStage(row, true);
    let action = app.git.commit.overflow.unwrap().dir_action.unwrap();
    assert!(action.x > app.commit_parts().list.right());
    assert_eq!(hit(&app, target), Some(action));
    let point = (action.x + action.w / 2.0, action.y + action.h / 2.0);
    app.on_mouse_move(point.0, point.1);
    paint(&mut app);
    assert_eq!(app.pointer.hover, Hover::Commit(target));
    // 下一个目录把固定行推走一半，溢出绘制与按钮命中都限制在本层格子内。
    let rows = app.commit_rows();
    let next = rows
        .iter()
        .enumerate()
        .skip(row + 1)
        .find(|(_, r)| r.level() <= rows[row].level())
        .unwrap()
        .0;
    app.git.commit.scroll = (next as f32 - 1.5) * COMMIT_ROW;
    paint(&mut app);
    let list = app.commit_parts().list;
    let slot_y = list.y + pinned.level as f32 * COMMIT_ROW;
    app.on_mouse_move(list.x + 60.0, slot_y + COMMIT_ROW / 4.0);
    paint(&mut app);
    let overflow = app
        .git
        .commit
        .overflow
        .expect("部分推出的固定目录仍显示全名");
    assert_eq!(overflow.row, row);
    assert!(overflow.y < slot_y);
    assert_eq!(overflow.clip.y, slot_y);
    let action = overflow.dir_action.unwrap();
    let visible = action.intersect(overflow.clip);
    assert_eq!(hit(&app, target), Some(visible));
    assert!(visible.h > 0.0 && visible.h < action.h);
    let hidden = (action.x + action.w / 2.0, slot_y - 1.0);
    assert_ne!(app.hover_at(hidden.0, hidden.1), Hover::Commit(target));
    let point = (visible.x + visible.w / 2.0, visible.y + visible.h / 2.0);
    app.on_mouse_move(point.0, point.1);
    paint(&mut app);
    assert_eq!(app.pointer.hover, Hover::Commit(target));
    click(&mut app, point);
    assert_eq!(git(&root, &["ls-files", "--", name]).lines().count(), 41);
    assert!(git(&root, &["ls-files", "--", "zz"]).is_empty());
}

fn long_directory_button(name: &str, near_edge: bool) {
    let (mut app, root) = fixture(name, false);
    std::fs::rename(root.join("pkg"), root.join(name)).unwrap();
    app.git_refresh();
    app.panels.sidebar.width = 200.0;
    let group = CommitGroup::Untracked;
    let path = Path::new(name);
    let mut row = hover_dir_named(&mut app, group, path);
    if near_edge {
        let action = app.git.commit.overflow.unwrap().dir_action.unwrap();
        let list = app.commit_parts().list;
        // 全名在侧栏中放得下，但占用了按钮在原位时所需的空间。
        app.panels.sidebar.width += action.x + 12.0 - list.right();
        row = hover_dir_named(&mut app, group, path);
    }
    let overflow = app.git.commit.overflow.expect("名称与按钮共同决定展开范围");
    assert_eq!(overflow.row, row);
    let action = overflow.dir_action.unwrap();
    let list = app.commit_parts().list;
    let name_end = action.x - 4.0;
    if near_edge {
        assert!(name_end <= list.right() - 14.0);
        assert!(name_end > list.right() - 36.0);
    } else {
        assert!(name_end > list.right());
    }
    let target = CommitTarget::DirStage(row, true);
    assert_eq!(hit(&app, target), Some(action));
    let point = (action.x + action.w / 2.0, action.y + action.h / 2.0);
    app.on_mouse_move(point.0, point.1);
    paint(&mut app);
    assert_eq!(app.pointer.hover, Hover::Commit(target));
    assert_eq!(hit(&app, target), Some(action));
    click(&mut app, point);
    assert_eq!(git(&root, &["ls-files", "--", name]).lines().count(), 1);
    assert_eq!(
        std::fs::read_to_string(root.join(name).join("new.txt")).unwrap(),
        "new\n"
    );
}

#[test]
fn a_directory_name_that_only_overlaps_its_button_expands_with_the_button() {
    long_directory_button("directory-name-near-the-button", true);
}

#[test]
fn a_directory_button_follows_the_full_name_beyond_the_sidebar() {
    long_directory_button("directory-name-extending-beyond-the-sidebar-edge", false);
}
#[test]
fn directory_buttons_reject_stale_hits_and_hide_in_checkbox_and_history_views() {
    let (mut app, root) = fixture("directory-staging-guards", true);
    let before = git(&root, &["diff", "--cached"]);
    let row = hover_dir(&mut app, CommitGroup::Changes);
    app.git.generation += 1;
    app.press_commit(CommitTarget::DirStage(row, true));
    assert_eq!(git(&root, &["diff", "--cached"]), before);

    app.git.commit.set_staging(false);
    hover_dir(&mut app, CommitGroup::Changes);
    assert!(
        !app.hits
            .commit
            .iter()
            .any(|(_, t)| matches!(t, CommitTarget::DirStage(..)))
    );
    app.press_commit(CommitTarget::DirStage(
        dir_row(&app, CommitGroup::Changes),
        true,
    ));
    assert_eq!(git(&root, &["diff", "--cached"]), before);

    app.git.commit.set_staging(true);
    app.git.commit.browse = true;
    let row = hover_dir(&mut app, CommitGroup::Changes);
    assert!(
        !app.hits
            .commit
            .iter()
            .any(|(_, t)| matches!(t, CommitTarget::DirStage(..)))
    );
    app.press_commit(CommitTarget::DirStage(row, true));
    assert_eq!(git(&root, &["diff", "--cached"]), before);
}

fn file_target(app: &App, path: &Path, group: CommitGroup) -> CommitTarget {
    let index = app
        .git
        .status
        .as_ref()
        .unwrap()
        .entries
        .iter()
        .position(|e| e.path == path && group.holds(e, true))
        .unwrap();
    CommitTarget::FileIn(index, group)
}

fn right_click_target(app: &mut App, target: CommitTarget) -> Vec<(MenuCommand, bool)> {
    paint(app);
    let rect = hit(app, target).unwrap();
    right_click(app, (rect.x + 100.0, rect.y + rect.h / 2.0));
    menu_commands(app)
}

fn read_file(root: &Path, path: &str) -> String {
    std::fs::read_to_string(root.join(path)).unwrap()
}

#[test]
fn working_row_rollback_keeps_the_staged_version_and_commit_scope() {
    let (mut app, root) = fixture("rollback-working-row", true);
    let a = root.join("pkg/a.txt");
    let staged = file_target(&app, &a, CommitGroup::Staged);
    let working = file_target(&app, &a, CommitGroup::Changes);
    let cached = git(&root, &["ls-files", "--stage"]);
    let menu = right_click_target(&mut app, staged);
    assert!(menu.contains(&(MenuCommand::CommitCheck(false), true)));
    assert!(!menu.iter().any(|(c, _)| matches!(
        c,
        MenuCommand::CommitCheck(true) | MenuCommand::GitRollbackFile
    )));
    app.overlays.context_menu = None;
    app.commit_list_key(&Key::Named(NamedKey::Delete));
    app.run_action(Action::ChangesRollback);
    assert_eq!(read_file(&root, "pkg/a.txt"), "work\n");
    let menu = right_click_target(&mut app, working);
    assert!(menu.contains(&(MenuCommand::CommitCheck(true), true)));
    assert!(menu.contains(&(MenuCommand::GitRollbackFile, true)));
    assert!(
        !menu
            .iter()
            .any(|(c, _)| *c == MenuCommand::CommitCheck(false))
    );
    app.run_menu_command(MenuCommand::GitRollbackFile);
    assert_eq!(read_file(&root, "pkg/a.txt"), "staged\n");
    assert_eq!(read_file(&root, "pkg/nested/b.txt"), "work\n");
    assert_eq!(git(&root, &["ls-files", "--stage"]), cached);
    assert_eq!(git(&root, &["show", "HEAD:pkg/a.txt"]), "base\n");
    git(&root, &["commit", "-qm", "staged"]);
    assert_eq!(git(&root, &["show", "HEAD:pkg/a.txt"]), "staged\n");
}

#[test]
fn staging_selection_actions_use_the_selected_side_and_keep_unsaved_text() {
    let (mut app, root) = fixture("staging-mixed-group-selection", true);
    let a = root.join("pkg/a.txt");
    let b = root.join("pkg/nested/b.txt");
    let staged = file_target(&app, &a, CommitGroup::Staged);
    let b_working = file_target(&app, &b, CommitGroup::Changes);
    app.press_commit(staged);
    app.pointer.modifiers = command_modifiers();
    app.press_commit(b_working);
    app.pointer.modifiers = winit::keyboard::ModifiersState::empty();
    let menu = right_click_target(&mut app, b_working);
    assert!(menu.contains(&(MenuCommand::CommitCheck(true), true)));
    app.run_menu_command(MenuCommand::CommitCheck(true));
    assert_eq!(git(&root, &["show", ":pkg/nested/b.txt"]), "work\n");
    assert_eq!(git(&root, &["show", ":pkg/a.txt"]), "staged\n");

    app.open_path(&a);
    let doc = app.editor.active_mut().unwrap();
    let id = doc.id;
    doc.apply(winc_core::edit::replace_selection(
        doc.text().rope().unwrap(),
        winc_core::Selection::point(0),
        "unsaved\n",
        winc_core::EditKind::Other,
    ));
    paint(&mut app);
    let staged = file_target(&app, &a, CommitGroup::Staged);
    app.press_commit(staged);
    app.commit_list_key(&Key::Named(NamedKey::Space));
    assert_eq!(git(&root, &["show", ":pkg/a.txt"]), "base\n");
    assert_eq!(read_file(&root, "pkg/a.txt"), "work\n");
    let doc = app.editor.document(id).unwrap();
    assert!(doc.is_dirty());
    assert_eq!(doc.text().rope().unwrap().to_string(), "unsaved\nwork\n");
}

#[test]
fn directory_menu_and_discard_all_limit_working_tree_scope() {
    let (mut app, root) = fixture("rollback-directory-scope", true);
    let row = dir_row(&app, CommitGroup::Changes);
    app.press_commit(CommitTarget::Dir(row));
    paint(&mut app);
    // The directory is part of a larger selection. The menu still applies to its own members.
    app.select_commit_files(
        vec![
            root.join("pkg/a.txt"),
            root.join("pkg/nested/b.txt"),
            root.join("pkg-extra/c.txt"),
        ],
        Some(CommitGroup::Changes),
    );
    let staged_dir = dir_row(&app, CommitGroup::Staged);
    let menu = right_click_target(&mut app, CommitTarget::Dir(staged_dir));
    assert!(menu.contains(&(MenuCommand::CommitCheck(false), true)));
    assert!(!menu.iter().any(|(c, _)| matches!(
        c,
        MenuCommand::CommitCheck(true) | MenuCommand::GitRollbackFile
    )));
    app.overlays.context_menu = None;
    app.select_commit_files(
        vec![
            root.join("pkg/a.txt"),
            root.join("pkg/nested/b.txt"),
            root.join("pkg-extra/c.txt"),
        ],
        Some(CommitGroup::Changes),
    );
    let menu = right_click_target(&mut app, CommitTarget::Dir(row));
    assert!(menu.contains(&(MenuCommand::GitRollbackFile, true)));
    let cached = git(&root, &["ls-files", "--stage"]);
    app.run_menu_command(MenuCommand::GitRollbackFile);
    assert_eq!(read_file(&root, "pkg/a.txt"), "staged\n");
    assert_eq!(read_file(&root, "pkg/nested/b.txt"), "base\n");
    assert_eq!(read_file(&root, "pkg-extra/c.txt"), "work\n");
    assert_eq!(git(&root, &["ls-files", "--stage"]), cached);

    git(&root, &["rm", "-f", "--", "pkg/staged.txt"]);
    write(&root, "pkg/staged.txt", "recreated\n");
    app.git_refresh();
    let cached = git(&root, &["ls-files", "--stage"]);
    app.open_context_menu(MenuTarget::CommitMore, (0.0, 0.0));
    assert!(menu_commands(&app).contains(&(MenuCommand::CommitDiscardAll, true)));
    app.run_menu_command(MenuCommand::CommitDiscardAll);
    assert_eq!(git(&root, &["ls-files", "--stage"]), cached);
    assert_eq!(read_file(&root, "pkg-extra/c.txt"), "base\n");
    assert!(!root.join("pkg/staged.txt").exists());
    assert_eq!(read_file(&root, "pkg/new.txt"), "new\n");
    assert_eq!(read_file(&root, "outside.txt"), "staged\n");
    app.open_context_menu(MenuTarget::CommitMore, (0.0, 0.0));
    assert!(menu_commands(&app).contains(&(MenuCommand::CommitDiscardAll, false)));
}

#[test]
fn rollback_confirmation_rejects_a_changed_repo_busy_write_or_stale_trash_candidate() {
    let (mut app, root) = fixture("rollback-confirmation-scope", true);
    let file = root.join("pkg/new.txt");
    let request = app.rollback_request(vec![file.clone()]).unwrap();
    app.prompt_rollback(request.clone());
    assert_eq!(
        app.overlays.alert.as_ref().unwrap().detail,
        winc_i18n::t!("git-rollback-index-detail")
    );
    assert!(app.alert_key(&Key::Named(NamedKey::Escape), false));
    assert!(file.exists());

    let other_root = sample_folder("rollback-other-repo");
    app.prompt_rollback(request.clone());
    app.git.root = Some(other_root);
    assert!(app.alert_key(&Key::Named(NamedKey::Enter), false));
    assert!(app.overlays.alert.is_none());
    assert!(file.exists());
    app.git.root = Some(root.clone());

    app.prompt_rollback(request.clone());
    app.git.commit.busy = true;
    assert!(app.alert_key(&Key::Named(NamedKey::Enter), false));
    assert!(file.exists());
    app.git.commit.busy = false;

    app.open_path(&file);
    let id = app.editor.active().unwrap().id;
    app.disk.saves.running.insert(id);
    app.git_rollback(request.clone());
    assert!(file.exists());
    assert_eq!(
        app.status.message.as_deref(),
        Some(winc_i18n::t!("git-rollback-saving"))
    );
    app.disk.saves.running.remove(&id);

    git(&root, &["add", "pkg/new.txt"]);
    app.prompt_rollback(request);
    assert!(app.alert_key(&Key::Named(NamedKey::Enter), false));
    assert_eq!(read_file(&root, "pkg/new.txt"), "new\n");
    assert_eq!(git(&root, &["show", ":pkg/new.txt"]), "new\n");
}

#[test]
fn working_group_button_restores_every_tracked_member() {
    let (mut app, root) = fixture("rollback-working-group", true);
    let cached = git(&root, &["ls-files", "--stage"]);
    app.press_commit(CommitTarget::GroupRevert(CommitGroup::Changes));
    assert_eq!(read_file(&root, "pkg/a.txt"), "staged\n");
    assert_eq!(read_file(&root, "pkg/nested/b.txt"), "base\n");
    assert_eq!(read_file(&root, "pkg-extra/c.txt"), "base\n");
    assert_eq!(read_file(&root, "pkg/new.txt"), "new\n");
    assert_eq!(git(&root, &["ls-files", "--stage"]), cached);

    let target = file_target(&app, &root.join("pkg/new.txt"), CommitGroup::Untracked);
    let menu = right_click_target(&mut app, target);
    assert!(menu.contains(&(MenuCommand::GitRollbackFile, true)));
    app.run_menu_command(MenuCommand::GitRollbackFile);
    assert!(!root.join("pkg/new.txt").exists());
    assert_eq!(git(&root, &["ls-files", "--stage"]), cached);
}

#[test]
fn a_working_tree_rename_keeps_index_entries_when_rolled_back() {
    let (mut app, root) = fixture("rollback-working-rename", true);
    let source = root.join("pkg/nested/b.txt");
    let target = root.join("renamed.txt");
    write(&root, "pkg/nested/b.txt", "base\n");
    std::fs::rename(&source, &target).unwrap();
    git(&root, &["add", "-N", "renamed.txt"]);
    app.git_refresh();
    let change = app
        .git
        .status
        .as_ref()
        .unwrap()
        .entries
        .iter()
        .find(|e| e.path == target)
        .unwrap();
    assert_eq!(change.from.as_deref(), Some(source.as_path()));
    let cached = git(&root, &["ls-files", "--stage"]);
    app.confirm_rollback(vec![target.clone()]);
    assert_eq!(read_file(&root, "pkg/nested/b.txt"), "base\n");
    assert!(!target.exists());
    assert_eq!(git(&root, &["ls-files", "--stage"]), cached);
}

#[test]
fn tree_and_tab_rollbacks_keep_staged_renames_and_new_files() {
    let (mut app, root) = fixture("rollback-tree-and-tab", true);
    git(&root, &["mv", "pkg/nested/b.txt", "renamed.txt"]);
    app.editor.workspace.as_mut().unwrap().refresh();
    git(&root, &["add", "pkg/new.txt"]);
    write(&root, "pkg/new.txt", "later\n");
    app.git_refresh();
    let renamed = root.join("renamed.txt");
    let new = root.join("pkg/new.txt");
    let status = app.git.status.as_ref().unwrap();
    let rename = status.entries.iter().find(|e| e.path == renamed).unwrap();
    assert_eq!(
        rename.from.as_deref(),
        Some(root.join("pkg/nested/b.txt").as_path())
    );
    let cached = git(&root, &["ls-files", "--stage"]);

    app.open_context_menu(MenuTarget::Tree(Some(renamed)), (0.0, 0.0));
    for command in [
        MenuCommand::CommitCheck(true),
        MenuCommand::CommitCheck(false),
        MenuCommand::GitRollbackFile,
    ] {
        let menu = menu_commands(&app);
        assert!(menu.contains(&(command, true)), "{command:?}: {menu:?}");
    }
    app.run_menu_command(MenuCommand::GitRollbackFile);
    assert_eq!(read_file(&root, "renamed.txt"), "base\n");
    assert!(!root.join("pkg/nested/b.txt").exists());
    assert_eq!(read_file(&root, "pkg/new.txt"), "later\n");

    app.open_path(&new);
    let id = app.editor.active().unwrap().id;
    app.open_context_menu(MenuTarget::Tab(id), (0.0, 0.0));
    assert!(menu_commands(&app).contains(&(MenuCommand::GitRollbackFile, true)));
    app.run_menu_command(MenuCommand::GitRollbackFile);
    assert_eq!(read_file(&root, "pkg/new.txt"), "new\n");
    assert_eq!(read_file(&root, "pkg/a.txt"), "work\n");
    assert_eq!(git(&root, &["ls-files", "--stage"]), cached);
    assert_eq!(git(&root, &["show", "HEAD:pkg/nested/b.txt"]), "base\n");
}
