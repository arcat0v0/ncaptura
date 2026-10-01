# NCaptura

中文 | [English](docs/README_EN.md)

本项目是一个基于 GTK4 + Libadwaita 的截图/录屏/OCR/翻译工具，除了图形界面，也支持通过 CLI 快速调用各项能力。

这份文档聚焦 CLI 使用方式，方便你直接绑定快捷键或在脚本中调用。

## 支持的功能

- **截图**：区域框选 / 全屏（聚焦输出），自动保存到图片目录
- **录屏**：区域 / 全屏录制，可选系统音频，右上角 HUD 可随时暂停或停止
- **截图 OCR**：框选屏幕区域，基于 PaddleOCR（Paddle 推理模型，首次识别时自动下载）高精度识别中英文文字
- **划词翻译**：鼠标选中文本后按下快捷键即可翻译
- **输入翻译**：快捷键呼出翻译窗口，输入文本后回车翻译
- **截图翻译**：框选屏幕区域，识别文字并翻译为目标语言
- **结果弹窗**：Libadwaita 风格悬浮窗承载识别 / 翻译结果，支持直接编辑、删除换行、复制文本、切换目标语言；窗口随所在屏幕方向自适应布局

## 1. 环境要求

推荐在 Wayland 会话下使用。运行依赖以下软件包（Arch 包名）：

- `grim`：截图
- `slurp`：区域选择（`region` 目标需要）
- `wf-recorder`：录屏
- `wl-clipboard`：剪贴板读写（复制结果、读取划词选中文本）

OCR 功能另外需要系统 Python 3.10–3.13 之一（见第 3 节）。

## 2. 安装

### 预编译包（推荐）

从 [Releases](https://github.com/arcat0v0/ncaptura/releases) 页面下载对应格式：

- **deb**：`sudo dpkg -i ncaptura_*.deb`
- **rpm**：`sudo dnf install ncaptura-*.rpm`
- **tar.gz**：便携二进制包，解压后将 `ncaptura` 放入 `PATH` 即可（不含桌面入口与系统集成）

注意：预编译包不包含 `grim`/`slurp`/`wf-recorder`/`wl-clipboard` 等系统工具，请用发行版包管理器另行安装。

### 通过 PKGBUILD 安装（Arch Linux）

仓库根目录自带 `ncaptura-git` 的 PKGBUILD（跟踪 main 分支），直接构建安装：

```bash
git clone https://github.com/arcat0v0/ncaptura.git
cd ncaptura
makepkg -si
```

会自动安装依赖并完成构建；卸载用 `sudo pacman -R ncaptura-git`。

### 从源码安装

```bash
cargo install --path . --root ~/.local
```

确保 `~/.local/bin` 在 `PATH` 中，然后用 `ncaptura help` 验证。

## 3. OCR 初始化

OCR 识别基于 PaddleOCR 及 Paddle 推理模型（PP-OCR 系列），需要一次性初始化（会在 `~/.local/share/ncaptura/ocr-venv` 创建独立的 Python 虚拟环境，不污染系统 Python）：

```bash
ncaptura ocr setup
```

- 使用**系统自带**的 Python（自动探测 `/usr/bin/python3.13` ~ `3.10`），不引入额外工具链
- paddlepaddle 官方暂未发布 Python 3.14 的预编译包，因此需要系统中有 3.10–3.13 之一（Arch：`sudo pacman -S python310`）
- paddlepaddle 锁定 3.2.x：3.3.0/3.3.1 在 CPU 推理时会触发 oneDNN PIR 崩溃（`ConvertPirAttribute2RuntimeAttribute not support`）

**模型需要额外下载**：首次执行 OCR 识别时，会自动从 PaddleX 模型服务器（`paddle-model-ecology.bj.bcebos.com`）下载推理模型（约百余 MB）到 `~/.local/share/ncaptura/models`，请保持网络畅通；之后识别完全离线。

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
ncaptura ocr setup
ncaptura translate region
ncaptura translate selection
ncaptura translate input
```

- `ocr`：框选区域后调用 PaddleOCR 识别文字，弹出「文字识别」结果窗口
- `ocr setup`：初始化 / 重建 OCR Python 环境（见第 3 节）
- `translate region`：框选区域识别文字并翻译
- `translate selection`：读取鼠标选中的文本（primary selection）并翻译
- `translate input`：呼出输入翻译窗口，输入文本后回车翻译

结果窗口支持直接编辑文本、「删除换行」整理段落、「复制」到剪贴板（窗口关闭后剪贴板内容仍保留）。
OCR 窗口可一键「翻译」当前识别文本；翻译窗口可在标题栏选择目标语言并即时重译。
弹窗为悬浮覆盖层（见第 8 节），按 `Esc` 或右上角按钮关闭；
窗口尺寸随所在屏幕自适应：横屏为左图右文，竖屏自动切换为上图下文。

翻译通过 [mozhi](https://codeberg.org/aryak/mozhi) 公共实例完成（聚合 Google 等引擎，免 API key），
默认在多个实例间自动故障转移；中文内容自动译向英文，其他语言译向中文。
公共实例存在限流可能，频繁使用建议在配置文件中指向自托管实例（见第 5 节）。

### 帮助

```bash
ncaptura version
ncaptura help
```

## 5. 配置文件

可选配置文件：`~/.config/ncaptura/config.yaml`（不存在时全部使用缺省值）。示例（各项均为缺省行为，按需取消注释修改）：

```yaml
# 输出根目录（截图、录屏），支持 ~ 展开
# 缺省：图片目录/NCaptura（通常为 ~/Pictures/NCaptura）
# output_dir: ~/Pictures/NCaptura

ocr:
  # PaddleOCR 环境的 Python 解释器路径
  # 缺省：~/.local/share/ncaptura/ocr-venv/bin/python
  # 也可用环境变量 NCAPTURA_OCR_PYTHON 覆盖
  # python: /path/to/python

  # 模型缓存目录
  # 缺省：~/.local/share/ncaptura/models
  # model_dir: ~/.local/share/ncaptura/models

translate:
  # 翻译后端（当前仅 mozhi，为未来后端预留）
  # backend: mozhi

  # mozhi 实例地址；缺省为多个公共实例自动故障转移
  # 自托管示例（Arch 可安装 mozhi-git，默认端口 3000）：
  # mozhi_url: http://127.0.0.1:3000

  # 翻译引擎：google / duckduckgo / deepl / reverso / yandex / mymemory
  # engine: google

  # 固定目标语言；缺省按内容自动判断（中文→英文，其他→中文）
  # target: zh-CN
```

配置文件解析失败时会提示并回退到缺省值，不会影响使用。

## 6. 输出文件位置

默认保存到 `图片目录/NCaptura` 下（可用 `output_dir` 配置修改）：

- 截图：`~/Pictures/NCaptura/screenshots/`
- 录屏：`~/Pictures/NCaptura/recordings/`

文件名格式示例：

- `screenshot-region-20260224-213015.png`
- `recording-fullscreen-20260224-213102.mkv`

应用自身的文件按 XDG 目录规范组织：

| 内容 | 位置 |
| --- | --- |
| 配置文件 | `~/.config/ncaptura/config.yaml` |
| OCR Python 环境 | `~/.local/share/ncaptura/ocr-venv/` |
| OCR 模型 | `~/.local/share/ncaptura/models/` |
| OCR 辅助脚本（可删，自动重建） | `~/.cache/ncaptura/` |
| 录屏状态文件 | `~/.local/state/ncaptura/recording.json` |

## 7. 录屏状态文件（CLI）

CLI 录屏启动后会写入状态文件，用于后续 `record stop`：

- `~/.local/state/ncaptura/recording.json`

如果你的系统设置了 `XDG_STATE_HOME`，则会使用对应状态目录。

## 8. niri 快捷键示例

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

## 9. 常见问题

### `record stop` 提示无法读取状态文件

通常表示当前没有由 CLI 启动的录屏，或状态文件已被清理。请先执行 `record start ...` 再停止。

### 提示某命令不存在（如 `grim`/`wf-recorder`）

请先安装依赖并确保命令在 `PATH` 中。

### `region` 无法选择区域

请确认 `slurp` 已安装，并且当前会话支持交互式区域选择。

### OCR 提示「未找到 OCR Python 环境」

执行 `ncaptura ocr setup` 完成初始化；需要系统 Python 3.10–3.13。
首次识别会下载模型，耗时较长属正常现象。

### 翻译失败或超时

公共 mozhi 实例可能限流，稍后再试，或在 `config.yaml` 中配置 `translate.mozhi_url` 指向自托管实例。
