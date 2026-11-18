"""Procedural photo-real hardware UI renderer shared by the Nyashk analog plugins.

Draws the whole static faceplate (chassis, print, knob bodies, buttons in their off state,
screens and meter faces) into one background picture, plus the few overlays the GUI shows on
top (lit buttons, glass), and writes `src/hw_layout.rs` with the positions of every control.
Everything is numpy + Pillow; rerun with `python3 tools/render_ui.py`.
"""
import math, os
import numpy as np
from PIL import Image, ImageDraw, ImageFont, ImageFilter
from scipy.ndimage import gaussian_filter

S = 2  # texture scale (2x the logical GUI size)
RNG = np.random.default_rng(20261118)
L = np.array([-0.45, -0.65, 0.62]); L /= np.linalg.norm(L)
FONT_DIR = '/usr/share/fonts/truetype/dejavu/'

def g(a, s, mode='reflect'): return gaussian_filter(np.asarray(a, np.float32), s, mode=mode)
def noise(h, w, s):
    n = g(RNG.standard_normal((h, w)), s); return n / (n.std() + 1e-9)
def smooth(e0, e1, x):
    t = np.clip((x - e0) / (e1 - e0), 0, 1); return t * t * (3 - 2 * t)
def normals(hm):
    gy, gx = np.gradient(hm)
    n = np.dstack([-gx, -gy, np.ones_like(hm)]); n /= np.linalg.norm(n, axis=2, keepdims=True); return n
def font(sz, bold=False, cond=False):
    return ImageFont.truetype(FONT_DIR + 'DejaVuSans' + ('Condensed' if cond else '') + ('-Bold' if bold else '') + '.ttf', int(sz))
def to_img(rgb, a=None):
    rgb = np.nan_to_num(np.clip(rgb, 0, 1))
    if a is None: return Image.fromarray((rgb * 255 + .5).astype(np.uint8), 'RGB')
    return Image.fromarray((np.dstack([rgb, np.clip(np.nan_to_num(a), 0, 1)[..., None]]) * 255 + .5).astype(np.uint8), 'RGBA')
def disc(N, r, cx=None, cy=None):
    cx = N / 2 if cx is None else cx; cy = N / 2 if cy is None else cy
    y, x = np.mgrid[0:N, 0:N] + 0.5; d = np.hypot(x - cx, y - cy)
    return np.clip(r - d + 0.5, 0, 1), d, np.arctan2(y - cy, x - cx)

# ------------------------------------------------------------------ shading
def envmap(r, rough):
    rx, ry, rz = r[..., 0], r[..., 1], r[..., 2]
    u = rx / (1 + np.abs(rz) + 1e-6); v = ry / (1 + np.abs(rz) + 1e-6)
    w = 0.04 + rough * 0.35
    def box(cx, cy, sx, sy, I):
        d = np.maximum(np.abs(u - cx) - sx, np.abs(v - cy) - sy); return I * smooth(w, -w * 0.3, d)
    e = 0.16 - 0.10 * np.clip(v, -1, 1)
    e = e * np.where(rz < 0, 0.5, 1)
    e += box(-0.18, -0.22, 0.20, 0.14, 1.25) + box(0.45, -0.45, 0.18, 0.04, 0.55) + box(-0.55, 0.10, 0.04, 0.30, 0.35)
    return e
def shade(n, base, metal, rough, amb=0.30):
    base = np.asarray(base, float); nz = np.clip(n[..., 2:3], 0, 1)
    r = 2 * n[..., 2:3] * n - np.array([0, 0, 1.0])
    e = envmap(r, rough)[..., None]
    f0 = base if metal else np.array([0.045] * 3)
    fres = f0 + (1 - f0) * (1 - nz) ** 5
    d = np.clip((n * L).sum(2), 0, 1)[..., None]
    return (0 if metal else 1) * base * (amb + 0.75 * d) + fres * e

# ------------------------------------------------------------------ wear
def scratches(h, w, count, length=(20, 160), alpha=0.5, seed=0):
    """thin random scratches (0..1 mask)"""
    im = Image.new('L', (w, h), 0); d = ImageDraw.Draw(im); rng = np.random.default_rng(seed)
    for _ in range(count):
        x, y = rng.uniform(0, w), rng.uniform(0, h)
        a = rng.normal(-0.3, 0.6); ln = rng.uniform(*length) * S / 2
        pts = []
        for i in range(8):
            t = i / 7; aa = a + 0.04 * math.sin(t * 3 + x)
            pts.append((x + math.cos(aa) * ln * t, y + math.sin(aa) * ln * t))
        d.line(pts, fill=int(255 * rng.uniform(0.25, 1.0) * alpha), width=1)
    return np.asarray(im, np.float32) / 255
def smudges(h, w, amt=0.03):
    n = noise(h, w, 60 * S) * 0.7 + noise(h, w, 18 * S) * 0.3
    return 1 + amt * np.clip(n, -2, 2) / 2
def dust(h, w, count, seed=1):
    rng = np.random.default_rng(seed); m = np.zeros((h, w), np.float32)
    ys = rng.integers(0, h, count); xs = rng.integers(0, w, count); m[ys, xs] = rng.uniform(0.3, 1, count)
    return g(m, 0.7) * 3

# ------------------------------------------------------------------ knobs (2.5D: wall + cap)
def cyl_knob(N, R_bot, R_top, off, cap_fn, wall_base, wall_metal=False, wall_rough=0.35, ribs=0, rib_depth=0.25, skirt=None):
    yy, xx = np.mgrid[0:N, 0:N] + 0.5; cx = N / 2; cy = N / 2 + off / 2
    col = np.zeros((N, N, 3)); A = np.zeros((N, N))
    def slice_(t, rad, base, metal, rough, ribs_):
        nonlocal col, A
        yc = cy - off * t
        d = np.hypot(xx - cx, yy - yc); phi = np.arctan2(yy - yc, xx - cx)
        rr = rad * (1 + (rib_depth * 0.02 * np.cos(phi * ribs_) if ribs_ else 0))
        m = np.clip(rr - d + 0.5, 0, 1)
        nphi = phi + (np.sin(phi * ribs_) * rib_depth * 0.6 if ribs_ else 0)
        n = np.dstack([np.cos(nphi) * 0.85, np.sin(nphi) * 0.55, np.full_like(phi, 0.35)])
        n /= np.linalg.norm(n, axis=2, keepdims=True)
        c = (shade(n, base, metal, rough) + (np.array(base) * 0.22 if metal else 0)) * (0.75 + 0.25 * t)
        col = col * (1 - m[..., None]) + c * m[..., None]; A = np.maximum(A, m)
    t0 = 0
    if skirt:
        sR, sh, sb, sm = skirt; k = max(2, int(sh * 2))
        for i in range(k): slice_((i / (k - 1)) * sh / off, sR, sb, sm, 0.3, 0)
        yc = cy - sh; d = np.hypot(xx - cx, yy - yc); rn = d / sR; th = np.arctan2(yy - yc, xx - cx)
        m = np.clip(sR - d + 0.5, 0, 1)
        n = normals(g(np.where(rn < 1, 0.4 - 0.25 * rn, 0), 1) * N * 0.25)
        c = shade(n, sb, sm, 0.3)
        if sm:
            lobe = np.cos(th - math.atan2(L[1], L[0])) ** 2
            c = c * 0.8 + np.array(sb) * (0.35 * lobe ** 10 + 0.1)[..., None]
        col = col * (1 - m[..., None]) + c * m[..., None]; A = np.maximum(A, m); t0 = sh / off
    steps = int(off * 2) + 2
    for i in range(steps):
        t = t0 + (1 - t0) * i / (steps - 1)
        slice_(t, R_bot + (R_top - R_bot) * (t - t0) / (1 - t0 + 1e-9), wall_base, wall_metal, wall_rough, ribs)
    yc = cy - off; d = np.hypot(xx - cx, yy - yc); rn = d / R_top; th = np.arctan2(yy - yc, xx - cx)
    m = np.clip(R_top - d + 0.5, 0, 1)
    col = col * (1 - m[..., None]) + cap_fn(rn, th, R_top) * m[..., None]; A = np.maximum(A, m)
    # pad so the cap centre is the image centre
    sh_ = int(round(N - 2 * yc)); out = np.zeros((N + sh_, N, 3)); oa = np.zeros((N + sh_, N))
    out[sh_:] = col; oa[sh_:] = A
    return out, oa
def dome(rn, amt, edge):
    return np.where(rn < edge, np.sqrt(np.clip(1 - (rn * amt) ** 2, 0, 1)), np.sqrt(np.clip(1 - (edge * amt) ** 2, 0, 1)) - (rn - edge) * 2.5)
def cap_alu(base=(0.85, 0.85, 0.86), wear=0.6):
    def f(rn, th, R):
        n = normals(g(dome(rn, 0.15, 0.95), 0.8) * R * 0.4); c = shade(n, base, True, 0.3)
        lobe = np.cos(th - math.atan2(L[1], L[0])) ** 2
        rings = 0.035 * np.sin(rn * R * 1.8) + 0.02 * g(RNG.standard_normal(rn.shape), 0.4)
        out = c * 0.55 + np.array(base) * (0.62 * lobe ** 10 + 0.18 * lobe ** 2 + 0.18 + rings)[..., None]
        sc = scratches(rn.shape[0], rn.shape[1], int(30 * wear), (6, 40), 0.6, seed=int(R)) * (rn < 0.95)
        out = out * (1 - 0.12 * sc[..., None]) + 0.10 * sc[..., None]
        return out
    return f
def cap_plastic(color, rough=0.22, wear=0.5):
    def f(rn, th, R):
        n = normals(g(dome(rn, 0.55, 0.9), 0.8) * R * 0.4); c = shade(n, color, False, rough)
        sc = scratches(rn.shape[0], rn.shape[1], int(14 * wear), (4, 20), 0.5, seed=int(R) + 3)
        return c + 0.06 * sc[..., None] * (rn < 0.9)[..., None]
    return f
def cap_inset(color, R_in=0.66):
    outer = cap_plastic((0.03, 0.03, 0.033), 0.18)
    inner = cap_plastic(color, 0.2)
    def f(rn, th, R):
        rr = rn / R_in; m = np.clip((1 - rr) * R * R_in + 0.5, 0, 1)[..., None]
        return outer(rn, th, R) * (1 - m) + inner(rr, th, R * R_in) * m
    return f
def knob_image(kind, D, cap=None):
    """kind: 'alu' (turned aluminium), 'cap' (black plastic with coloured cap), 'red' (red cap on alu skirt)"""
    D = int(D)
    if kind == 'alu':
        N = int(D * 1.25); R = D * 0.42; off = D * 0.10
        col, A = cyl_knob(N, R, R * 0.98, off, cap_alu(), [0.75, 0.75, 0.77], True, 0.35, ribs=110, rib_depth=0.35)
        ptr = dict(r0=R * 0.30, r1=R * 0.82, w=max(1.6, D * 0.035 / S), col=(30, 30, 32, 235), kind='engraved')
    elif kind == 'cap':
        N = int(D * 1.3); R = D * 0.42; off = D * 0.13
        col, A = cyl_knob(N, R, R * 0.9, off, cap_inset(cap), [0.09, 0.095, 0.1], False, 0.3, ribs=28, rib_depth=0.6)
        ptr = dict(r0=R * 0.12, r1=R * 0.86, w=max(1.6, D * 0.04 / S), col=(250, 250, 248, 255), kind='paint')
    elif kind == 'red':
        N = int(D * 1.25); R = D * 0.42; off = D * 0.12
        col, A = cyl_knob(N, R * 0.62, R * 0.58, off, cap_plastic([0.62, 0.05, 0.035], 0.2), [0.5, 0.04, 0.03], False, 0.25, ribs=8, rib_depth=1.2,
                          skirt=(R, off * 0.35, [0.8, 0.81, 0.83], True))
        ptr = dict(r0=R * 0.12, r1=R * 0.55, w=max(1.6, D * 0.03 / S), col=(250, 245, 240, 255), kind='paint', skirt=(R * 0.72, R * 0.97))
    return to_img(col, A), ptr

# ------------------------------------------------------------------ buttons / screws / glass
def button_image(w, h, on, accent, label, body=(0.30, 0.30, 0.32), wear=0.6):
    pad = 12 * S; W_, H_ = w + 2 * pad, h + 2 * pad
    yy, xx = np.mgrid[0:h, 0:w] + 0.5; r = 4 * S
    dx = np.maximum(np.maximum(r - xx, xx - (w - r)), 0); dy = np.maximum(np.maximum(r - yy, yy - (h - r)), 0)
    a = np.clip(r - np.hypot(dx, dy) + 0.5, 0, 1); e = np.minimum.reduce([xx, yy, w - xx, h - yy])
    hm = np.clip(e / (4 * S), 0, 1) * 4 * S
    acc = np.array(accent) / 255
    base = acc * 0.95 + 0.05 if on else np.array(body)
    n = normals(g(hm, 1)); col = shade(n, base, False, 0.3, amb=0.65)
    if on: col = col * 0.7 + acc * 0.45 * np.exp(-(((xx - w / 2) / (w * 0.6)) ** 2 + ((yy - h / 2) / (h * 0.6)) ** 2))[..., None]
    # worn edges: lighter rubbed corners, grime near the bottom
    wearm = np.clip(noise(h, w, 2) * 0.5 + 0.5, 0, 1) * np.exp(-e / (2.5 * S))
    col += 0.10 * wear * wearm[..., None]
    col *= (1 - 0.06 * wear * smooth(0.6, 1.0, yy / h))[..., None]
    im = to_img(col, a); d = ImageDraw.Draw(im)
    fs = min(h * 0.40, 9.5 * S)
    tc = (25, 22, 18, 235) if on else (210, 210, 205, 225)
    d.text((w / 2, h / 2 + S * 0.5), label, font=font(fs, True), fill=tc, anchor='mm')
    out = Image.new('RGBA', (W_, H_))
    if on:
        gl = np.zeros((H_, W_)); gl[pad:pad + h, pad:pad + w] = a
        out.alpha_composite(to_img(np.zeros((H_, W_, 3)) + acc, g(gl, 5 * S) * 0.75))
    out.alpha_composite(im, (pad, pad)); return out
def hexscrew(D):
    N = int(D); a, d, th = disc(N, N * 0.45); rn = d / (N * 0.45)
    h = np.where(rn < 0.9, 0.3, 0.3 - (rn - 0.9) * 3)
    hexa = np.cos(np.pi / 6) / np.cos(((th) % (np.pi / 3)) - np.pi / 6)
    h = h - (rn < 0.38 * hexa) * 0.25
    c = shade(normals(g(h, 0.6) * N * 0.4), [0.55, 0.56, 0.58], True, 0.4)
    return to_img(c, a)
def glass(w, h, r=6):
    yy, xx = np.mgrid[0:h, 0:w]
    R = r * S; dx = np.maximum(np.maximum(R - xx, xx - (w - R)), 0); dy = np.maximum(np.maximum(R - yy, yy - (h - R)), 0)
    a0 = np.clip(R - np.hypot(dx, dy) + 0.5, 0, 1)
    band = np.exp(-(((xx * 0.55 + yy) - h * 0.28) / (h * 0.16)) ** 2) * 0.03
    top = 0.0 * np.clip(1 - yy / (h * 0.5), 0, 1)
    sm = 0
    dd = dust(h, w, int(w * h / (6000 * S * S)), 7) * 0.15
    alpha = np.clip(dd, 0, 0.5) * a0
    # inner shadow at the rim (the glass sits behind a bezel)
    e = np.minimum.reduce([xx, yy, w - xx, h - yy]).astype(float)
    rim = np.exp(-e / (5 * S)) * 0.55
    col = np.ones((h, w, 3)); col = np.where((rim > alpha)[..., None], 0, col)
    return to_img(col, np.maximum(alpha, rim) * a0)
def layered_shadow(alpha, strength=1.0):
    pad = int(48 * S); A = np.zeros((alpha.shape[0] + 2 * pad, alpha.shape[1] + 2 * pad)); A[pad:-pad, pad:-pad] = alpha
    sh = lambda dx, dy, b, o: g(np.roll(np.roll(A, int(dy), 0), int(dx), 1), b, 'constant') * o * strength
    s = 1 - (1 - sh(0.5 * S, 1 * S, 1.2 * S, 0.75)) * (1 - sh(3 * S, 6 * S, 4 * S, 0.40)) * (1 - sh(9 * S, 16 * S, 12 * S, 0.30))
    return Image.fromarray((np.dstack([np.zeros(A.shape + (3,)), np.clip(s, 0, 1)]) * 255).astype(np.uint8), 'RGBA')

# ------------------------------------------------------------------ the panel builder
class Panel:
    def __init__(self, W, H, base, accent, ink, dim, bg=(0.06, 0.06, 0.065), wear=1.0, light_panel=False):
        self.W, self.H = W, H; self.w, self.h = W * S, H * S
        self.accent, self.ink, self.dim = accent, ink, dim; self.wear = wear; self.light = light_panel
        self.knobs = []; self.buttons = []; self.screens = []; self.vus = []; self.ladders = []; self.texts = []; self.overlays = {}
        h, w = self.h, self.w
        t = noise(h, w, 0.5) * 0.5 + noise(h, w, 1.5) * 0.3
        y, x = np.mgrid[0:h, 0:w]
        c = np.array(base)[None, None, :] * (1 + 0.025 * t[..., None])
        c *= (1.06 - 0.12 * (y / h)[..., None]); c += 0.025 * np.exp(-((y / h - 0.1) / 0.25) ** 2)[..., None]
        c *= smudges(h, w, 0.035 * wear)[..., None]
        # scratches: bright on dark anodising, dark on light
        sc = scratches(h, w, int(140 * wear), (10, 90), 0.38, seed=11)
        c = c + (0.18 if not light_panel else -0.10) * sc[..., None]
        c += (0.10 if not light_panel else -0.05) * dust(h, w, int(900 * wear), 3)[..., None]
        R = 18 * S; xx, yy = x + 0.5, y + 0.5
        dx = np.maximum(np.maximum(R - xx, xx - (w - R)), 0); dy = np.maximum(np.maximum(R - yy, yy - (h - R)), 0)
        d = np.hypot(dx, dy); A = np.clip(R - d + 0.5, 0, 1); e = R - d
        ch = np.clip(1 - e / (5 * S), 0, 1) * (e > 0)
        lit = np.clip(-(xx - w / 2) / w - (yy - h / 2) / h, -1, 1)
        c += (ch * 0.22 * np.sign(lit) * np.minimum(np.abs(lit) * 3, 1))[..., None]
        # edge wear: bare aluminium shows through on the chamfer
        ew = (np.clip(noise(h, w, 1.5) * 0.6 + noise(h, w, 8) * 0.5 - 0.9, 0, 1) * ch * wear * 0.6 * (e > 1.5 * S))
        c = c * (1 - ew[..., None]) + np.array([0.78, 0.78, 0.8]) * ew[..., None]
        self.img = to_img(c * A[..., None] + np.array(bg) * (1 - A[..., None])).convert('RGBA')
        self.dr = ImageDraw.Draw(self.img)
        self.K = 2
        self.ink_layer = Image.new('RGBA', (w * self.K, h * self.K)); self.idr = ImageDraw.Draw(self.ink_layer)
        for sx, sy in [(18, 18), (W - 18, 18), (18, H - 18), (W - 18, H - 18)]: self.put(hexscrew(14 * S), sx * S, sy * S)
    # ---- low level
    def refresh(self): self.dr = ImageDraw.Draw(self.img)
    def put(self, im, cx, cy, shadow=1.0):
        if shadow:
            a = np.array(im)[..., 3] / 255; sh = layered_shadow(a, shadow)
            self.img.alpha_composite(sh, (int(cx - sh.width / 2), int(cy - sh.height / 2)))
        self.img.alpha_composite(im, (int(cx - im.width / 2), int(cy - im.height / 2)))
    def text(self, t, x, y, size, col=None, bold=False, anchor='mm', sp=0):
        col = col or self.ink; Q = S * self.K; f = font(size * Q, bold); d = self.idr; x *= Q; y *= Q
        if sp:
            wdt = sum(d.textlength(ch, font=f) for ch in t) + sp * Q * (len(t) - 1)
            xx = x - wdt / 2 if anchor[0] == 'm' else (x - wdt if anchor[0] == 'r' else x)
            for ch in t: d.text((xx, y), ch, font=f, fill=col, anchor='l' + anchor[1]); xx += d.textlength(ch, font=f) + sp * Q
        else: d.text((x, y), t, font=f, fill=col, anchor=anchor)
    def flush_ink(self):
        """silkscreen: slightly worn, with a hairline of shadow so it sits on the paint"""
        big = np.array(self.ink_layer)
        if big[..., 3].max() == 0: return
        # premultiplied downsampling, so edges keep their colour
        bf = big.astype(np.float32) / 255; bf[..., :3] *= bf[..., 3:4]
        pm = to_img(np.clip(bf[..., :3], 0, 1), bf[..., 3]).resize((self.w, self.h), Image.LANCZOS)
        ink = np.array(pm).astype(np.float32) / 255
        ink[..., :3] = ink[..., :3] / np.maximum(ink[..., 3:4], 1e-4)
        wearm = np.clip(1 - self.wear * 0.25 * np.clip(noise(self.h, self.w, 1.0) * 0.6 + noise(self.h, self.w, 6) * 0.4 - 0.3, 0, 1), 0, 1)
        ink[..., 3] *= wearm
        self.img.alpha_composite(to_img(ink[..., :3], ink[..., 3]))
        self.ink_layer = Image.new('RGBA', (self.w * self.K, self.h * self.K)); self.idr = ImageDraw.Draw(self.ink_layer); self.refresh()
    def line(self, pts, col, width):
        Q = S * self.K
        self.idr.line([(x * Q, y * Q) for x, y in pts], fill=col, width=max(1, int(round(width * Q))))
    def inset(self, x0, y0, x1, y1, fill=(0.085, 0.085, 0.095), r=10):
        self.flush_ink()
        X0, Y0, X1, Y1 = x0 * S, y0 * S, x1 * S, y1 * S
        h, w = self.h, self.w; yy, xx = np.mgrid[0:h, 0:w] + 0.5; R = r * S
        cx = np.clip(xx, X0 + R, X1 - R); cy = np.clip(yy, Y0 + R, Y1 - R)
        d = np.hypot(xx - cx, yy - cy); m = np.clip(R - d + 0.5, 0, 1)
        e = R - d
        f = np.array(fill)[None, None, :] * (1 + 0.03 * noise(h, w, 0.6)[..., None]) * smudges(h, w, 0.03)[..., None]
        top = np.exp(-np.clip(yy - Y0, 0, None) / (5 * S)); lef = np.exp(-np.clip(xx - X0, 0, None) / (5 * S))
        bot = np.exp(-np.clip(Y1 - yy, 0, None) / (1.2 * S)); rig = np.exp(-np.clip(X1 - xx, 0, None) / (1.2 * S))
        f = f * (1 - 0.6 * np.maximum(top, lef)[..., None]) + 0.10 * np.maximum(bot, rig)[..., None]
        arr = np.array(self.img).astype(float) / 255
        arr[..., :3] = arr[..., :3] * (1 - m[..., None]) + f * m[..., None]
        self.img = to_img(arr[..., :3], arr[..., 3]); self.refresh()
    def recess_frame(self, x0, y0, x1, y1, r=6):
        """dark bezel around a screen / meter, slightly raised"""
        self.flush_ink()
        w, h = int((x1 - x0) * S) + 8 * S, int((y1 - y0) * S) + 8 * S
        yy, xx = np.mgrid[0:h, 0:w] + 0.5; R = (r + 4) * S
        dx = np.maximum(np.maximum(R - xx, xx - (w - R)), 0); dy = np.maximum(np.maximum(R - yy, yy - (h - R)), 0)
        d = np.hypot(dx, dy); a = np.clip(R - d + 0.5, 0, 1); e = R - d
        hm = np.clip(e / (4 * S), 0, 1) * 4 * S
        c = shade(normals(g(hm, 1)), [0.035, 0.035, 0.04], False, 0.4, amb=0.6)
        im = to_img(c, a); self.put(im, (x0 + x1) / 2 * S, (y0 + y1) / 2 * S, 0.8)
    # ---- controls
    def scale(self, cx, cy, r, n=11, major=None, labels=None, lsz=8, col=None, tick=6):
        col = col or self.dim
        for i in range(n):
            q = math.radians(-135 + 270 * i / (n - 1)); big = (major is None) or (i % major == 0)
            r1 = r + (tick if big else tick * 0.5)
            self.line([(cx + r * math.sin(q), cy - r * math.cos(q)), (cx + r1 * math.sin(q), cy - r1 * math.cos(q))], col, 1.6 if big else 1.0)
        if labels:
            for i, t in enumerate(labels):
                q = math.radians(-135 + 270 * i / (len(labels) - 1))
                self.text(t, cx + (r + tick + 9) * math.sin(q), cy - (r + tick + 9) * math.cos(q), lsz, col)
    def knob(self, id, cx, cy, D, kind='alu', cap=None, label=None, label_dy=None, scale=None, led=None, bip=False, lsz=7, lcol=None):
        """D: knob diameter (logical). scale: dict(r, n, major, labels). led: dict(r, n) LED ring (off dots baked)"""
        if scale: self.scale(cx, cy, **scale)
        if led:
            for i in range(led['n']):
                q = math.radians(-135 + 270 * i / (led['n'] - 1)); x, y = cx + led['r'] * math.sin(q), cy - led['r'] * math.cos(q)
                Q = S * self.K
                # an unlit LED: dark lens with a tiny specular dot
                self.idr.ellipse([(x - 2.0) * Q, (y - 2.0) * Q, (x + 2.0) * Q, (y + 2.0) * Q], fill=(14, 14, 16, 255) if not self.light else (110, 114, 120, 255))
                self.idr.ellipse([(x - 1.6) * Q, (y - 1.6) * Q, (x + 1.6) * Q, (y + 1.6) * Q], fill=(30, 30, 33, 255) if not self.light else (70, 74, 80, 255))
                self.idr.ellipse([(x - 1.0) * Q, (y - 1.2) * Q, (x + 0.2) * Q, (y - 0.3) * Q], fill=(110, 110, 115, 150))
        self.flush_ink()
        im, ptr = knob_image(kind, D * S, cap)
        self.put(im, cx * S, cy * S)
        if label: self.text(label, cx, cy + (label_dy if label_dy is not None else D * 0.5 + 14), lsz, lcol or self.dim, True, sp=1)
        self.knobs.append(dict(id=id, c=(cx, cy), r=D * 0.5, ptr=dict(r0=ptr['r0'] / S, r1=ptr['r1'] / S, w=ptr['w'], col=ptr['col'], kind=ptr['kind'], skirt=[v / S for v in ptr.get('skirt', (0, 0))]),
                               led=(led['r'], led['n']) if led else (0, 0), bip=bip))
    def button(self, id, cx, cy, w, h, label, accent=None, body=(0.30, 0.30, 0.32)):
        accent = accent or self.accent
        self.flush_ink()
        off = button_image(int(w * S), int(h * S), False, accent, label, body, self.wear)
        on = button_image(int(w * S), int(h * S), True, accent, label, body, self.wear)
        # shadow + off state into the background, lit state as an overlay
        a = np.array(off)[..., 3] / 255; sh = layered_shadow(a, 0.8)
        self.img.alpha_composite(sh, (int(cx * S - sh.width / 2), int(cy * S - sh.height / 2)))
        self.img.alpha_composite(off, (int(cx * S - off.width / 2), int(cy * S - off.height / 2)))
        name = 'btn_' + id; self.overlays[name] = on
        self.buttons.append(dict(id=id, rect=(cx - w / 2, cy - h / 2, w, h), tex=name, tex_size=(on.width / S, on.height / S)))
    def screen(self, id, x, y, w, h, fill=(0.012, 0.013, 0.015)):
        self.recess_frame(x, y, x + w, y + h)
        arr = np.array(self.img).astype(float) / 255
        X, Y, W_, H_ = int(x * S), int(y * S), int(w * S), int(h * S)
        arr[Y:Y + H_, X:X + W_, :3] = np.array(fill)
        self.img = to_img(arr[..., :3], arr[..., 3]); self.refresh()
        name = 'glass_' + id; self.overlays[name] = glass(W_, H_)
        self.screens.append(dict(id=id, rect=(x, y, w, h), glass=name))
    def vu(self, id, x, y, w, h, accent=None, label='', kind='vu'):
        """modern VU: black face, warm backlight, white scale; needle and glass drawn by the GUI"""
        accent = accent or self.accent
        self.recess_frame(x, y, x + w, y + h, 8)
        W_, H_ = int(w * S), int(h * S)
        yy, xx = np.mgrid[0:H_, 0:W_] + 0.5
        glow = np.exp(-(((xx - W_ / 2) / (W_ * 0.55)) ** 2 + ((yy - H_ * 1.05) / (H_ * 0.75)) ** 2))
        face = np.array([0.02, 0.018, 0.016]) + np.array(accent) / 255 * 0.30 * glow[..., None]
        face *= (1 + 0.02 * noise(H_, W_, 0.7))[..., None]
        im = to_img(face).convert('RGBA'); vd = ImageDraw.Draw(im)
        Rr = min(H_ * 0.78, W_ * 0.36); fcx, fcy = W_ / 2, H_ * 0.30 + Rr
        ang = lambda db: -48 + 96 * (10 ** (db / 20) - 0.1) / (10 ** (3 / 20) - 0.1)
        wcol = (235, 232, 224, 255); acc = tuple(accent) + (255,)
        arcp = lambda a0, a1, r: [(fcx + r * math.sin(math.radians(q)), fcy - r * math.cos(math.radians(q))) for q in np.linspace(a0, a1, 60)]
        vd.line(arcp(-48, ang(0), Rr), fill=wcol, width=int(1.4 * S)); vd.line(arcp(ang(0), 48, Rr), fill=acc, width=int(3 * S))
        small = W_ < 200 * S
        for t, db in [('20', -20), ('10', -10), ('7', -7), ('5', -5), ('3', -3), ('2', -2), ('1', -1), ('0', 0), ('+1', 1), ('+2', 2), ('+3', 3)]:
            q = math.radians(ang(db)); cc = acc if db > 0 else wcol
            vd.line([(fcx + Rr * math.sin(q), fcy - Rr * math.cos(q)), (fcx + (Rr + 9 * S) * math.sin(q), fcy - (Rr + 9 * S) * math.cos(q))], fill=cc, width=int(1.6 * S))
            if small and db in (-7, -2, -1, 1, 2): continue
            vd.text((fcx + (Rr + 20 * S) * math.sin(q), fcy - (Rr + 20 * S) * math.cos(q)), t, font=font(9 * S), fill=cc, anchor='mm')
        if label: vd.text((fcx, H_ * 0.86), label, font=font(7 * S, True), fill=(150, 145, 135, 255), anchor='mm')
        im = im.filter(ImageFilter.GaussianBlur(0.35))
        self.img.alpha_composite(im, (int(x * S), int(y * S))); self.refresh()
        name = 'glass_' + id; self.overlays[name] = glass(W_, H_, 6)
        self.vus.append(dict(id=id, rect=(x, y, w, h), pivot=(x + fcx / S, y + fcy / S), r=Rr / S, glass=name))
    def ladder(self, id, x, y, w, h, n, colors):
        """LED bar: segments baked dark, the GUI lights them. colors: list of (from_fraction, rgb)"""
        self.flush_ink()
        self.recess_frame(x, y, x + w, y + h, 3)
        seg = h / n
        for k in range(n):
            y1 = y + h - k * seg - 1.2; t = k / n
            c = [cc for f, cc in colors if t >= f][-1]
            self.dr.rounded_rectangle([(x + 2) * S, (y1 - seg + 2.2) * S, (x + w - 2) * S, y1 * S], radius=S, fill=tuple(int(v * 0.14) for v in c) + (255,))
        self.ladders.append(dict(id=id, rect=(x, y, w, h), n=n, colors=colors))
    def readout(self, id, x, y, anchor='mm', size=9, col=None):
        self.texts.append(dict(id=id, pos=(x, y), anchor=anchor, size=size, col=col or self.ink))
    # ---- export
    def export(self, repo, plugin_dir):
        self.flush_ink()
        assets = os.path.join(repo, plugin_dir, 'assets', 'hw'); os.makedirs(assets, exist_ok=True)
        self.img.convert('RGB').save(os.path.join(assets, 'bg.jpg'), quality=90, optimize=True, subsampling=0)
        for name, im in self.overlays.items(): im.save(os.path.join(assets, name + '.png'), optimize=True)
        a, d, _ = disc(32 * S, 1); glow = 0.65 * np.exp(-(d / (3.5 * S)) ** 2) + 0.35 * np.exp(-(d / (8 * S)) ** 2); glow *= np.clip((16 * S - d) / (3 * S), 0, 1)
        to_img(np.ones((32 * S, 32 * S, 3)), glow / glow.max()).save(os.path.join(assets, 'glow.png'))
        c = lambda t: '[' + ','.join(str(int(v)) for v in t) + ']'
        f = lambda v: f'{float(v):.2f}'
        out = ['//! Generated by tools/render_ui.py: positions of the controls on the rendered faceplate.', '#![allow(dead_code)]',
               'use crate::hw::*;', f'pub const W: f32 = {f(self.W)};', f'pub const H: f32 = {f(self.H)};', '']
        out.append('pub const ASSETS: &[(&str, &[u8])] = &[')
        out.append(f'    ("bg", include_bytes!("../assets/hw/bg.jpg")),')
        out.append(f'    ("glow", include_bytes!("../assets/hw/glow.png")),')
        for name in self.overlays: out.append(f'    ("{name}", include_bytes!("../assets/hw/{name}.png")),')
        out.append('];\n')
        out.append(f'pub const ACCENT: [u8; 3] = {c(self.accent)};')
        out.append('pub const KNOBS: &[KnobL] = &[')
        for k in self.knobs:
            p = k['ptr']
            out.append(f'    KnobL {{ id: "{k["id"]}", c: ({f(k["c"][0])}, {f(k["c"][1])}), r: {f(k["r"])}, ptr0: {f(p["r0"])}, ptr1: {f(p["r1"])}, ptr_w: {f(p["w"])}, ptr_col: {c(p["col"])}, engraved: {str(p["kind"]=="engraved").lower()}, skirt: ({f(p["skirt"][0])}, {f(p["skirt"][1])}), led_r: {f(k["led"][0])}, led_n: {k["led"][1]}, bip: {str(k["bip"]).lower()} }},')
        out.append('];\npub const BUTTONS: &[BtnL] = &[')
        for b in self.buttons:
            r = b['rect']; out.append(f'    BtnL {{ id: "{b["id"]}", rect: ({f(r[0])}, {f(r[1])}, {f(r[2])}, {f(r[3])}), tex: "{b["tex"]}", tex_size: ({f(b["tex_size"][0])}, {f(b["tex_size"][1])}) }},')
        out.append('];\npub const SCREENS: &[ScreenL] = &[')
        for s in self.screens:
            r = s['rect']; out.append(f'    ScreenL {{ id: "{s["id"]}", rect: ({f(r[0])}, {f(r[1])}, {f(r[2])}, {f(r[3])}), glass: "{s["glass"]}" }},')
        out.append('];\npub const VUS: &[VuL] = &[')
        for v in self.vus:
            r = v['rect']; out.append(f'    VuL {{ id: "{v["id"]}", rect: ({f(r[0])}, {f(r[1])}, {f(r[2])}, {f(r[3])}), pivot: ({f(v["pivot"][0])}, {f(v["pivot"][1])}), r: {f(v["r"])}, glass: "{v["glass"]}" }},')
        out.append('];\npub const LADDERS: &[LadderL] = &[')
        for l in self.ladders:
            r = l['rect']; cols = ', '.join(f'({f(fr)}, {c(cc)})' for fr, cc in l['colors'])
            out.append(f'    LadderL {{ id: "{l["id"]}", rect: ({f(r[0])}, {f(r[1])}, {f(r[2])}, {f(r[3])}), n: {l["n"]}, colors: &[{cols}] }},')
        out.append('];\npub const TEXTS: &[TextL] = &[')
        for t in self.texts:
            out.append(f'    TextL {{ id: "{t["id"]}", pos: ({f(t["pos"][0])}, {f(t["pos"][1])}), anchor: "{t["anchor"]}", size: {f(t["size"])}, col: {c(t["col"][:3])} }},')
        out.append('];')
        open(os.path.join(repo, plugin_dir, 'src', 'hw_layout.rs'), 'w').write('\n'.join(out) + '\n')
        self.img.save(os.path.join('/tmp/hw', os.path.basename(plugin_dir) + '_bg_preview.png'))

def header(p, name, sub, x0=40, y=32):
    p.text('NYASHK', x0, y, 14, p.ink, True, 'lm', sp=2.2)
    wdt = sum(p.idr.textlength(ch, font=font(14 * S, True)) for ch in 'NYASHK') / S + 2.2 * 5
    p.text(name, x0 + wdt + 12, y, 14, tuple(p.accent) + (255,), False, 'lm', sp=2.2)
    if sub: p.text(sub, p.W / 2 + 20, y + 1, 8, p.dim, False, sp=2.4)
    p.line([(x0, y + 22), (p.W - x0, y + 22)], (255, 255, 255, 26 if not p.light else 60), 1)
    p.line([(x0, y + 23), (p.W - x0, y + 23)], (0, 0, 0, 80), 1)
