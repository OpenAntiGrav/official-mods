# Slipstream pickup icons

Original art for the Slipstream pickup, one icon per title in that title's icon style: a single
chevron, the portal arc, then a double chevron. Drawn from scratch; no game pixel was traced,
copied or edited. Licensed like the rest of this repository (MIT OR Apache-2.0).

- `<title>.png`: the shipped icon at the title's native size (Pulse PSP, Pulse PS2, Pure, HD, Omega, 2048).
- `2048.png` carries 2048's green disc; `2048_glyph_only.png` is the glyph alone, for an engine
  that draws the disc itself.
- `src/<title>.svg`: the hand-authored sources the PNGs are rendered from. The mod ships the PNGs.

`pure_psp.png` is Pure's: 64x36, the glyph alone (55x28, 6 px bands as Pure's own pickup glyphs,
Pure's glyph green). Pure draws its pickup icons as models on a striped backdrop, so the engine
draws the backdrop and the mod ships only the glyph; the source is `src/pure_psp.svg`.
