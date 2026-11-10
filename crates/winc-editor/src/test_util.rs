//! 各模块测试共用的辅助函数：在系统临时目录下为每个测试创建互不干扰的目录。

use std::path::PathBuf;
use std::process::Command;

/// 在系统临时目录下新建一个空目录（已有的先清空），返回规范化后的路径（macOS 的临时目录位于
/// `/private` 之下，打开的文件记录的是真实路径）。
///
/// 目录名带调用处的源文件、`name` 和进程号，不同模块的测试用同一个 `name` 也不会撞在一起。
///
/// # Panics
///
/// 创建目录或规范化路径失败（例如临时目录不可写）时 panic。
#[track_caller]
pub(crate) fn scratch(name: &str) -> PathBuf {
    let file = std::panic::Location::caller().file();
    let module = file
        .rsplit_once("src/")
        .map_or(file, |(_, rest)| rest)
        .trim_end_matches(".rs")
        .replace(['/', '\\'], "-");
    let dir = std::env::temp_dir().join(format!(
        "winc-editor-{module}-{name}-{}",
        std::process::id()
    ));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    winc_config::files::canonicalize(&dir).unwrap()
}

/// 给测试里启动的命令行程序（`git` 等）加上不创建控制台窗口的标志：由没有控制台的进程启动
/// 测试时，Windows 为每个控制台程序单独弹出一个窗口，跑一遍测试会弹出大量窗口。
/// `0x0800_0000` 是 `CREATE_NO_WINDOW`，与产品内启动 `git` 的方式一致。
pub(crate) fn quiet(command: Command) -> Command {
    #[cfg(windows)]
    {
        let mut command = command;
        std::os::windows::process::CommandExt::creation_flags(&mut command, 0x0800_0000);
        command
    }
    #[cfg(not(windows))]
    command
}
