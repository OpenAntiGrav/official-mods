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
//!   on, `EXIT_SECONDS` of the entry speed past the entrance, at `EXIT_FACTOR` of its speed. A
//!   person's view fills with the title's portal colour and the field of view jumps for the
//!   whole transit.
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

use oag_ext::{
    Craft, Distance, Event, Extension, Observation, Pickup, Reply, Rgba, Slot, SoundMode,
    ViewUpdate, title,
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
const EXIT_SECONDS: f32 = 2.0;
/// The craft leaves at this share of its entry speed.
const EXIT_FACTOR: f32 = 0.9;
/// The vertical field of view's widening while out of the world, degrees.
const FOV_JUMP: f32 = 20.0;
/// Ticks the screen fill takes to come in, and to go out before the exit.
const FILL_RAMP: u16 = 10;
const SLOTS: usize = 8;
/// Km/h in a track unit a second: the exit's distance is `EXIT_SECONDS` of the entry speed.
const SPEED_TO_KMH: f32 = 3.6;
/// How far the portal's surface reaches for the view's visibility test, units: a little over
/// half `fx/portal_*.toml`'s `size`.
const VIEW_RADIUS: u8 = 8;
/// `[shaders]`, by place in name order: `portal`.
const SHADER_PORTAL: u16 = 0;
/// A snapshot is taken again when this changes: it counts the portals, up to the engine's cap.
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
    /// One slot's account (2 bytes).
    #[derive(Clone, Copy, Default)]
    pub struct Memory {
        /// The last answer to its `probe_clearance`.
        pub fits: bool,
        /// Ticks of its transit left, 0 when it is not in one.
        pub left: u8,
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

/// The view of a person's craft in transit: the portal colour in, held, then out ahead of the
/// exit, and the widened field of view.
fn view(reply: &mut Reply<'_>, craft: &Craft<'_>, look: &Look, left: u16) {
    if !craft.is_person() {
        return;
    }
    let came = TRANSIT_TICKS.saturating_sub(left);
    let share = |n: u16| f32::from(n.min(FILL_RAMP)) / f32::from(FILL_RAMP);
    let alpha = share(came + 1) * share(left);
    let [r, g, b] = look.rgb;
    let _ = reply.screen_fill(craft, Rgba::new(r, g, b, (alpha * 255.0) as u8));
    let _ = reply.fov_offset(craft, FOV_JUMP);
}

/// `progress` on by `units`, taken round the lap.
fn on(progress: f32, units: f32, length: f32) -> f32 {
    let at = progress + units;
    let laps = (at / length) as i32 as f32;
    let at = at - laps * length;
    if at < 0.0 { at + length } else { at }
}

/// Where a fire by `craft` stands and looks out: the entrance `AHEAD` units on and the exit
/// `EXIT_SECONDS` of its speed past that. `None` with no course or no fix.
fn places(craft: &Craft<'_>, length: f32) -> Option<(f32, f32)> {
    let progress = craft.progress().filter(|_| length > 0.0)?;
    let exit_units = EXIT_SECONDS * craft.speed_kmh() / SPEED_TO_KMH;
    Some((
        on(progress, AHEAD, length),
        on(progress, AHEAD + exit_units, length),
    ))
}

/// `to` less `from` round the lap, in `-length / 2..length / 2`: negative once `from` is past it.
fn between(from: f32, to: f32, length: f32) -> f32 {
    let delta = to - from;
    delta - (delta / length + if delta < 0.0 { -0.5 } else { 0.5 }) as i32 as f32 * length
}

/// Asks for the road at the exit through the entrance, in the view of every person's craft,
/// and tells the shader the quality. Low asks for no view.
fn show(obs: &Observation<'_>, state: &Portal, reply: &mut Reply<'_>, quality: Quality) {
    for craft in obs.crafts().filter(Craft::is_person) {
        // A reply that is full loses a look and nothing else.
        let _ = reply.set_shader_param(&craft, SHADER_PORTAL, 0, quality.param());
        let update = match quality {
            Quality::Low => continue,
            Quality::Medium => ViewUpdate::Once,
            Quality::High => ViewUpdate::Continuous,
        };
        let _ = reply.portal_view(
            &craft,
            (state.entrance, state.exit),
            VIEW_RADIUS,
            update,
            state.generation,
        );
    }
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

/// Whether the craft that fired has reached the entrance: its progress is at or past it, or
/// without a fix it has flown `AHEAD` units.
fn arrived(craft: &Craft<'_>, state: &mut Portal, length: f32) -> bool {
    match craft
        .progress()
        .filter(|_| length > 0.0 && state.entrance >= 0.0)
    {
        Some(progress) => between(progress, state.entrance, length) <= 0.0,
        None => {
            state.flown += craft.speed_kmh() / SPEED_TO_KMH / 60.0;
            state.flown >= AHEAD
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
                    if let Some(memory) = state.slots.get_mut(slot.index()) {
                        memory.left = 0;
                    }
                    if usize::from(state.active) == slot.index() + 1 {
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
                        show(obs, state, reply, Quality::of(obs.option(0)));
                        if arrived(&craft, state, obs.course_length()) {
                            state.approaching = false;
                            state.slots[index].left = TRANSIT_TICKS as u8;
                            let length = obs.course_length();
                            let progress = craft.progress().filter(|_| state.entrance >= 0.0);
                            let units = progress.map(|p| between(p, state.exit, length));
                            enter(reply, &craft, &look, units.map(|u| u.max(0.0)));
                        }
                    }
                }
                Some(craft) if left > 0 => {
                    let left = left - 1;
                    state.slots[index].left = left;
                    view(reply, &craft, &look, u16::from(left));
                    show(obs, state, reply, Quality::of(obs.option(0)));
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
                    state.generation = (state.generation + 1) % GENERATIONS;
                    state.approaching = true;
                    state.waited = 0;
                    state.flown = 0.0;
                } else {
                    // No fix: `portal_view` refuses a progress below zero, so no view, and
                    // there is no progress to wait on, so the craft goes through now.
                    state.entrance = -1.0;
                    memory.left = TRANSIT_TICKS as u8;
                    enter(reply, &craft, &look, None);
                }
            }
        }

        for (index, memory) in state.slots.iter_mut().enumerate() {
            let craft = Slot::new(index).and_then(|slot| obs.craft(slot));
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
