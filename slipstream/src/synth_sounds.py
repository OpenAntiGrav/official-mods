#!/usr/bin/env python3
"""Synthesises Slipstream's three sounds: sounds/portal-open.wav, sounds/transit.wav and
sounds/portal-exit.wav. Standard library only; the noise is seeded, so a run reproduces the
files byte for byte. Licence: MIT OR Apache-2.0, like the rest of the mod.

Mono, 16-bit, 22,050 Hz. The engine resamples to its mixer's rate.

    python3 src/synth_sounds.py            (from the mod's directory)

- portal-open: a band of noise sweeping up from 300 to 3,000 Hz over a rising sine, with a
  fast attack and a long soft decay. 0.7 s.
- transit: a held hum, three sine partials at whole numbers of cycles per second so the
  loop has no seam, with a slow tremolo. 1 s, played as a loop and stopped at the exit.
- portal-exit: the entry's sweep run downwards and softer. 0.5 s.

Peaks are set so each sits below the quietest of the title's own effects; the numbers are
printed on every run.
"""

import math
import random
import struct
import sys
import wave
from pathlib import Path

RATE = 22_050
SEED = 0x5117


def band_noise(seconds, centre_at, rng):
    """White noise through a state-variable band-pass whose centre follows `centre_at(t)`."""
    low = band = 0.0
    out = []
    for n in range(int(seconds * RATE)):
        t = n / RATE
        f = 2.0 * math.sin(math.pi * centre_at(t) / RATE)
        q = 0.35
        high = rng.uniform(-1.0, 1.0) - low - q * band
        band += f * high
        low += f * band
        out.append(band)
    return out


def sweep_sine(seconds, freq_at):
    """A sine whose frequency follows `freq_at(t)`, phase carried so it never jumps."""
    phase = 0.0
    out = []
    for n in range(int(seconds * RATE)):
        phase += 2.0 * math.pi * freq_at(n / RATE) / RATE
        out.append(math.sin(phase))
    return out


def envelope(n, total, attack, release):
    """Linear attack, exponential-ish decay to zero at `total`; no click at either end."""
    t = n / RATE
    length = total / RATE
    rise = min(1.0, t / attack)
    fall = max(0.0, 1.0 - t / length) ** 2.0
    tail = min(1.0, (length - t) / release)
    return rise * fall * max(0.0, tail)


def normalise(samples, peak):
    top = max(abs(s) for s in samples) or 1.0
    return [s * peak / top for s in samples]


def portal_open(rng):
    seconds = 0.7
    noise = band_noise(seconds, lambda t: 300.0 + 2700.0 * (t / seconds) ** 2, rng)
    tone = sweep_sine(seconds, lambda t: 200.0 + 700.0 * (t / seconds))
    total = len(noise)
    mixed = [
        (0.8 * a + 0.35 * b) * envelope(n, total, 0.03, 0.05)
        for n, (a, b) in enumerate(zip(noise, tone))
    ]
    return normalise(mixed, 0.50)


def transit(_rng):
    seconds = 1.0
    out = []
    for n in range(int(seconds * RATE)):
        t = n / RATE
        hum = (
            math.sin(2 * math.pi * 110 * t)
            + 0.6 * math.sin(2 * math.pi * 165 * t)
            + 0.3 * math.sin(2 * math.pi * 330 * t)
        )
        out.append(hum * (0.75 + 0.25 * math.sin(2 * math.pi * 4 * t)))
    return normalise(out, 0.28)


def portal_exit(rng):
    seconds = 0.5
    noise = band_noise(seconds, lambda t: 2000.0 - 1700.0 * (t / seconds), rng)
    tone = sweep_sine(seconds, lambda t: 700.0 - 450.0 * (t / seconds))
    total = len(noise)
    mixed = [
        (0.8 * a + 0.3 * b) * envelope(n, total, 0.02, 0.06)
        for n, (a, b) in enumerate(zip(noise, tone))
    ]
    return normalise(mixed, 0.38)


def write(path, samples):
    with wave.open(str(path), "wb") as file:
        file.setnchannels(1)
        file.setsampwidth(2)
        file.setframerate(RATE)
        file.writeframes(b"".join(struct.pack("<h", round(s * 32767)) for s in samples))
    peak = max(abs(s) for s in samples)
    rms = math.sqrt(sum(s * s for s in samples) / len(samples))
    print(
        f"{path.name}: {len(samples) / RATE:.2f} s, peak {20 * math.log10(peak):.1f} dBFS, "
        f"rms {20 * math.log10(rms):.1f} dBFS"
    )


def main():
    target = Path(sys.argv[1]) if len(sys.argv) > 1 else Path(__file__).resolve().parent.parent / "sounds"
    target.mkdir(exist_ok=True)
    rng = random.Random(SEED)
    write(target / "portal-open.wav", portal_open(rng))
    write(target / "transit.wav", transit(rng))
    write(target / "portal-exit.wav", portal_exit(rng))


if __name__ == "__main__":
    main()
