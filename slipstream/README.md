# Slipstream (`oag.slipstream`)

A pickup of its own, handed out by the weapon pads beside the title's weapons. Fire it and
the craft goes through a portal and comes out about two seconds further along the circuit,
a little slower. It adds no game content: the icons are original art (`icons/README.md`) and
the portal is a tinted half disc (`fx/portal.png`).

Off until you turn it on on the MODS page. It supports Wipeout Pulse (PSP and PS2 share one
icon), HD/Fury, Omega and 2048. Wipeout Pure is left out: no Slipstream icon art is shipped
for it.

![A half disc standing across the road ahead, the moment after fire.](at-the-portal-pulse.png)

## How it plays

- **Pick it up** on a weapon pad. Its share of the draws is its own `[[pickup.odds]]` row
  (the same for every title and class), not a swap of another weapon.
- **Fire it** (the pickup button) when the road ahead has room: the portal is a half disc
  12 units across the road's middle, 30 units in front of the craft, and it must fit (the
  road at least 24 wide, nothing between its middle and its rim). With no room the press
  is dropped and you keep the pickup. The AI fires it where the engine's Turbo gate would,
  with the same room rule.
- **Transit** lasts one second: the craft is out of the world (not drawn, not collided with,
  not hit or locked, no steering, no thrust). Your view fills with the title's portal colour
  and the field of view widens by 20 degrees for the whole second.
- **Exit** two seconds of your speed further along the circuit at 90 % of your speed. Laps and
  the finish count through the jump: a jump over the line is a lap, or the finish, on the exit
  tick.
- **One portal at a time.** A fire while another craft is in transit waits in a queue and
  runs when that transit ends, if the portal still fits then.

![In transit on Wipeout Pulse: the view filled with the Pulse portal colour.](in-transit-pulse.png)

![In transit on Wipeout HD: HD's own portal colour, and the icon in HD's pickup frame.](in-transit-hd.png)

## Tuning

Every number is in `guest/src/lib.rs` (distance, radius, transit ticks, exit factor, field of
view, fill ramp) and is **chosen, not measured**; the odds are in `oag-mod.toml`. The portal
look is `Look::of` and `fx/`: a later engine piece draws a real portal, and only those two
change then.

Rebuild after changing the guest: `guest/build.sh` (needs `rustup target add
wasm32-unknown-unknown`), and commit `ext/slipstream.wasm`. `cargo test` in `guest/` runs the
rules natively.
