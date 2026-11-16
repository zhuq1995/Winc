# 本地 Windows 打包

在应用仓库根目录执行。版本占位符替换为当前 `VERSION` 对应值；长构建使用后台任务，网络下载设置超时。构建与打包按用户请求执行，安装单独确认其授权范围。

发布流水线是 `.github/workflows/release-windows.yml`，产出 MSI、NSIS 安装包和便携 zip，并做代码签名与更新签名。本机生成未签名 NSIS 安装包与便携 zip；主程序未配置更新公钥时自动更新关闭。

需要的工具：`x86_64-pc-windows-msvc` 工具链；`x86_64-unknown-linux-musl` target（链接器用 `rust-lld`），`cargo xtask server` 会自己 `rustup target add` 这个目标和 `aarch64-unknown-linux-musl`；NSIS（`C:\Program Files (x86)\NSIS\makensis.exe`）；Node；PowerShell 7 的 `pwsh`（`release/windows/conpty.ps1` 用了 `-MaximumRetryCount`，Windows PowerShell 5.1 不认这个参数）。

```bash
# 1. 编译主程序、WSL 文件监视器和远端服务进程，并生成图标
cargo build --release --locked --target x86_64-pc-windows-msvc -p winc -p winc-pty
CARGO_TARGET_X86_64_UNKNOWN_LINUX_MUSL_LINKER=rust-lld \
  cargo build --release --locked --target x86_64-unknown-linux-musl -p winc-watch
cargo xtask server
cargo xtask icons target/x86_64-pc-windows-msvc/release/bundle/icons

# 2. 取微软的 conpty.dll 与 OpenConsole.exe（NuGet 固定版本，校验 SHA-256）
pwsh -NoProfile -File release/windows/conpty.ps1 -Destination target/x86_64-pc-windows-msvc/release

# 3. build-config.json：package.ps1 从 $env:RUNNER_TEMP/build-config.json 读 version 与 msiVersion
#    本机没有 WINC_UPDATE_PRIVATE_KEY，release/common/config.mjs 会拒绝生成，手写这两项即可
export RUNNER_TEMP="$(cygpath -w "$PWD/target/bundle-run")"
mkdir -p target/bundle-run
printf '{"version":"<VERSION>","msiVersion":"<MSI_VERSION>"}\n' > target/bundle-run/build-config.json

# 4. 摆放安装包目录（-Phase portable 同时产出便携 zip）
pwsh -NoProfile -File release/windows/package.ps1 -Phase portable -Target x86_64-pc-windows-msvc

# 5. 编 NSIS 安装包。package.ps1 -Phase installers 还会编 MSI，需要 WiX 3.14；只出安装包时直接调 makensis
mkdir -p target/x86_64-pc-windows-msvc/release/bundle/stage/nsis
printf 'nsis\n' > target/x86_64-pc-windows-msvc/release/bundle/stage/nsis/winc-installation
B=$(cygpath -w "$PWD/target/x86_64-pc-windows-msvc/release/bundle")
MSYS2_ARG_CONV_EXCL='*' "/c/Program Files (x86)/NSIS/makensis.exe" /V2 /INPUTCHARSET UTF8 \
  /DVERSION="<VERSION>" /DVIVERSION="<VIVERSION>" \
  "/DSOURCE=$B\\stage" "/DOUT_FILE=$B\\nsis\\Winc-<VERSION>-windows-x64-setup.exe" \
  release/windows/installer.nsi
```

- `version` 取自 `VERSION` 文件，`release/windows/package.ps1` 会与它比对，不一致直接退出。`msiVersion` 是语义版本的前三段；带预发布号时取其中的数字作第四段（`1.2.3-beta.4` → `1.2.3.4`）。`<VIVERSION>` 是 `msiVersion` 补零到四段的值（`0.1.660` → `0.1.660.0`），`makensis` 的 `VIProductVersion` 只接受四段。
- Git Bash 会把 `/V2`、`/DSOURCE=…` 当路径改写，`makensis` 因此找不到脚本，调用时必须带 `MSYS2_ARG_CONV_EXCL='*'`。
- 产物：`target/x86_64-pc-windows-msvc/release/bundle/nsis/Winc-<VERSION>-windows-x64-setup.exe`；便携包：`…/bundle/portable/Winc-<VERSION>-windows-x64-portable.zip`。
- 未签名的影响：安装包没有 Authenticode 签名，首次运行会触发 SmartScreen 提示。编译时若没有 `WINC_UPDATE_PUBLIC_KEY`（`crates/winc-app/src/updates.rs:113` 用 `option_env!` 读），安装后的程序不检查更新。
- `release/windows/verify.ps1` 会静默安装 NSIS 与 MSI 两条链并改写注册表、开始菜单和 Path，需要 MSI 在场；本机已有 Winc 在运行时不要执行。检查安装包可以用 `pwsh` 读版本资源，或把安装目录指到空目录后手工卸载。

包生成后检查版本资源、必需文件和压缩包完整性，提取包内 `winc.exe` 与 release 构建比较 SHA-256。功能与交付清单见 [F20](fork-features.md#f20-本地-windows-包)。
