"""Render the emTasks app icon (a 2x2 Eisenhower matrix) as a 1024px PNG.

Pure Python, no dependencies. Usage: python3 make_icon.py out.png
"""
import math
import struct
import sys
import zlib

S = 1024
BG = (31, 36, 48)
QUADS = [  # (col, row, color)
    (0, 0, (225, 85, 85)),    # Do
    (1, 0, (80, 140, 230)),   # Schedule
    (0, 1, (235, 155, 50)),   # Delegate
    (1, 1, (120, 170, 120)),  # Eliminate
]


def sd_round_rect(px, py, cx, cy, hw, hh, r):
    qx = abs(px - cx) - hw + r
    qy = abs(py - cy) - hh + r
    outside = math.hypot(max(qx, 0.0), max(qy, 0.0))
    return outside + min(max(qx, qy), 0.0) - r


def sd_segment(px, py, ax, ay, bx, by):
    pax, pay, bax, bay = px - ax, py - ay, bx - ax, by - ay
    h = max(0.0, min(1.0, (pax * bax + pay * bay) / (bax * bax + bay * bay)))
    return math.hypot(pax - bax * h, pay - bay * h)


def cover(d):
    """Signed distance -> pixel coverage with 1px anti-aliasing."""
    return max(0.0, min(1.0, 0.5 - d))


def blend(dst, src, a):
    return tuple(d + (s - d) * a for d, s in zip(dst, src))


def render():
    # macOS icon grid: 824px body centred in 1024 canvas.
    body_c, body_hw, body_r = S / 2, 412, 185
    gap, pad = 36, 92
    cell = (2 * body_hw - 2 * pad - gap) / 2
    left = body_c - body_hw + pad
    # Check mark inside the "Do" quadrant.
    qx0, qy0 = left, left
    check = [
        (qx0 + cell * 0.24, qy0 + cell * 0.52),
        (qx0 + cell * 0.43, qy0 + cell * 0.70),
        (qx0 + cell * 0.78, qy0 + cell * 0.31),
    ]
    stroke = cell * 0.075

    rows = []
    for y in range(S):
        row = bytearray([0])
        py = y + 0.5
        for x in range(S):
            px = x + 0.5
            a_body = cover(sd_round_rect(px, py, body_c, body_c, body_hw, body_hw, body_r))
            if a_body <= 0:
                row += b"\0\0\0\0"
                continue
            col = BG
            for cx, cy, c in QUADS:
                x0 = left + cx * (cell + gap)
                y0 = left + cy * (cell + gap)
                a = cover(sd_round_rect(px, py, x0 + cell / 2, y0 + cell / 2, cell / 2, cell / 2, 44))
                if a > 0:
                    col = blend(col, c, a)
            d = min(sd_segment(px, py, *check[0], *check[1]), sd_segment(px, py, *check[1], *check[2]))
            a = cover(d - stroke)
            if a > 0:
                col = blend(col, (255, 255, 255), a)
            row += bytes(int(round(v)) for v in col) + bytes([int(round(a_body * 255))])
        rows.append(bytes(row))
    return b"".join(rows)


def write_png(path, raw):
    def chunk(tag, data):
        c = struct.pack(">I", len(data)) + tag + data
        return c + struct.pack(">I", zlib.crc32(tag + data) & 0xFFFFFFFF)

    png = b"\x89PNG\r\n\x1a\n"
    png += chunk(b"IHDR", struct.pack(">IIBBBBB", S, S, 8, 6, 0, 0, 0))
    png += chunk(b"IDAT", zlib.compress(raw, 9))
    png += chunk(b"IEND", b"")
    with open(path, "wb") as f:
        f.write(png)


if __name__ == "__main__":
    write_png(sys.argv[1], render())
