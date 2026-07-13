"""Generate Lumia's multi-resolution Windows icon from the canonical PNG."""

from pathlib import Path

from PIL import Image


ROOT = Path(__file__).resolve().parents[1]
SOURCE = ROOT / "crates" / "lumia-app" / "resources" / "icon.png"
OUTPUT = ROOT / "crates" / "lumia-app" / "resources" / "icon.ico"
SIZES = (16, 24, 32, 48, 64, 128, 256)


def main() -> None:
    with Image.open(SOURCE) as source:
        rgba = source.convert("RGBA")
        if rgba.size != (512, 512):
            raise SystemExit(f"expected a 512x512 source PNG, got {rgba.size}")
        rgba.save(OUTPUT, format="ICO", sizes=[(size, size) for size in SIZES])


if __name__ == "__main__":
    main()
