use oag_ext::test::{CraftRecord, Race};
use oag_ext::{Command, Event};

use super::*;

fn holder(person: bool) -> CraftRecord {
    CraftRecord {
        pickup: Pickup::Ours,
        person,
        racing: true,
        speed_kmh: 400.0,
        ..CraftRecord::default()
    }
}

fn slot(index: usize) -> Slot {
    Slot::new(index).unwrap()
}

fn fits(slot: Slot, fits: bool) -> Event {
    Event::Clearance {
        slot,
        fits,
        ahead: AHEAD,
    }
}

fn transits(commands: &[Command]) -> usize {
    commands
        .iter()
        .filter(|c| matches!(c, Command::Transit { .. }))
        .count()
}

#[test]
fn a_holder_probes_every_tick_and_a_press_without_room_keeps_the_pickup() {
    let me = slot(0);
    let mut race = Race::<Slipstream>::new().with_craft(me, holder(true));
    let first = race.tick();
    assert!(first.contains(&Command::ProbeClearance {
        slot: me,
        distance: Distance::Units(AHEAD),
        radius: RADIUS
    }));
    race.event(Event::PickupFire { slot: me });
    race.event(fits(me, false));
    let refused = race.tick();
    assert_eq!(transits(&refused), 0);
    assert!(!refused.iter().any(|c| matches!(c, Command::ClearPickup { .. })));
}

#[test]
fn a_press_with_room_goes_through_and_the_pickup_is_spent() {
    let me = slot(0);
    let mut race = Race::<Slipstream>::new()
        .with_craft(me, holder(true))
        .with_title(title!("wipeout-hd"));
    race.event(Event::PickupFire { slot: me });
    race.event(fits(me, true));
    let sent = race.tick();
    assert!(sent.contains(&Command::Transit {
        slot: me,
        ticks: TRANSIT_TICKS,
        distance: Distance::Seconds(EXIT_SECONDS),
        exit_factor: EXIT_FACTOR
    }));
    assert!(sent.contains(&Command::ClearPickup { slot: me }));
    let portals: Vec<u16> = sent
        .iter()
        .filter_map(|c| match c {
            Command::FireModEffectAhead { index, .. } => Some(*index),
            _ => None,
        })
        .collect();
    assert_eq!(portals, [1, 1], "HD's portal at the entry and the exit");
}

#[test]
fn the_entry_opens_with_a_sound_and_starts_the_transit_hum_which_the_exit_stops() {
    let me = slot(0);
    let mut race = Race::<Slipstream>::new().with_craft(me, holder(true));
    race.event(Event::PickupFire { slot: me });
    race.event(fits(me, true));
    let sent = race.tick();
    let sounds = |commands: &[Command]| -> Vec<(u16, SoundMode)> {
        commands
            .iter()
            .filter_map(|c| match c {
                Command::PlayModSound { index, mode, .. } => Some((*index, *mode)),
                _ => None,
            })
            .collect()
    };
    assert_eq!(
        sounds(&sent),
        [(SOUND_OPEN, SoundMode::Once), (SOUND_TRANSIT, SoundMode::Loop)]
    );
    *race.craft_mut(me) = Some(CraftRecord {
        pickup: Pickup::None,
        in_transit: true,
        ..holder(true)
    });
    for _ in 0..10 {
        assert!(sounds(&race.tick()).is_empty(), "the hum is not re-sent");
    }
    race.event(Event::TransitEnd { slot: me });
    assert_eq!(
        sounds(&race.tick()),
        [(SOUND_TRANSIT, SoundMode::Stop), (SOUND_EXIT, SoundMode::Once)]
    );
}

#[test]
fn the_sound_indices_are_the_name_order_of_the_manifest() {
    let manifest = include_str!("../../oag-mod.toml");
    let table = manifest.split("[sounds]").nth(1).expect("a [sounds] table");
    let names: Vec<&str> = table
        .lines()
        .skip(1)
        .take_while(|line| !line.starts_with('['))
        .filter_map(|line| line.split('=').next())
        .map(str::trim)
        .filter(|name| !name.is_empty() && !name.starts_with('#'))
        .collect();
    assert_eq!(names[usize::from(SOUND_EXIT)], "portal-exit");
    assert_eq!(names[usize::from(SOUND_OPEN)], "portal-open");
    assert_eq!(names[usize::from(SOUND_TRANSIT)], "transit");
    let mut sorted = names.clone();
    sorted.sort_unstable();
    assert_eq!(names, sorted, "listed in name order");
}

#[test]
fn the_view_fills_with_the_titles_colour_and_widens_for_the_whole_transit() {
    let me = slot(0);
    let mut race = Race::<Slipstream>::new()
        .with_craft(me, holder(true))
        .with_title(title!("wipeout-2048"));
    race.event(Event::PickupFire { slot: me });
    race.event(fits(me, true));
    race.tick();
    *race.craft_mut(me) = Some(CraftRecord {
        pickup: Pickup::None,
        in_transit: true,
        ..holder(true)
    });
    let mut alphas = Vec::new();
    for _ in 0..TRANSIT_TICKS - 1 {
        let tick = race.tick();
        assert!(tick.contains(&Command::FovOffset {
            slot: me,
            degrees: FOV_JUMP
        }));
        let Some(Command::ScreenFill { rgba, .. }) = tick
            .iter()
            .find(|c| matches!(c, Command::ScreenFill { .. }))
        else {
            panic!("a fill every tick");
        };
        assert_eq!((rgba.r, rgba.g, rgba.b), (0x20, 0xE0, 0x70));
        alphas.push(rgba.a);
    }
    assert_eq!(alphas[FILL_RAMP as usize + 2], 255, "full in the middle");
    assert!(alphas[0] < 60 && *alphas.last().unwrap() < 60, "ramps at both ends");
}

#[test]
fn a_second_fire_waits_for_the_transit_to_end_and_runs_if_it_still_fits() {
    let (a, b) = (slot(0), slot(1));
    let mut race = Race::<Slipstream>::new()
        .with_craft(a, holder(true))
        .with_craft(b, holder(false));
    race.event(Event::PickupFire { slot: a });
    race.event(Event::PickupFire { slot: b });
    race.event(fits(a, true));
    race.event(fits(b, true));
    let first = race.tick();
    assert_eq!(transits(&first), 1, "one portal pair at a time");
    *race.craft_mut(a) = Some(CraftRecord {
        pickup: Pickup::None,
        in_transit: true,
        ..holder(true)
    });
    for _ in 0..10 {
        assert_eq!(transits(&race.tick()), 0, "queued while the first is out");
    }
    *race.craft_mut(a) = Some(CraftRecord {
        pickup: Pickup::None,
        ..holder(true)
    });
    race.event(Event::TransitEnd { slot: a });
    race.event(fits(b, true));
    let second = race.tick();
    assert_eq!(transits(&second), 1);
    assert!(second.contains(&Command::ClearPickup { slot: b }));
}

#[test]
fn a_queued_fire_is_dropped_when_the_portal_no_longer_fits() {
    let (a, b) = (slot(0), slot(1));
    let mut race = Race::<Slipstream>::new()
        .with_craft(a, holder(true))
        .with_craft(b, holder(false));
    race.event(Event::PickupFire { slot: a });
    race.event(Event::PickupFire { slot: b });
    race.event(fits(a, true));
    race.event(fits(b, true));
    race.tick();
    race.event(Event::TransitEnd { slot: a });
    race.event(fits(b, false));
    assert_eq!(transits(&race.tick()), 0);
    race.event(fits(b, true));
    assert_eq!(transits(&race.tick()), 0, "the dropped fire is not remembered");
}

#[test]
fn a_slot_past_the_eighth_is_ignored_not_a_trap() {
    let far = slot(12);
    let mut race = Race::<Slipstream>::new().with_craft(far, holder(false));
    race.event(Event::PickupFire { slot: far });
    race.event(fits(far, true));
    race.event(Event::TransitEnd { slot: far });
    assert_eq!(transits(&race.tick()), 0);
}
