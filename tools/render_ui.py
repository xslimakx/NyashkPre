#!/usr/bin/env python3
"""Render NyashkPre's faceplate (see hwrender.py). Run from the repo root."""
import os, sys
sys.path.insert(0, os.path.dirname(__file__))
from hwrender import *

ACC = (240, 74, 58)
p = Panel(700, 450, (0.17, 0.22, 0.29), ACC, (236, 238, 240, 235), (160, 170, 182, 225))
header(p, 'PRE', 'CLASS-A VOCAL PREAMP', x0=36)
p.button('power', 628, 32, 64, 22, 'POWER')
p.button('phase', 568, 32, 36, 22, 'Ø')
p.knob('gain', 106, 152, 100, 'red', label='GAIN', label_dy=88, lsz=9.5, lcol=p.ink,
       scale=dict(r=66, n=37, major=6, labels=['0', '6', '12', '18', '24', '30', '36'], lsz=8), led=dict(r=58, n=31))
p.screen('sat', 212, 78, 150, 150)
p.text('SATURATION', 287, 240, 6.5, None, True, sp=1)
p.screen('tone', 378, 78, 222, 150)
p.text('TONE  ·  SPECTRUM', 489, 240, 6.5, None, True, sp=1)
lad = [(0.0, (60, 160, 255)), (0.75, (255, 190, 50)), (0.9, (255, 70, 60))]
p.ladder('in', 614, 78, 16, 150, 24, lad); p.ladder('out', 644, 78, 16, 150, 24, lad)
p.text('IN', 622, 240, 6.5, None, True); p.text('OUT', 652, 240, 6.5, None, True)
p.inset(22, 262, 678, 432)
kn = dict(scale=dict(r=27, n=11, tick=4), led=dict(r=22, n=15))
p.knob('drive', 66, 340, 44, 'alu', label='DRIVE', label_dy=44, **kn)
p.text('CHARACTER', 146, 294, 6.5, None, True, sp=1)
for i, (id, t) in enumerate([('tube', 'TUBE'), ('iron', 'IRON'), ('clean', 'CLEAN')]):
    p.button(id, 146, 316 + i * 26, 58, 20, t)
for i, (id, lbl, bip) in enumerate([('lowcut', 'LOW CUT', False), ('warmth', 'WARMTH', True), ('presence', 'PRESENCE', True), ('silk', 'SILK', True)]):
    p.knob(id, 226 + i * 66, 340, 44, 'alu', label=lbl, label_dy=44, bip=bip, **kn)
p.knob('mix', 500, 340, 44, 'alu', label='MIX', label_dy=44, **kn)
p.knob('out', 566, 340, 44, 'alu', label='OUTPUT', label_dy=44, bip=True, **kn)
p.button('auto', 636, 340, 60, 22, 'AUTO LVL')
p.export(os.path.join(os.path.dirname(__file__), '..'), 'plugins/nyashkpre')
