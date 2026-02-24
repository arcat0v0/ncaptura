# NCaptura CLI 快速指南

本项目是一个基于 GTK4 + Libadwaita 的截图/录屏工具，除了图形界面，也支持通过 CLI 快速调用截图与录屏能力。

这份文档聚焦 CLI 使用方式，方便你直接绑定快捷键或在脚本中调用。

## 1. 环境要求

推荐在 Wayland 会话下使用，并确保以下命令可用：

- `grim`：截图
- `slurp`：区域选择（`region` 目标需要）
- `wf-recorder`：录屏
- `pactl`：可选，仅在 `--audio` 时用于自动选择系统混音设备
- `niri`：可选，在 `fullscreen` 模式下用于识别当前聚焦输出

## 2. 快速运行方式

如果你还没安装二进制，可直接通过 Cargo 调用：

```bash
cargo run -- screenshot region
cargo run -- record start region
```

如果你已经有 `ncaptura` 可执行文件（例如 `target/release/ncaptura` 放进了 `PATH`），推荐直接使用：

```bash
ncaptura help
```

## 3. CLI 命令一览

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

### 帮助

```bash
ncaptura help
```

## 4. 输出文件位置

默认保存到 `图片目录/NCaptura` 下：

- 截图：`~/Pictures/NCaptura/screenshots/`
- 录屏：`~/Pictures/NCaptura/recordings/`

文件名格式示例：

- `screenshot-region-20260224-213015.png`
- `recording-fullscreen-20260224-213102.mkv`

## 5. 录屏状态文件（CLI）

CLI 录屏启动后会写入状态文件，用于后续 `record stop`：

- `~/.local/state/ncaptura/recording.json`

如果你的系统设置了 `XDG_STATE_HOME`，则会使用对应状态目录。

## 6. niri 快捷键示例

可在 niri 配置中直接绑定：

```kdl
Mod+Shift+S    { spawn "ncaptura" "screenshot" "region"; }
Mod+Shift+F    { spawn "ncaptura" "screenshot" "fullscreen"; }
Mod+Shift+R    { spawn "ncaptura" "record" "start" "region"; }
Mod+Shift+A    { spawn "ncaptura" "record" "start" "region" "--audio"; }
Mod+Shift+E    { spawn "ncaptura" "record" "stop"; }
```

## 7. 常见问题

### `record stop` 提示无法读取状态文件

通常表示当前没有由 CLI 启动的录屏，或状态文件已被清理。请先执行 `record start ...` 再停止。

### 提示某命令不存在（如 `grim`/`wf-recorder`）

请先安装依赖并确保命令在 `PATH` 中。

### `region` 无法选择区域

请确认 `slurp` 已安装，并且当前会话支持交互式区域选择。
