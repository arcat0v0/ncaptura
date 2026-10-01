# NCaptura CLI 快速指南

本项目是一个基于 GTK4 + Libadwaita 的截图/录屏/OCR/翻译工具，除了图形界面，也支持通过 CLI 快速调用各项能力。

这份文档聚焦 CLI 使用方式，方便你直接绑定快捷键或在脚本中调用。

当前版本：`0.1.0`

## 1. 环境要求

推荐在 Wayland 会话下使用，并确保以下命令可用：

- `grim`：截图
- `slurp`：区域选择（`region` 目标需要）
- `wf-recorder`：录屏
- `wl-clipboard`：剪贴板读写（复制结果、读取划词选中文本）
- `pactl`：可选，仅在 `--audio` 时用于自动选择系统混音设备
- `niri`：可选，在 `fullscreen` 模式下用于识别当前聚焦输出

## 2. 通过 PKGBUILD 安装（Arch Linux / AUR）

当前仓库提供 `ncaptura-git` 的 `PKGBUILD`，可按以下方式安装。

先安装构建工具：

```bash
sudo pacman -S --needed base-devel git
```

### 方式 A：本地 PKGBUILD 构建安装

在本仓库根目录执行：

```bash
makepkg -si
```

### 方式 B：发布到 AUR 后通过 AUR Helper 安装

当 AUR 包上线后可直接安装：

```bash
yay -S ncaptura-git
```

安装完成后可用以下命令验证：

```bash
ncaptura help
```

## 3. 快速运行方式

如果你还没安装二进制，可直接通过 Cargo 调用：

```bash
cargo run -- screenshot region
cargo run -- record start region
```

如果你已经有 `ncaptura` 可执行文件（例如 `target/release/ncaptura` 放进了 `PATH`），推荐直接使用：

```bash
ncaptura help
```

## 4. CLI 命令一览

### 截图

```bash
ncaptura screenshot region
ncaptura screenshot fullscreen
```

- `region`：调用 `slurp` 交互框选区域
- `fullscreen`：全屏截图（在 niri 下会优先当前聚焦输出）

### 录屏

```bash
ncaptura record start region
ncaptura record start fullscreen
ncaptura record start region --audio
ncaptura record start fullscreen --audio
ncaptura record stop
```

- `record start ...`：启动后台录制并弹出右上角 HUD（可暂停/停止）
- `--audio`：开启音频录制
- `record stop`：停止当前由 CLI 启动的录屏

### OCR 与翻译


```bash
ncaptura ocr
ncaptura translate region
ncaptura translate selection
ncaptura translate input
```

- `ocr`：框选区域后调用 PaddleOCR 识别文字，弹出「文字识别」结果窗口
- `translate region`：框选区域识别文字并翻译（翻译 API 尚未接入，会提示未接入）
- `translate selection`：读取鼠标选中的文本（primary selection）并翻译
- `translate input`：呼出输入翻译窗口，输入文本后回车翻译

结果窗口支持直接编辑文本、「删除换行」整理段落、「复制」到剪贴板。

OCR 依赖独立的 Python 环境（不会污染系统 Python），首次使用前执行：

```bash
scripts/setup-ocr.sh
```

脚本通过 `uv` 在 `~/.local/share/ncaptura/ocr-venv` 创建虚拟环境并安装 `paddlepaddle==3.2.2` 与 `paddleocr`。
注意：paddlepaddle 必须保持 3.2.x，3.3.0/3.3.1 在 CPU 推理时会触发 oneDNN PIR 崩溃（`ConvertPirAttribute2RuntimeAttribute not support`）。
首次识别会自动下载模型到 `~/.paddlex`。如需自定义 Python 环境，设置 `NCAPTURA_OCR_PYTHON` 指向目标解释器。

### 帮助

```bash
ncaptura help
```

## 5. 输出文件位置

默认保存到 `图片目录/NCaptura` 下：

- 截图：`~/Pictures/NCaptura/screenshots/`
- 录屏：`~/Pictures/NCaptura/recordings/`

文件名格式示例：

- `screenshot-region-20260224-213015.png`
- `recording-fullscreen-20260224-213102.mkv`

## 6. 录屏状态文件（CLI）

CLI 录屏启动后会写入状态文件，用于后续 `record stop`：

- `~/.local/state/ncaptura/recording.json`

如果你的系统设置了 `XDG_STATE_HOME`，则会使用对应状态目录。

## 7. niri 快捷键示例

可在 niri 配置中直接绑定：

```kdl
Mod+Shift+S    { spawn "ncaptura" "screenshot" "region"; }
Mod+Shift+F    { spawn "ncaptura" "screenshot" "fullscreen"; }
Mod+Shift+R    { spawn "ncaptura" "record" "start" "region"; }
Mod+Shift+A    { spawn "ncaptura" "record" "start" "region" "--audio"; }
Mod+Shift+E    { spawn "ncaptura" "record" "stop"; }
Mod+Shift+O    { spawn "ncaptura" "ocr"; }
Mod+Shift+T    { spawn "ncaptura" "translate" "region"; }
Mod+Shift+Y    { spawn "ncaptura" "translate" "selection"; }
Mod+Shift+I    { spawn "ncaptura" "translate" "input"; }
```

OCR / 翻译弹窗通过 wlr-layer-shell 协议以覆盖层形式悬浮在屏幕上（与录屏 HUD 同一机制），
在 niri 等支持该协议的合成器上不会被平铺，无需任何窗口规则配置；
在不支持 layer-shell 的桌面上则回退为普通窗口。

## 8. 常见问题

### `record stop` 提示无法读取状态文件

通常表示当前没有由 CLI 启动的录屏，或状态文件已被清理。请先执行 `record start ...` 再停止。

### 提示某命令不存在（如 `grim`/`wf-recorder`）

请先安装依赖并确保命令在 `PATH` 中。

### `region` 无法选择区域

请确认 `slurp` 已安装，并且当前会话支持交互式区域选择。
