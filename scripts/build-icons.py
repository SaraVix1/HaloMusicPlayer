#!/usr/bin/env python3
"""Export Halo's app icons from the SVG sources.

Sources (crates/halo-app/assets/icons/src/):
  halo.svg       master, used for 48 px and up
  halo-32.svg    hand-tuned on the 32 px grid (also used for 40 px)
  halo-24.svg    hand-tuned on the 24 px grid (also used for 20 px)
  halo-16.svg    hand-tuned on the 16 px grid

Outputs (crates/halo-app/assets/icons/):
  16x16.png … 512x512.png   Linux hicolor, window and tray icons
  icon.ico                  Windows exe / installer / taskbar / tray (16–256)
  icon.icns                 macOS (16–1024)

Usage:  python3 scripts/build-icons.py
Needs:  python3-gi + gir1.2-rsvg-2.0 (librsvg), python3-cairo, python3-pil
        (Debian/Ubuntu package names). Commit the outputs; nothing in the
        build runs this script.
"""

import io
import sys
from pathlib import Path

import gi

gi.require_version("Rsvg", "2.0")
import cairo  # noqa: E402
from gi.repository import Rsvg  # noqa: E402
from PIL import Image  # noqa: E402

ROOT = Path(__file__).resolve().parent.parent
ICONS = ROOT / "crates" / "halo-app" / "assets" / "icons"
SRC = ICONS / "src"

PNG_SIZES = [16, 24, 32, 48, 64, 128, 256, 512]
ICO_SIZES = [16, 20, 24, 32, 40, 48, 64, 256]
ICNS_SIZES = [16, 32, 64, 128, 256, 512, 1024]


def source_for(size: int) -> Path:
    """Smallest hand-tuned source drawn for this size or the next one up."""
    if size <= 16:
        return SRC / "halo-16.svg"
    if size <= 24:
        return SRC / "halo-24.svg"
    if size <= 40:
        return SRC / "halo-32.svg"
    return SRC / "halo.svg"


def render(size: int) -> Image.Image:
    handle = Rsvg.Handle.new_from_file(str(source_for(size)))
    surface = cairo.ImageSurface(cairo.FORMAT_ARGB32, size, size)
    viewport = Rsvg.Rectangle()
    viewport.x, viewport.y, viewport.width, viewport.height = 0, 0, size, size
    handle.render_document(cairo.Context(surface), viewport)
    buf = io.BytesIO()
    surface.write_to_png(buf)  # cairo un-premultiplies alpha here
    buf.seek(0)
    return Image.open(buf).convert("RGBA")


def main() -> int:
    renders = {s: render(s) for s in sorted(set(PNG_SIZES + ICO_SIZES + ICNS_SIZES))}

    for s in PNG_SIZES:
        out = ICONS / f"{s}x{s}.png"
        renders[s].save(out, optimize=True)
        print(f"wrote {out.relative_to(ROOT)}")

    # Pillow takes each entry from the image of exactly that size when one
    # is provided (so the hand-tuned small sizes are kept, not resampled).
    largest = renders[max(ICO_SIZES)]
    largest.save(
        ICONS / "icon.ico",
        sizes=[(s, s) for s in ICO_SIZES],
        append_images=[renders[s] for s in ICO_SIZES if s != max(ICO_SIZES)],
    )
    print(f"wrote {(ICONS / 'icon.ico').relative_to(ROOT)}")

    largest = renders[max(ICNS_SIZES)]
    largest.save(
        ICONS / "icon.icns",
        append_images=[renders[s] for s in ICNS_SIZES if s != max(ICNS_SIZES)],
    )
    print(f"wrote {(ICONS / 'icon.icns').relative_to(ROOT)}")
    return 0


if __name__ == "__main__":
    sys.exit(main())
