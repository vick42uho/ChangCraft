#!/usr/bin/env python3
"""
Generate all ChangCraft icons from the master Thai elephant image.
Generates:
  - assets/app-icon/pdfcraft.ico (multi-resolution 16, 20, 24, 32, 40, 48, 64, 128, 256 px)
  - assets/app-icon/pdfcraft-1024.png
  - assets/app-icon/hicolor/<size>x<size>/apps/ai.storyteller.pdfcraft.png
"""

import os
import struct
import sys
from io import BytesIO
from pathlib import Path
from PIL import Image

ROOT = Path(__file__).resolve().parent.parent
APP_ICON_DIR = ROOT / "assets" / "app-icon"
DEFAULT_SOURCE = APP_ICON_DIR / "changcraft-master.png"

def make_square(img: Image.Image) -> Image.Image:
    """Pad image to a square with transparent background, keeping aspect ratio."""
    w, h = img.size
    max_side = max(w, h)
    square = Image.new("RGBA", (max_side, max_side), (0, 0, 0, 0))
    offset = ((max_side - w) // 2, (max_side - h) // 2)
    square.paste(img, offset)
    return square

def generate_icons(source_path: Path):
    if not source_path.exists():
        print(f"Error: Source image not found: {source_path}", file=sys.stderr)
        sys.exit(1)

    print(f"Loading master icon: {source_path}")
    raw_img = Image.open(source_path).convert("RGBA")
    square_img = make_square(raw_img)

    # 1. Generate 1024x1024
    p1024 = square_img.resize((1024, 1024), Image.Resampling.LANCZOS)
    p1024_path = APP_ICON_DIR / "pdfcraft-1024.png"
    p1024.save(p1024_path, "PNG")
    print(f"Saved: {p1024_path}")

    # 2. Generate hicolor resolutions
    hicolor_sizes = [16, 24, 32, 48, 64, 128, 256, 512]
    for s in hicolor_sizes:
        hdir = APP_ICON_DIR / "hicolor" / f"{s}x{s}" / "apps"
        hdir.mkdir(parents=True, exist_ok=True)
        himg = square_img.resize((s, s), Image.Resampling.LANCZOS)
        out_png = hdir / "ai.storyteller.pdfcraft.png"
        himg.save(out_png, "PNG")
        print(f"Saved: {out_png}")

    # 3. Generate Windows multi-resolution .ico (PNG compressed)
    ico_sizes = [16, 20, 24, 32, 40, 48, 64, 128, 256]
    blobs = []
    for s in ico_sizes:
        rimg = square_img.resize((s, s), Image.Resampling.LANCZOS)
        buf = BytesIO()
        rimg.save(buf, format="PNG")
        blobs.append((s, buf.getvalue()))

    # Build standard Windows ICO header with PNG data
    head = struct.pack("<HHH", 0, 1, len(blobs))
    entries = b""
    data = b""
    offset = 6 + 16 * len(blobs)
    for s, b in blobs:
        w = s % 256
        h = s % 256
        entries += struct.pack("<BBBBHHII", w, h, 0, 0, 1, 32, len(b), offset)
        data += b
        offset += len(b)

    ico_path = APP_ICON_DIR / "pdfcraft.ico"
    with open(ico_path, "wb") as f:
        f.write(head + entries + data)
    print(f"Saved: {ico_path} with sizes: {ico_sizes}")

if __name__ == "__main__":
    src = Path(sys.argv[1]) if len(sys.argv) > 1 else DEFAULT_SOURCE
    generate_icons(src)
