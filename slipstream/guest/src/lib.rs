//! Slipstream: a pickup that carries a craft through a portal to a point further along the
//! circuit.
//!
//! - **Pick it up**: the weapon pads hand it out beside the title's weapons (`[[pickup.odds]]`).
//! - **Fire it**: the press (a person's) or the driver's Turbo gate (an AI craft's) raises
//!   `pickup-fire`. The craft goes through only if the half disc standing `AHEAD` units in front
//!   of it fits (`probe-clearance`, answered a tick late, re-asked every tick the craft holds
//!   the pickup); otherwise the press is dropped and the craft keeps the pickup.
//! - **Transit**: `TRANSIT_TICKS` out of the world, then `EXIT_SECONDS` of its own speed further
//!   along the circuit at `EXIT_FACTOR` of its speed. A person's view fills with the title's
//!   portal colour and the field of view jumps for the whole transit.
//! - **One at a time**: a race has one active portal pair. A fire requested while another craft
//!   is in transit waits in a queue and runs when that transit ends, if the portal still fits.
//!
//! The portal is the engine's mod effect at the road ahead (`fx/portal_<title>.toml`, a
//! camera-facing sprite): all of its look is in `Look` and `fx/`, apart from the rules.
#![cfg_attr(not(test), no_std)]

use oag_ext::{
    Craft, Cue, Distance, Event, Extension, Observation, Pickup, Reply, Rgba, Slot, title,
};

// The numbers below are chosen, not measured.

/// Where the entry portal stands, units ahead of the craft.
const AHEAD: f32 = 30.0;
/// The half disc's radius, units: the road must be at least twice as wide.
const RADIUS: f32 = 12.0;
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

/// What a title's portal looks like: its effect (`[effects]` in name order) and the colour of
/// the screen fill.
struct Look {
    effect: u16,
    rgb: [u8; 3],
}

impl Look {
    /// The four titles the manifest lists; Pulse's look is the fallback for a title the host
    /// does not name.
    fn of(title: u32) -> Self {
        if title == title!("wipeout-2048") {
            Self { effect: 0, rgb: [0x20, 0xE0, 0x70] }
        } else if title == title!("wipeout-hd") {
            Self { effect: 1, rgb: [0xFF, 0x8A, 0x20] }
        } else if title == title!("wipeout-omega-collection") {
            Self { effect: 2, rgb: [0xA0, 0x60, 0xFF] }
        } else {
            Self { effect: 3, rgb: [0x30, 0xB8, 0xFF] }
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

oag_ext::blob! {
    /// Every slot, the queue of fires waiting for the portal (slot + 1, 0 is empty, oldest
    /// first) and the slot in transit (slot + 1, 0 is none).
    pub struct Portal {
        pub slots: [Memory; SLOTS],
        pub queue: [u8; SLOTS],
        pub active: u8,
    }
}

impl Portal {
    fn enqueue(&mut self, slot: Slot) {
        let mark = slot.index() as u8 + 1;
        if self.queue.contains(&mark) {
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

fn go(reply: &mut Reply<'_>, craft: &Craft<'_>, look: &Look) {
    let exit = Distance::Seconds(EXIT_SECONDS);
    let _ = reply.transit(craft, TRANSIT_TICKS, exit, EXIT_FACTOR);
    let _ = reply.clear_pickup(craft);
    let _ = reply.fire_mod_effect_ahead(craft, look.effect, Distance::Units(AHEAD));
    let _ = reply.fire_mod_effect_ahead(craft, look.effect, exit);
    let _ = reply.play_cue(craft, Cue::Turbo);
}

impl Extension for Slipstream {
    type State = Portal;

    fn tick(obs: &Observation<'_>, state: &mut Portal, reply: &mut Reply<'_>) {
        let look = Look::of(obs.title());
        for event in obs.events() {
            match event {
                Event::PickupFire { slot } => state.enqueue(slot),
                Event::Clearance { slot, fits, .. } => state.slots[slot.index()].fits = fits,
                Event::TransitEnd { slot } => {
                    state.slots[slot.index()].left = 0;
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
            let memory = &mut state.slots[index];
            match craft {
                Some(craft) if memory.left > 0 => {
                    memory.left -= 1;
                    view(reply, &craft, &look, u16::from(memory.left));
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
                && state.slots[index].fits
            {
                go(reply, &craft, &look);
                state.slots[index].left = TRANSIT_TICKS as u8;
                state.active = index as u8 + 1;
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
