# Slipstream (`oag.slipstream`)

A pickup of its own, handed out by the weapon pads beside the title's weapons. Fire it and
the craft goes through a portal and comes out about two seconds further along the circuit,
a little slower. It adds no game content: the icons are original art (`icons/README.md`), the
portal and the tunnel are drawn by the mod's own shaders (`fx/portal.wgsl`,
`fx/wormhole.wgsl`) and the sounds are synthesised by `src/synth_sounds.py`.

Off until you turn it on on the MODS page. It supports Wipeout Pulse (PSP and PS2 share one
icon), Wipeout Pure, HD/Fury, Omega and 2048.

![A half disc standing across the road ahead, the moment after fire.](at-the-portal-pulse.png)

## How it plays

- **Pick it up** on a weapon pad. Its share of the draws is its own `[[pickup.odds]]` row
  (the same for every title and class), not a swap of another weapon.
- **Fire it** (the pickup button) when the road ahead has room: the portal is a half disc
  12 units across the road's middle, 30 units in front of the craft, and it must fit (the
  road at least 24 wide, nothing between its middle and its rim). With no room the press
  is dropped and you keep the pickup. The AI fires it where the engine's Turbo gate would,
  with the same room rule.
- **Approach**: the portal opens at the press, with a flash, and you keep flying, in the
  world, to it; the window is in front of you until you reach it. The pickup is spent at the
  entrance. If you have not arrived within 50 ticks (stopped, wrecked, lost the pickup) the
  portal closes and you keep the pickup. Other craft do not enter it.
- **Transit** lasts one second from the entrance: the craft is out of the world (not drawn for
  anyone else, not collided with, not hit or locked, no steering, no thrust).
- **Exit** two seconds of your speed (at the press) past the entrance along the circuit at 90 % of your speed, on
  the road's middle at flying height: no drop, you fly on from the first tick. Laps and
  the finish count through the jump: a jump over the line is a lap, or the finish, on the exit
  tick.
- **What you see of your own flight**: the portal is a tunnel (`fx/wormhole.wgsl`, the
  guest's `Flight`). Its half disc is drawn out along the way you fly, an arch over a floor.
  Your craft goes in through the mouth, still on screen, and the camera after it; the track is
  there round the mouth until the camera is through. The exit moves back down the tunnel as you
  go in, for a sixth of a second at your speed, with the road there seen through it at its own
  size. Then you close on it at a steady rate and fly out through it: on the last tick it
  stands where the exit portal does and its picture is the one your camera has a tick later,
  so there is no cut. The field of view rides one curve from the approach to 24 ticks after
  the exit, 24 degrees at its widest. Your craft has no exhaust in the tunnel: the engine
  draws its hull there, not the effects of a craft in the world.
- **One portal at a time.** A fire while another craft is in transit waits in a queue and
  runs when that transit ends, if the portal still fits then.

- **The portal's surface** shows the road at the exit (`fx/portal.wgsl`). OPTIONS > SLIPSTREAM > PORTAL VIEW: Low is an animated
  surface with no second view, Medium one snapshot per portal, High the view every frame (default). It changes nothing in the race.
  The tunnel's far end follows the same pick: the road at the exit at High, a snapshot of it at Medium (a second one a
  portal, taken a quarter of a second in), and light at Low. The second seat of a split screen sees the light too.

- **Sound.** Entering plays a short rising whoosh (`portal-open`) and a low hum
  (`transit`) that lasts the transit; coming out ends the hum and plays a softer falling
  whoosh (`portal-exit`). They come from the craft: you hear your own dry and a rival's
  placed in the world. These are placeholders we synthesise ourselves: `src/synth_sounds.py`
  writes `sounds/*.wav` (standard library only, seeded, MIT OR Apache-2.0), peaks between
  -6 and -11 dBFS. Real sounds replace the three files under the same names.

![In transit on Wipeout Pulse: the craft in the tunnel, in the Pulse portal colour, the road at the exit at its far end.](in-transit-pulse.png)

![In transit on Wipeout HD: HD's own portal colour, and the icon in HD's pickup frame.](in-transit-hd.png)

## Tuning

Every number is in `guest/src/lib.rs` (distance, radius, transit ticks, exit factor, the field
of view's curve) and is **chosen, not measured**; the odds are in `oag-mod.toml`. The look is
`Look::of` and `fx/`: the tunnel's shape (its radius, which is the portal's, how far under the
craft its floor is, how long the exit moves back for, how far apart the wall's rings are) is
the constants at the top of `fx/wormhole.wgsl`, and its pace is your craft's speed; the
portal's opening is the curves in `fx/portal_<title>.toml`. A later engine piece draws a real portal, and only those change
then.

Rebuild after changing the guest: `guest/build.sh` (needs `rustup target add
wasm32-unknown-unknown`), and commit `ext/slipstream.wasm`. `cargo test` in `guest/` runs the
rules natively.
