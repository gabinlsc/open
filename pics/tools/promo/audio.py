"""Bande-son synthétisée (aucun échantillon externe, donc aucun droit à gérer) : nappe, arpèges, souffles de transition.
Usage : python3 audio.py sortie.wav"""
import sys
import wave

import numpy as np

SR = 44100
DUR = 48.0
STARTS = [0, 4.0, 8.0, 12.5, 18.0, 24.0, 30.0, 34.0, 39.5, 43.5]
rng = np.random.default_rng(3)
n = int(SR * DUR)
t = np.arange(n) / SR
L = np.zeros(n)
R = np.zeros(n)

def hz(m):
    return 440.0 * 2 ** ((m - 69) / 12)

def env(length, a, r):
    e = np.ones(length)
    ai, ri = int(a * SR), int(r * SR)
    if ai:
        e[:ai] = np.linspace(0, 1, ai) ** 2
    if ri:
        e[-ri:] *= np.linspace(1, 0, ri) ** 2
    return e

def add(buf, start, sig, gain=1.0, pan=0.0):
    i = int(start * SR)
    if i >= n:
        return
    sig = sig[: n - i]
    l, r = np.cos((pan + 1) * np.pi / 4), np.sin((pan + 1) * np.pi / 4)
    L[i : i + len(sig)] += sig * gain * l
    R[i : i + len(sig)] += sig * gain * r

# Progression (4 s par accord) : La mineur add9 – Fa – Do – Sol
CHORDS = [[57, 60, 64, 71], [53, 57, 60, 67], [48, 55, 60, 64], [55, 59, 62, 69]]
for k in range(12):
    chord = CHORDS[k % 4]
    start = k * 4.0
    ln = int((4.0 + 1.2) * SR)
    tt = np.arange(ln) / SR
    sig = np.zeros(ln)
    for j, m in enumerate(chord):
        for det in (-0.12, 0.12):  # désaccord léger = chaleur
            f = hz(m) * 2 ** (det / 12)
            sig += np.sin(2 * np.pi * f * tt) * 0.5 + np.sin(2 * np.pi * 2 * f * tt) * 0.12
    sig *= env(ln, 1.4, 1.8) / (len(chord) * 2)
    level = 0.0 if start < 1.5 else min(1.0, (start + 2) / 8)
    add(L, start, sig, 0.55 * max(level, 0.35) if start > 0 else 0.25, -0.1)
    add(R, start, sig, 0.0, 0.1)

# Basse douce
for k in range(12):
    f = hz(CHORDS[k % 4][0] - 12)
    ln = int(4.4 * SR)
    tt = np.arange(ln) / SR
    sig = np.sin(2 * np.pi * f * tt) * env(ln, 0.6, 1.2)
    add(L, k * 4.0, sig, 0.22 if k >= 1 else 0.08)

# Arpèges (croches à 100 BPM) à partir de la scène 2 ; plus présents à partir de la scène 4.
beat = 60 / 100 / 2
step = 0
tcur = 4.0
while tcur < 43.0:
    chord = CHORDS[int(tcur // 4) % 4]
    m = chord[step % 4] + 12 + (12 if (step // 4) % 2 else 0)
    ln = int(1.1 * SR)
    tt = np.arange(ln) / SR
    f = hz(m)
    pluck = (np.sin(2 * np.pi * f * tt) + 0.3 * np.sin(2 * np.pi * 3 * f * tt)) * np.exp(-tt * 5.5)
    vol = 0.07 if tcur < 12.5 else 0.11
    if tcur > 39.5:
        vol *= 0.7
    add(L, tcur, pluck, vol, -0.5 + 0.2 * (step % 5))
    add(R, tcur, pluck, 0.0, 0.5 - 0.2 * (step % 5))
    tcur += beat
    step += 1

# Souffles de transition (bruit filtré qui monte) à chaque changement de scène.
for s in STARTS[1:]:
    ln = int(0.9 * SR)
    noise = rng.standard_normal(ln)
    k = np.linspace(0.02, 0.5, ln)
    out = np.zeros(ln)
    y = 0.0
    for i in range(ln):  # passe-bas dont la fréquence de coupure monte
        y += k[i] * (noise[i] - y)
        out[i] = y
    out *= np.sin(np.linspace(0, np.pi, ln)) ** 2
    add(L, s - 0.25, out, 0.5, -0.3)
    add(R, s - 0.25, out, 0.0, 0.3)

# Impact d'ouverture et carillon final.
ln = int(1.6 * SR)
tt = np.arange(ln) / SR
add(L, 0.25, np.sin(2 * np.pi * 55 * tt) * np.exp(-tt * 3.2), 0.5)
for j, m in enumerate([81, 88, 93]):
    ln = int(2.6 * SR)
    tt = np.arange(ln) / SR
    f = hz(m)
    add(L, 43.9 + j * 0.12, np.sin(2 * np.pi * f * tt) * np.exp(-tt * 1.8), 0.09, -0.3 + 0.3 * j)

# Normalisation douce et fondu final.
peak = max(np.abs(L).max(), np.abs(R).max())
L, R = L / peak * 0.85, R / peak * 0.85
fade = np.clip((DUR - t) / 1.2, 0, 1)
L, R = L * fade, R * fade
data = (np.stack([L, R], axis=1) * 32767).astype("<i2")
with wave.open(sys.argv[1], "wb") as w:
    w.setnchannels(2)
    w.setsampwidth(2)
    w.setframerate(SR)
    w.writeframes(data.tobytes())
print("ok")
