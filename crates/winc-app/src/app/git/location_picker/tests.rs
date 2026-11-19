//! `location_picker` 的测试：一个位置的分支、与上游的差异和未提交的更改数。

use super::*;

#[test]
fn place_state_reads_branch_and_counts() {
    let status = Status {
        head: Some(Head::Branch("main".into())),
        ahead: 2,
        behind: 1,
        ..Status::default()
    };
    let state = PlaceState::of(&status);
    assert_eq!(state.head.as_deref(), Some("main"));
    assert_eq!((state.ahead, state.behind, state.changes), (2, 1, 0));
}

#[test]
fn location_menu_copies_the_selected_worktree_name() {
    let mut app = App::new(
        winc_editor::Editor::default(),
        None,
        crate::bench::Bench::off(),
        winc_config::Config::default(),
    );
    let root = PathBuf::from("main-repo");
    app.git.root = Some(root.clone());
    app.git.repositories = vec![root.clone()];
    app.git.worktrees = [
        (root.clone(), "main"),
        (PathBuf::from("worktrees").join("review-checkout"), "topic"),
    ]
    .into_iter()
    .map(|(path, branch)| Worktree {
        path,
        head: Some(Head::Branch(branch.into())),
        locked: false,
        prunable: false,
    })
    .collect();

    for (i, name) in ["main-repo", "review-checkout"].into_iter().enumerate() {
        app.location_picker_refresh();
        app.overlays.picker.selected = i;
        assert!(app.location_picker_side(true));
        app.run_menu_command(MenuCommand::LocationCopyName);
        assert_eq!(app.clipboard_text().as_deref(), Some(name));
        assert_eq!(app.git.root.as_deref(), Some(root.as_path()));
    }
}
