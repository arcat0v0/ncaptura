#!/usr/bin/env bash
set -euo pipefail

VENV_DIR="${XDG_DATA_HOME:-$HOME/.local/share}/ncaptura/ocr-venv"

if ! command -v uv >/dev/null 2>&1; then
    echo "错误: 未找到 uv，请先安装 uv (https://docs.astral.sh/uv/)" >&2
    exit 1
fi

uv venv "$VENV_DIR" --python 3.12 --seed
uv pip install --python "$VENV_DIR/bin/python" 'paddlepaddle==3.2.2' 'paddleocr>=3,<4'

echo "OCR 环境已就绪: $VENV_DIR"
echo "首次识别会自动下载 PaddleOCR 模型到 ~/.paddlex，请保持网络畅通。"
