import json
import sys


def extract_lines(result):
    lines = []
    for page in result:
        data = page
        if not isinstance(page, dict):
            data = getattr(page, "json", None)
            if isinstance(data, dict) and "res" in data:
                data = data["res"]
        if isinstance(data, dict) and "rec_texts" in data:
            lines.extend(str(text) for text in data["rec_texts"])
            continue
        if isinstance(page, (list, tuple)):
            for entry in page:
                if isinstance(entry, (list, tuple)) and len(entry) >= 2:
                    text_part = entry[1]
                    if isinstance(text_part, (list, tuple)):
                        lines.append(str(text_part[0]))
                    else:
                        lines.append(str(text_part))
    return lines


def main():
    if len(sys.argv) != 2:
        print(json.dumps({"error": "usage: ocr_helper.py <image>"}, ensure_ascii=False))
        return 2

    image_path = sys.argv[1]

    from paddleocr import PaddleOCR

    try:
        engine = PaddleOCR(
            use_doc_orientation_classify=False,
            use_doc_unwarping=False,
            use_textline_orientation=False,
            lang="ch",
        )
    except TypeError:
        engine = PaddleOCR(use_angle_cls=True, lang="ch")

    if hasattr(engine, "predict"):
        result = engine.predict(image_path)
    else:
        result = engine.ocr(image_path, cls=False)

    lines = [line for line in extract_lines(result) if line.strip()]
    print(json.dumps({"lines": lines}, ensure_ascii=False))
    return 0


if __name__ == "__main__":
    sys.exit(main())
