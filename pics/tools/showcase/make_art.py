"""Illustration de démonstration (œuvre originale, générée) : une orange sur fond crème, supersamplée ×2."""
import math
import random

from PIL import Image, ImageChops, ImageDraw, ImageFilter

W, H, S = 1600, 1067, 2
w, h = W * S, H * S
random.seed(7)

# Fond crème, léger dégradé vertical.
bg = Image.new("RGB", (w, h))
px = bg.load()
for y in range(h):
    t = y / h
    c = (int(246 - 10 * t), int(240 - 12 * t), int(228 - 14 * t))
    for x in range(w):
        px[x, y] = c

cx, cy, r = w * 0.5, h * 0.52, h * 0.30

# Ombre portée douce.
shadow = Image.new("L", (w, h), 0)
ImageDraw.Draw(shadow).ellipse([cx - r * 0.95, cy + r * 0.86, cx + r * 0.95, cy + r * 1.12], fill=150)
shadow = shadow.filter(ImageFilter.GaussianBlur(S * 26))
dark = Image.new("RGB", (w, h), (120, 96, 70))
bg = Image.composite(dark, bg, shadow.point(lambda v: int(v * 0.55)))

# Orange : dégradé radial décalé vers la lumière (haut-gauche).
orange = Image.new("RGB", (w, h))
op = orange.load()
lx, ly = cx - r * 0.35, cy - r * 0.4
for y in range(int(cy - r) - 2, int(cy + r) + 3):
    for x in range(int(cx - r) - 2, int(cx + r) + 3):
        d = math.hypot(x - lx, y - ly) / (r * 1.55)
        d = min(max(d, 0.0), 1.0)
        a = (255, 186, 74)
        b = (240, 130, 10)
        c2 = (176, 80, 6)
        if d < 0.55:
            t = d / 0.55
            col = tuple(a[i] + (b[i] - a[i]) * t for i in range(3))
        else:
            t = (d - 0.55) / 0.45
            col = tuple(b[i] + (c2[i] - b[i]) * t for i in range(3))
        n = random.random() * 10 - 5  # grain de peau
        op[x, y] = tuple(int(min(255, max(0, v + n))) for v in col)
mask = Image.new("L", (w, h), 0)
ImageDraw.Draw(mask).ellipse([cx - r, cy - r, cx + r, cy + r], fill=255)
bg.paste(orange, (0, 0), mask)

# Reflet.
hl = Image.new("L", (w, h), 0)
ImageDraw.Draw(hl).ellipse([cx - r * 0.62, cy - r * 0.72, cx - r * 0.12, cy - r * 0.34], fill=120)
hl = hl.filter(ImageFilter.GaussianBlur(S * 16))
bg.paste(Image.new("RGB", (w, h), (255, 236, 200)), (0, 0), ImageChops.multiply(hl, mask))

# Tige et feuille.
d = ImageDraw.Draw(bg)
d.polygon([(cx - 14 * S, cy - r + 6 * S), (cx + 14 * S, cy - r + 6 * S), (cx + 8 * S, cy - r - 46 * S), (cx - 6 * S, cy - r - 46 * S)], fill=(92, 62, 30))
leaf = [(cx + 10 * S, cy - r - 8 * S), (cx + 120 * S, cy - r - 70 * S), (cx + 230 * S, cy - r - 24 * S), (cx + 130 * S, cy - r + 30 * S)]
d.polygon(leaf, fill=(64, 140, 62))
d.line([(cx + 14 * S, cy - r - 4 * S), (cx + 200 * S, cy - r - 28 * S)], fill=(40, 100, 44), width=3 * S)

bg.resize((W, H), Image.LANCZOS).save("assets/orange.png", optimize=True)
print("ok")
