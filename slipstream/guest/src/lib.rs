//! Slipstream: a pickup that carries a craft through a portal to a point further along the
//! circuit.
//!
//! - **Pick it up**: the weapon pads hand it out beside the title's weapons (`[[pickup.odds]]`).
//! - **Fire it**: the press (a person's) or the driver's Turbo gate (an AI craft's) raises
//!   `pickup-fire`. The craft goes through only if the half disc standing `AHEAD` units in front
//!   of it fits (`probe-clearance`, answered a tick late, re-asked every tick the craft holds
//!   the pickup); otherwise the press is dropped and the craft keeps the pickup.
//! - **The approach**: the portal opens `AHEAD` units in front of the craft at the fire and the
//!   craft keeps flying; it goes through when its progress reaches the entrance, so the firer
//!   sees the window before the screen fills. The pickup is spent at the entrance, not at the
//!   press. A craft that has not arrived `APPROACH_LIMIT` ticks after the press (it stopped,
//!   hit a wall, was wrecked or lost the pickup) closes the portal and keeps the pickup. With no
//!   course fix the craft goes through on the press.
//! - **Transit**: `TRANSIT_TICKS` out of the world, landing at the exit the portal looked out
//!   on, `EXIT_SECONDS` of the entry speed past the entrance, at `EXIT_FACTOR` of its speed.
//! - **The flight**, a person's own view of it (`Flight`, `fx/wormhole.wgsl`): the portal is
//!   a tunnel, its half disc drawn out along the way the craft flies. The craft goes in
//!   through the mouth, still drawn, its camera after it; the exit moves back down the tunnel
//!   as it goes in, with the road there seen through it at its own size; then the craft closes
//!   on it and flies out through it. The field of view rides one curve from the approach to
//!   the ticks after the exit, with no step at either end.
//! - **One at a time**: a race has one active portal pair. A fire requested while another craft
//!   is in transit waits in a queue and runs when that transit ends, if the portal still fits.
//!
//! - **Sound**: the entry, a hum for the transit and the exit are the mod's own sounds
//!   (`[sounds]`), played from the craft; a person hears their own craft dry and the others
//!   placed in the world.
//!
//! - **The view**: while a portal stands, `portal_view` asks the engine for the road at the exit
//!   through the entrance for every person's craft, and `fx/portal.wgsl` draws it on the portal's
//!   surface. How is the player's QUALITY row (`menu.toml`, read with `Observation::option`):
//!   Low asks for nothing and the shader draws its own animated fallback, Medium asks for one
//!   snapshot per portal, High for the view every frame. Drawn only: the pick never reaches the
//!   rules, so a race plays out the same at all three.
//!
//! The portal is the engine's mod effect at the road ahead (`fx/portal_<title>.toml`, a
//! camera-facing sprite drawn by `fx/portal.wgsl`): all of its look is in `Look` and `fx/`,
//! apart from the rules.
#![cfg_attr(not(test), no_std)]

use oag_ext::lap::{ahead, gap};
use oag_ext::{
    Craft, Distance, Event, Extension, Observation, Pickup, Reply, Rgba, Slot, SoundMode,
    TICKS_PER_SECOND, ViewUpdate, title,
};

// The numbers below are chosen, not measured.

/// Where the entry portal stands, units ahead of the craft.
const AHEAD: f32 = 30.0;
/// The half disc's radius, units: the road must be at least twice as wide.
const RADIUS: f32 = 12.0;
/// Ticks after the press the craft has to reach the entrance in, else the portal closes: under
/// the portal effect's `lifetime`, so the window is still standing when the portal gives up.
const APPROACH_LIMIT: u8 = 50;
/// Ticks out of the world (1 s).
const TRANSIT_TICKS: u16 = 60;
/// The exit lies this many seconds ahead at the speed the craft has.
const EXIT_SECONDS: f32 = 6.0;
/// The craft leaves at this share of its entry speed.
const EXIT_FACTOR: f32 = 0.9;
/// The vertical field of view's widening at its widest, in the tunnel, degrees.
const FOV_PEAK: f32 = 24.0;
/// The widening the approach has built by the entrance, degrees.
const FOV_APPROACH: f32 = 5.0;
/// Ticks from the entrance to the widest field.
const FOV_RISE: f32 = 16.0;
/// Ticks before the exit the field starts to narrow again.
const FOV_FALL: u16 = 18;
/// Ticks from the entrance by which the engine has seated the camera of the craft going
/// through on the exit's framing: the ease it takes and the spring behind it. A snapshot of
/// the exit taken from here is the picture the camera has at the exit.
const SEATED: u16 = 16;
/// Ticks after the exit the field takes to settle; what is left of the tunnel has fallen
/// behind well inside them.
const TAIL_TICKS: u8 = 24;
/// The last tick out of the world, counted from the entrance: the tick before the exit.
const LAST: f32 = (TRANSIT_TICKS - 1) as f32;
const SLOTS: usize = 8;
/// How far the portal's surface reaches for the view's visibility test, units: a little over
/// half `fx/portal_*.toml`'s `size`.
const VIEW_RADIUS: u8 = 8;
/// `[shaders]`, by place in name order: `portal`, `wormhole`.
const SHADER_PORTAL: u16 = 0;
const SHADER_WORMHOLE: u16 = 1;
/// `fx/wormhole.wgsl`'s scalars, by place in its header.
const PARAM_PHASE: u8 = 0;
const PARAM_TRANSIT: u8 = 1;
const PARAM_SPEED: u8 = 2;
/// A snapshot is taken again when this changes: a portal takes two, one as it opens and one
/// for the far end of its tunnel once the camera is seated, up to the engine's cap.
const GENERATIONS: u8 = 64;

/// The mod's own sounds by their place in `[sounds]`, which is name order: `portal-exit`,
/// `portal-open`, `transit` (`sounds/`, made by `src/synth_sounds.py`).
const SOUND_EXIT: u16 = 0;
const SOUND_OPEN: u16 = 1;
const SOUND_TRANSIT: u16 = 2;

/// What a title's portal looks like: its effect (`[effects]` in name order) and the colour of
/// the screen fill.
struct Look {
    effect: u16,
    rgb: [u8; 3],
}

impl Look {
    /// The five titles the manifest lists; Pulse's look is the fallback for a title the host
    /// does not name.
    fn of(title: u32) -> Self {
        if title == title!("wipeout-2048") {
            Self {
                effect: 0,
                rgb: [0x20, 0xE0, 0x70],
            }
        } else if title == title!("wipeout-hd") {
            Self {
                effect: 1,
                rgb: [0xFF, 0x8A, 0x20],
            }
        } else if title == title!("wipeout-omega-collection") {
            Self {
                effect: 2,
                rgb: [0xA0, 0x60, 0xFF],
            }
        } else if title == title!("wipeout-pure") {
            Self {
                effect: 4,
                rgb: [0x40, 0xFF, 0x40],
            }
        } else {
            Self {
                effect: 3,
                rgb: [0x30, 0xB8, 0xFF],
            }
        }
    }
}

oag_ext::blob! {
    /// One slot's account (7 bytes).
    #[derive(Clone, Copy, Default)]
    pub struct Memory {
        /// The last answer to its `probe_clearance`.
        pub fits: bool,
        /// Ticks of its transit left, 0 when it is not in one.
        pub left: u8,
        /// Ticks since it came out, while the tunnel clears its view; 0 when it has.
        pub after: u8,
        /// What it went in at, units a tick: its tunnel is drawn at that to the end.
        pub pace: f32,
    }
}

/// How the portal's surface is drawn, the mod's QUALITY row (`menu.toml`: low, medium, high).
#[derive(Clone, Copy, PartialEq, Debug)]
enum Quality {
    /// No second view: the shader's own animated surface.
    Low,
    /// One snapshot of the exit per portal, held while the craft closes.
    Medium,
    /// The exit's view every frame.
    High,
}

impl Quality {
    fn of(option: u8) -> Self {
        match option {
            0 => Self::Low,
            1 => Self::Medium,
            _ => Self::High,
        }
    }

    /// The shader's `quality` parameter.
    fn param(self) -> f32 {
        f32::from(self as u8)
    }
}

oag_ext::blob! {
    /// Every slot, the queue of fires waiting for the portal (slot + 1, 0 is empty, oldest
    /// first) and the slot in transit (slot + 1, 0 is none).
    pub struct Portal {
        pub slots: [Memory; SLOTS],
        pub queue: [u8; SLOTS],
        pub active: u8,
        /// Where the portal in use stands and where it looks out, progress on the lap.
        pub entrance: f32,
        pub exit: f32,
        /// Counts the portals, for a snapshot to be taken again.
        pub generation: u8,
        /// The fire is flying to the entrance, not yet in transit.
        pub approaching: bool,
        /// Ticks since the press, while approaching.
        pub waited: u8,
        /// Units flown since the press, the arrival test when the craft has no course fix.
        pub flown: f32,
    }
}

impl Portal {
    fn enqueue(&mut self, slot: Slot) {
        let mark = slot.index() as u8 + 1;
        if self.queue.contains(&mark) || self.active == mark {
            return;
        }
        if let Some(free) = self.queue.iter_mut().find(|q| **q == 0) {
            *free = mark;
        }
    }

    fn dequeue(&mut self) -> Option<usize> {
        let first = self.queue[0];
        self.queue.copy_within(1.., 0);
        self.queue[SLOTS - 1] = 0;
        (first != 0).then(|| usize::from(first) - 1)
    }
}

pub struct Slipstream;

fn holds_it(craft: &Craft<'_>) -> bool {
    craft.pickup() == Pickup::Ours && craft.is_racing() && !craft.in_transit()
}

/// Where a person's craft is on its way through, which is all its view is drawn from.
#[derive(Clone, Copy, PartialEq, Debug)]
enum Flight {
    /// Flying to the open portal, this share of the way there: the field of view alone.
    Approach(f32),
    /// At or past the entrance: ticks since it, `0.0..=LAST` out of the world and
    /// `LAST + 2.0..` once it is out. No tick is `LAST + 1.0`, so a frame drawn between two
    /// ticks never shows the far side of the exit before the camera is there.
    Through(f32),
}

impl Flight {
    /// The ticks since the entrance of a craft with `left` ticks of its transit to go.
    fn out_of_the_world(left: u8) -> Self {
        Self::Through(f32::from(TRANSIT_TICKS.saturating_sub(u16::from(left))).min(LAST))
    }

    /// The ticks since the entrance of a craft `after` ticks out of its transit.
    fn back(after: u8) -> Self {
        Self::Through(LAST + 1.0 + f32::from(after))
    }

    /// The field of view's widening, degrees: built gently on the approach, up to the peak
    /// past the entrance, and down from before the exit to the end of the tail on one curve.
    fn widening(self) -> f32 {
        match self {
            Self::Approach(share) => FOV_APPROACH * share * share,
            Self::Through(ticks) => {
                // The tick `Through` leaves out is no tick of the curve's.
                let ticks = if ticks > LAST { ticks - 1.0 } else { ticks };
                let rise = FOV_APPROACH + (FOV_PEAK - FOV_APPROACH) * smooth(ticks / FOV_RISE);
                let from = LAST - f32::from(FOV_FALL);
                let over = f32::from(FOV_FALL) + f32::from(TAIL_TICKS);
                rise * (1.0 - smooth((ticks - from) / over))
            }
        }
    }
}

/// The smoothstep of `x` held to `0..1`.
fn smooth(x: f32) -> f32 {
    let x = x.clamp(0.0, 1.0);
    x * x * (3.0 - 2.0 * x)
}

/// A person's view of their own flight: the field of view, and from the entrance the tunnel
/// (`fx/wormhole.wgsl`) in the title's colour, told how long the craft has been through and
/// its `pace`, what it went in at in units a tick, which is how fast the tunnel is flown.
fn view(reply: &mut Reply<'_>, craft: &Craft<'_>, look: &Look, flight: Flight, pace: f32) {
    if !craft.is_person() {
        return;
    }
    let _ = reply.fov_offset(craft, flight.widening());
    let Flight::Through(ticks) = flight else {
        return;
    };
    let [r, g, b] = look.rgb;
    let mut scalar = |index, value| {
        let _ = reply.set_shader_param(craft, SHADER_WORMHOLE, index, value);
    };
    scalar(PARAM_PHASE, ticks);
    scalar(PARAM_TRANSIT, LAST);
    scalar(PARAM_SPEED, pace);
    let _ = reply.screen_fill_shaded(craft, Rgba::new(r, g, b, 255), SHADER_WORMHOLE);
}

/// Where a fire by `craft` stands and looks out: the entrance `AHEAD` units on and the exit
/// `EXIT_SECONDS` of its speed past that. `None` with no course or no fix.
fn places(craft: &Craft<'_>, length: f32) -> Option<(f32, f32)> {
    let progress = craft.progress().filter(|_| length > 0.0)?;
    let exit_units = EXIT_SECONDS * craft.speed();
    Some((
        ahead(progress, AHEAD, length),
        ahead(progress, AHEAD + exit_units, length),
    ))
}

/// Asks for the road at the exit through the entrance, in the view of every person's craft,
/// and tells the shader the quality. Low asks for no view.
///
/// The craft going through (`through`: the ticks since its entrance) looks from where it
/// stands instead of from where the portal was opened: the engine seats its camera for that
/// pair, so the road at the far end of its tunnel is the one its camera has on the exit tick.
/// At High from the entrance, the picture following the camera onto its seat; at Medium one
/// more snapshot, taken once the camera is there.
fn show(
    obs: &Observation<'_>,
    state: &Portal,
    reply: &mut Reply<'_>,
    quality: Quality,
    through: Option<u16>,
) {
    for craft in obs.crafts().filter(Craft::is_person) {
        // A reply that is full loses a look and nothing else.
        let _ = reply.set_shader_param(&craft, SHADER_PORTAL, 0, quality.param());
        let update = match quality {
            Quality::Low => continue,
            Quality::Medium => ViewUpdate::Once,
            Quality::High => ViewUpdate::Continuous,
        };
        let stands = through.is_some_and(|ticks| quality == Quality::High || ticks >= SEATED);
        let own = stands && craft.slot().index() + 1 == usize::from(state.active);
        let (entrance, generation) = match craft.progress().filter(|_| own) {
            Some(stands) => (stands, (state.generation + 1) % GENERATIONS),
            None => (state.entrance, state.generation),
        };
        let _ = reply.portal_view(
            &craft,
            (entrance, state.exit),
            VIEW_RADIUS,
            update,
            generation,
        );
    }
}

/// What the craft flies, units a tick.
fn pace_of(craft: &Craft<'_>) -> f32 {
    craft.speed() / TICKS_PER_SECOND
}

/// The portal opens in front of the craft.
fn open(reply: &mut Reply<'_>, craft: &Craft<'_>, look: &Look) {
    let _ = reply.fire_mod_effect_ahead(craft, look.effect, Distance::Units(AHEAD));
    let _ = reply.play_mod_sound(craft, SOUND_OPEN, SoundMode::Once);
}

/// The craft is at the entrance: out of the world, to land `exit_units` on, with the exit
/// standing there and the hum playing. `None` flies `EXIT_SECONDS` of its speed.
fn enter(reply: &mut Reply<'_>, craft: &Craft<'_>, look: &Look, exit_units: Option<f32>) {
    let exit = exit_units.map_or(Distance::Seconds(EXIT_SECONDS), Distance::Units);
    let _ = reply.transit(craft, TRANSIT_TICKS, exit, EXIT_FACTOR);
    let _ = reply.clear_pickup(craft);
    let _ = reply.fire_mod_effect_ahead(craft, look.effect, exit);
    let _ = reply.play_mod_sound(craft, SOUND_TRANSIT, SoundMode::Loop);
}

/// How far the craft that fired still has to fly to the entrance, units: none once its
/// progress is at or past it, or without a fix once it has flown `AHEAD` units.
fn short_of(craft: &Craft<'_>, state: &mut Portal, length: f32) -> f32 {
    match craft
        .progress()
        .filter(|_| length > 0.0 && state.entrance >= 0.0)
    {
        Some(progress) => gap(progress, state.entrance, length).max(0.0),
        None => {
            state.flown += pace_of(craft);
            (AHEAD - state.flown).max(0.0)
        }
    }
}

/// The craft is back in the world: the transit hum ends and the exit sounds.
fn arrive(reply: &mut Reply<'_>, craft: &Craft<'_>) {
    let _ = reply.play_mod_sound(craft, SOUND_TRANSIT, SoundMode::Stop);
    let _ = reply.play_mod_sound(craft, SOUND_EXIT, SoundMode::Once);
}

impl Extension for Slipstream {
    type State = Portal;

    fn tick(obs: &Observation<'_>, state: &mut Portal, reply: &mut Reply<'_>) {
        let look = Look::of(obs.title());
        for event in obs.events() {
            match event {
                Event::PickupFire { slot } => state.enqueue(slot),
                Event::Clearance { slot, fits, .. } => {
                    if let Some(memory) = state.slots.get_mut(slot.index()) {
                        memory.fits = fits;
                    }
                }
                Event::TransitEnd { slot } => {
                    if let Some(craft) = obs.craft(slot) {
                        arrive(reply, &craft);
                    }
                    let ours = usize::from(state.active) == slot.index() + 1;
                    if let Some(memory) = state.slots.get_mut(slot.index()) {
                        memory.left = 0;
                        // Another mod's transit ends here too, and has no tunnel to clear.
                        if ours {
                            memory.after = 1;
                        }
                    }
                    if ours {
                        state.active = 0;
                    }
                }
                _ => {}
            }
        }

        if state.active != 0 {
            let index = usize::from(state.active) - 1;
            let craft = Slot::new(index).and_then(|slot| obs.craft(slot));
            let left = state.slots.get(index).map_or(0, |memory| memory.left);
            match craft {
                Some(craft) if state.approaching => {
                    state.waited = state.waited.saturating_add(1);
                    if !holds_it(&craft) || state.waited > APPROACH_LIMIT {
                        state.approaching = false;
                        state.active = 0;
                    } else {
                        show(obs, state, reply, Quality::of(obs.option(0)), None);
                        let short = short_of(&craft, state, obs.course_length());
                        if short > 0.0 {
                            view(
                                reply,
                                &craft,
                                &look,
                                Flight::Approach(1.0 - short / AHEAD),
                                0.0,
                            );
                        } else {
                            state.approaching = false;
                            let pace = pace_of(&craft);
                            state.slots[index].left = TRANSIT_TICKS as u8;
                            state.slots[index].pace = pace;
                            let length = obs.course_length();
                            let progress = craft.progress().filter(|_| state.entrance >= 0.0);
                            let units = progress.map(|p| gap(p, state.exit, length));
                            enter(reply, &craft, &look, units.map(|u| u.max(0.0)));
                            view(reply, &craft, &look, Flight::Through(0.0), pace);
                        }
                    }
                }
                Some(craft) if left > 0 => {
                    let left = left - 1;
                    state.slots[index].left = left;
                    let pace = state.slots[index].pace;
                    view(reply, &craft, &look, Flight::out_of_the_world(left), pace);
                    let through = TRANSIT_TICKS.saturating_sub(u16::from(left));
                    show(obs, state, reply, Quality::of(obs.option(0)), Some(through));
                }
                Some(craft) if craft.in_transit() => {}
                _ => state.active = 0,
            }
        }

        while state.active == 0 {
            let Some(index) = state.dequeue() else { break };
            let craft = Slot::new(index).and_then(|slot| obs.craft(slot));
            if let Some(craft) = craft
                && holds_it(&craft)
                && let Some(memory) = state.slots.get_mut(index)
                && memory.fits
            {
                open(reply, &craft, &look);
                state.active = index as u8 + 1;
                if let Some((entrance, exit)) = places(&craft, obs.course_length()) {
                    state.entrance = entrance;
                    state.exit = exit;
                    state.generation = (state.generation + 2) % GENERATIONS;
                    state.approaching = true;
                    state.waited = 0;
                    state.flown = 0.0;
                } else {
                    // No fix: `portal_view` refuses a progress below zero, so no view, and
                    // there is no progress to wait on, so the craft goes through now.
                    state.entrance = -1.0;
                    memory.left = TRANSIT_TICKS as u8;
                    memory.pace = pace_of(&craft);
                    enter(reply, &craft, &look, None);
                    view(reply, &craft, &look, Flight::Through(0.0), memory.pace);
                }
            }
        }

        for (index, memory) in state.slots.iter_mut().enumerate() {
            let craft = Slot::new(index).and_then(|slot| obs.craft(slot));
            if memory.after > 0 {
                if let Some(craft) = &craft {
                    view(reply, craft, &look, Flight::back(memory.after), memory.pace);
                }
                memory.after = if memory.after < TAIL_TICKS {
                    memory.after + 1
                } else {
                    0
                };
            }
            match craft {
                Some(craft) if holds_it(&craft) && state.active != index as u8 + 1 => {
                    let _ = reply.probe_clearance(&craft, Distance::Units(AHEAD), RADIUS);
                }
                _ => memory.fits = false,
            }
        }
    }
}

oag_ext::export!(Slipstream);

#[cfg(test)]
mod tests;
