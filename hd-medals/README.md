# HD Medals (`oag.hd-medals`)

Wipeout Pulse's campaign hexagons show Wipeout HD's own medal icons: on Cell Selection each
earned medal is the HD frame of its tier, from the difficulty block it was earned on. Grid
Selection's tier hexagons keep the title's own swatch (HD's Grid Selection has no tier medals).

It needs a **Wipeout HD disc image** on the game's search path. The icons are read from it at run
time; nothing of them ships here. Without one the colour swatches stay and the MODS page says
`NEEDS  WIPEOUT HD DISC`. Off until you turn it on on the MODS page. Art only: it changes no
rule, so it does not count against the LAN fingerprint.

The whole mod is `oag-mod.toml`: a `[campaign-medals]` table (`docs/tools/mods.md` in the engine
repository, "Campaign medals").
