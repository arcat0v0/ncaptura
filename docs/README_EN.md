# NCaptura CLI Quick Guide

[中文](../README.md) | English

NCaptura is a GTK4 + Libadwaita screenshot / screen-recording / OCR / translation tool. Besides the graphical UI, every capability is also available through the CLI.

This document focuses on CLI usage, so you can bind hotkeys or call it from scripts directly.

## Features

- **Screenshot**: interactive region / fullscreen (focused output), saved to your pictures directory
- **Screen recording**: region / fullscreen, optional system audio, with a top-right HUD to pause or stop anytime
- **Screenshot OCR**: select a screen region and recognize Chinese/English text with PaddleOCR (Paddle inference models, downloaded automatically on first recognition)
- **Selection translation**: select text with the mouse, press the hotkey, and get it translated
- **Input translation**: hotkey opens a translation window; type the text and press Enter
- **Screenshot translation**: select a screen region to recognize its text and translate it into the target language
- **Result popups**: Libadwaita-style floating windows carrying OCR / translation results, with in-place editing, line-break removal, copy-to-clipboard, and target-language switching; the layout adapts to the orientation of the screen the window lands on

## 1. Requirements

A Wayland session is recommended. Runtime dependencies (Arch package names):

- `grim`: screenshots
- `slurp`: region selection (required for the `region` target)
- `wf-recorder`: screen recording
- `wl-clipboard`: clipboard read/write (copying results, reading the primary selection)
- `libpulse`: optional, only used for automatic audio device selection with `--audio` (pactl)

OCR additionally requires a system Python 3.10–3.13 (see section 3).

## 2. Installation

### Prebuilt packages (recommended)

Download the matching format from the [Releases](https://github.com/arcat0v0/ncaptura/releases) page:

- **deb**: `sudo dpkg -i ncaptura_*.deb`
- **rpm**: `sudo dnf install ncaptura-*.rpm`
- **AppImage**: provided as a portable CLI form (no desktop entry or system integration); `chmod +x ncaptura-*.AppImage` and invoke from the command line

Note: prebuilt packages do not bundle system tools like `grim`/`slurp`/`wf-recorder`/`wl-clipboard`; install them with your distribution's package manager.

### From source

```bash
cargo install --path . --root ~/.local
```

Make sure `~/.local/bin` is in your `PATH`, then verify with `ncaptura help`.

## 3. OCR Setup

OCR recognition is built on PaddleOCR and its Paddle inference models (the PP-OCR series) and requires a one-time setup (it creates an isolated Python virtual environment at `~/.local/share/ncaptura/ocr-venv`, leaving your system Python untouched):

```bash
ncaptura ocr setup
```

- Uses the **system** Python (auto-detects `/usr/bin/python3.13` ~ `3.10`), no extra toolchain is introduced
- PaddlePaddle does not publish prebuilt wheels for Python 3.14 yet, so one of 3.10–3.13 must be present (Arch: `sudo pacman -S python310`)
- paddlepaddle is pinned to 3.2.x: 3.3.0/3.3.1 crash on CPU inference inside oneDNN PIR (`ConvertPirAttribute2RuntimeAttribute not support`)

**Models are downloaded separately**: the first OCR run automatically downloads inference models (roughly a hundred MB) from the PaddleX model server (`paddle-model-ecology.bj.bcebos.com`) into `~/.local/share/ncaptura/models`; keep the network available for that first run — recognition is fully offline afterwards.

## 4. CLI Reference

### Screenshots

```bash
ncaptura screenshot region
ncaptura screenshot fullscreen
```

- `region`: interactive region selection via `slurp`
- `fullscreen`: full-screen capture (prefers the focused output under niri)

### Recording

```bash
ncaptura record start region
ncaptura record start fullscreen
ncaptura record start region --audio
ncaptura record start fullscreen --audio
ncaptura record stop
```

- `record start ...`: starts a background recording with a top-right HUD (pause/stop)
- `--audio`: records audio
- `record stop`: stops the recording started via the CLI

### OCR & Translation

```bash
ncaptura ocr
ncaptura ocr setup
ncaptura translate region
ncaptura translate selection
ncaptura translate input
```

- `ocr`: select a region, recognize text with PaddleOCR, and open the result window
- `ocr setup`: initialize / rebuild the OCR Python environment (see section 3)
- `translate region`: select a region, recognize its text, and translate
- `translate selection`: translate the mouse-selected text (primary selection)
- `translate input`: open the input-translation window; type and press Enter

Result windows support in-place editing, line-break removal ("整理段落"), and copying to the clipboard (content survives window close).
The OCR window can send the current text to translation in one click; the translation window lets you pick the target language from the header and re-translates instantly.
Popups float as overlay layers (see section 8); close them with `Esc` or the header button.
Window size adapts to the current screen: image-left/text-right on landscape displays, image-top/text-bottom on portrait ones.

Translation is served by public [mozhi](https://codeberg.org/aryak/mozhi) instances (aggregating Google and other engines, no API key needed),
failing over across multiple instances by default; Chinese content auto-translates to English, other languages to Chinese.
Public instances may be rate-limited; heavy users should point the config file at a self-hosted instance (see section 5).

### Help

```bash
ncaptura help
```

## 5. Configuration File

Optional config file: `~/.config/ncaptura/config.yaml` (defaults apply when absent). Example (every item shows its default behavior; uncomment to change):

```yaml
# Output root directory (screenshots, recordings); ~ expansion supported
# Default: pictures directory/NCaptura (usually ~/Pictures/NCaptura)
# output_dir: ~/Pictures/NCaptura

ocr:
  # Python interpreter of the PaddleOCR environment
  # Default: ~/.local/share/ncaptura/ocr-venv/bin/python
  # Overridable via the NCAPTURA_OCR_PYTHON environment variable
  # python: /path/to/python

  # Model cache directory
  # Default: ~/.local/share/ncaptura/models
  # model_dir: ~/.local/share/ncaptura/models

translate:
  # Translation backend (only mozhi today; reserved for future backends)
  # backend: mozhi

  # mozhi instance URL; defaults to automatic failover across public instances
  # Self-hosted example (mozhi-git on Arch, default port 3000):
  # mozhi_url: http://127.0.0.1:3000

  # Translation engine: google / duckduckgo / deepl / reverso / yandex / mymemory
  # engine: google

  # Fixed target language; default auto-detects by content (Chinese→English, others→Chinese)
  # target: zh-CN
```

If the config file fails to parse, a warning is printed and defaults are used — nothing breaks.

## 6. Output Locations

Saved under `pictures directory/NCaptura` by default (changeable via `output_dir`):

- Screenshots: `~/Pictures/NCaptura/screenshots/`
- Recordings: `~/Pictures/NCaptura/recordings/`

File name examples:

- `screenshot-region-20260224-213015.png`
- `recording-fullscreen-20260224-213102.mkv`

Application data follows the XDG directory specification:

| Content | Location |
| --- | --- |
| Config file | `~/.config/ncaptura/config.yaml` |
| OCR Python environment | `~/.local/share/ncaptura/ocr-venv/` |
| OCR models | `~/.local/share/ncaptura/models/` |
| OCR helper script (auto-rebuilt if deleted) | `~/.cache/ncaptura/` |
| Recording state file | `~/.local/state/ncaptura/recording.json` |

## 7. Recording State File (CLI)

A recording started via the CLI writes a state file used by a later `record stop`:

- `~/.local/state/ncaptura/recording.json`

If `XDG_STATE_HOME` is set, the corresponding state directory is used instead.

## 8. niri Hotkey Examples

Bind directly in your niri config:

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

OCR / translation popups float above the screen as wlr-layer-shell overlay layers (the same mechanism as the recording HUD);
on compositors supporting the protocol, such as niri, they are never tiled and need no window rules;
on desktops without layer-shell they fall back to regular windows.

## 9. FAQ

### `record stop` complains about an unreadable state file

Usually means no CLI-started recording exists, or the state file was cleaned up. Run `record start ...` before stopping.

### A command is reported missing (e.g. `grim`/`wf-recorder`)

Install the dependencies and make sure the commands are in your `PATH`.

### `region` selection does not work

Make sure `slurp` is installed and your session supports interactive region selection.

### OCR reports "OCR Python environment not found"

Run `ncaptura ocr setup`; a system Python 3.10–3.13 is required.
The first recognition downloads models, so a long wait is normal.

### Translation fails or times out

Public mozhi instances may be rate-limited; retry later, or point `translate.mozhi_url` in `config.yaml` at a self-hosted instance.
