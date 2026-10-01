# NCaptura 项目指南

## 项目速览

Rust + GTK4/Libadwaita 的 Wayland（目标 niri）截图/录屏/OCR/翻译工具。

- 截图/录屏通过子进程编排 `grim`、`slurp`、`wf-recorder`，应用自身不含 Wayland 抓帧协议代码
- OCR/翻译弹窗使用 gtk4-layer-shell 覆盖层（`src/ui/result_window.rs`），不依赖合成器窗口规则；`is_supported()` 失败时回退普通窗口
- OCR 通过独立 Python venv 运行 PaddleOCR（`src/ocr/`），JSON 协议通信
- 翻译走 `Translator` trait（`src/translate/mod.rs`），后端经 `default_translator()` 工厂分发；UI 只依赖 trait，新增后端只改工厂与新增模块
- 用户配置在 `~/.config/ncaptura/config.yaml`（`src/config.rs`），翻译相关设置只走配置文件，不新增环境变量

## 构建与测试

本机默认 rustup 工具链是 nightly（不含 cargo），所有 cargo 命令必须加 `+stable`：

```bash
cargo +stable build
cargo +stable test          # 单元测试
cargo +stable clippy --all-targets
cargo +stable fmt --check
```

集成测试（默认 ignore，需显式运行）：

```bash
NCAPTURA_OCR_TEST_IMAGE=/path/to/text.png cargo +stable test -- --ignored   # 真实 OCR 引擎 + 网络模型
cargo +stable test -- --ignored translates_via_public_instance               # 公共 mozhi 实例
```

改动 OCR/翻译链路时必须实际运行对应集成测试；GTK 弹窗冒烟需真实 Wayland 会话。

## 关键约束

- **paddlepaddle 锁定 3.2.2**：3.3.0/3.3.1 在 CPU 推理触发 oneDNN PIR 崩溃（上游已知 bug），升级前必须实机验证
- **OCR 只用系统 Python**（`/usr/bin/python3.10`–`3.13` 绝对路径探测，paddlepaddle 无 cp314 wheel）；不引入 uv/pyenv 等额外工具链；venv 用 `--clear` 保证可重建
- **OCR 相关文件必须落在 ncaptura 专属目录**（XDG 规范）：venv 与模型在 `~/.local/share/ncaptura/`，辅助脚本在 `~/.cache/ncaptura/`；模型目录通过 `PADDLE_PDX_CACHE_HOME` 控制，禁止散落 `~/.paddlex`
- **gtk4 feature 只开 `v4_8`**：`save_dialog.rs` 等旧代码用到 GTK 4.10 起废弃的 API，升级到 `v4_10` 会引入弃用警告，需连同旧代码一起迁移才可开启
- **翻译设置只进配置文件**（`translate.*`），不经环境变量；`NCAPTURA_OCR_PYTHON` 是 OCR 仅有的环境变量逃生口
- **Arch 打包必须 `options=('!lto')`**：makepkg 注入的 `-flto=auto` 会让 ring（ureq→rustls 链）的 C 对象被 rust-lld 丢弃符号导致链接失败
- 提交使用 gitmoji，不加 `Co-Authored-By`；暂存内容仅限本次任务修改

## 发版流程

1. **版本号**：更新 `Cargo.toml` 的 `version`（semver），单独提交
2. **发布前验证**：
   - `cargo +stable test` 全绿
   - `cargo +stable clippy --all-targets` 本次改动零警告
   - 运行两个 ignored 集成测试（OCR 真实图片、mozhi 公共实例）
   - `cargo +stable build --release` 成功
3. **触发发版**：GitHub Actions 页面手动运行 `Release` workflow（`workflow_dispatch`），输入版本号（不带 `v`，必须与 `Cargo.toml` 一致）。CI 校验版本并拒绝已存在的 tag → 创建并推送 tag → 单测 → 构建 `ncaptura_<version>_amd64.deb`、`ncaptura-<version>.x86_64.rpm`、`ncaptura-<version>-x86_64.tar.gz`（deb/rpm 依赖声明分别用 Debian / Fedora 包名，见 `Cargo.toml` 的 `package.metadata.deb` / `generate-rpm`）→ 创建**草稿 Release**
4. **确认与发版信息**：在 Releases 页面打开草稿，撰写/修订发版说明，确认后手动 **Publish**；publish 事件自动触发 AUR 稳定包 `ncaptura` 更新（以 `aur/PKGBUILD` 为模板替换 `pkgver`/`sha256sums` 并推送 AUR 仓库；需要 repo secret `AUR_SSH_PRIVATE_KEY`，未配置则跳过）。`ncaptura-git`（根目录 `PKGBUILD`）跟随 main，无需发版动作
   **注意**：README 的安装指引包含仓库自带 PKGBUILD（makepkg）方式；AUR 助手命令（`yay -S ncaptura` 等）在包正式注册到 AUR 之前不写入 README
5. **发版后验证**：在干净环境用 deb 或 tar.gz 安装，`ncaptura help` 与 `ncaptura ocr` 冒烟通过
