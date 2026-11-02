# OpenAntiGrav official mods

The project's own features that ship as mods: the game runs without any of them, and
each can be turned off on the MODS page like any other mod
([ADR-0083](https://github.com/topaxi/OpenAntiGrav/blob/main/docs/architecture/adr/0083-official-mods-are-a-public-folder-and-the-fingerprint-counts-race-changers.md)).
The engine repository mounts this one as a submodule at `assets/mods/official`, which is
on the game's default mod search path.

| Mod | What it adds |
| --- | --- |
| [`remix/`](remix/) (`oag.remix`) | RACE REMIX: a REMIX row under RACEBOX (and a tile on Wipeout 2048's grid) opening a page that races one title's track with another title's craft, from every disc this machine has. Its picks are saved under `[mod_settings."oag.remix"]`. Turned off, the row, the page and the tile are gone and nothing else changes. |

The format is documented in the engine repository: `docs/tools/mods.md`.

## Rules

- **No game content.** Nothing extracted from a disc.
- One folder per mod, each with its own `oag-mod.toml`.
- A mod here names only engine vocabulary (rows, pages, queries, `launch_race`); the
  engine names no mod of this repository.

## Licence

Dual-licensed under [MIT](LICENSE-MIT) or [Apache 2.0](LICENSE-APACHE), at your option.
