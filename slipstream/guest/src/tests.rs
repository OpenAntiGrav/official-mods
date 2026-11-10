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
    assert!(
        !refused
            .iter()
            .any(|c| matches!(c, Command::ClearPickup { .. }))
    );
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
        [
            (SOUND_OPEN, SoundMode::Once),
            (SOUND_TRANSIT, SoundMode::Loop)
        ]
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
        [
            (SOUND_TRANSIT, SoundMode::Stop),
            (SOUND_EXIT, SoundMode::Once)
        ]
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

/// The tunnel a tick sends a person's view: the fill's colour and the shader's `phase` and
/// `speed`. `None` where it sends no fill.
fn tunnel_of(commands: &[Command]) -> Option<(Rgba, f32, f32)> {
    let rgba = commands.iter().find_map(|c| match c {
        Command::ScreenFill { rgba, shader, .. } => {
            assert_eq!(*shader, Some(SHADER_WORMHOLE), "the fill is the tunnel");
            Some(*rgba)
        }
        _ => None,
    })?;
    let scalar = |index| {
        commands.iter().find_map(|c| match c {
            Command::SetShaderParam {
                shader,
                index: at,
                value,
                ..
            } if *shader == SHADER_WORMHOLE && *at == index => Some(*value),
            _ => None,
        })
    };
    assert_eq!(scalar(PARAM_TRANSIT), Some(LAST));
    Some((rgba, scalar(PARAM_PHASE)?, scalar(PARAM_SPEED)?))
}

/// The widening of the field of view a tick sends.
fn widening_of(commands: &[Command]) -> Option<f32> {
    commands.iter().find_map(|c| match c {
        Command::FovOffset { degrees, .. } => Some(*degrees),
        _ => None,
    })
}

/// A person's whole flight on a 4000 unit lap at 360 km/h: the press at 3900, 5 units a tick
/// to the entrance at 3930, the transit, the exit at 324 km/h and the ticks after it. Each
/// tick's commands.
fn whole_flight() -> Vec<Vec<Command>> {
    let me = slot(0);
    let mut race = flier(3900.0).with_title(title!("wipeout-2048"));
    race.event(Event::PickupFire { slot: me });
    race.event(fits(me, true));
    let mut ticks = vec![race.tick()];
    let mut at = 3900.0;
    loop {
        at += 5.0;
        ticks.push(commands_of(&mut race, at));
        if transits(ticks.last().unwrap()) == 1 {
            break;
        }
    }
    *race.craft_mut(me) = Some(CraftRecord {
        pickup: Pickup::None,
        in_transit: true,
        speed_kmh: 360.0,
        ..holder(true)
    });
    for _ in 1..TRANSIT_TICKS {
        ticks.push(race.tick());
    }
    *race.craft_mut(me) = Some(CraftRecord {
        pickup: Pickup::None,
        speed_kmh: 324.0,
        ..holder(true)
    });
    race.event(Event::TransitEnd { slot: me });
    for _ in 0..usize::from(TAIL_TICKS) + 3 {
        ticks.push(race.tick());
    }
    ticks
}

#[test]
fn the_tunnel_is_told_how_long_the_craft_has_been_through_and_how_fast_it_flies() {
    let tunnels: Vec<_> = whole_flight().iter().map(|t| tunnel_of(t)).collect();
    // The press, then five ticks short of the entrance at 5 units a tick over 30: no tunnel
    // before the craft is at the portal.
    let approach = tunnels.iter().take_while(|t| t.is_none()).count();
    assert_eq!(approach, 6);
    let flown: Vec<(Rgba, f32, f32)> = tunnels[approach..].iter().map_while(|t| *t).collect();
    assert!(
        tunnels[approach + flown.len()..]
            .iter()
            .all(Option::is_none),
        "one unbroken run, then nothing"
    );
    for (rgba, ..) in &flown {
        assert_eq!(
            (rgba.r, rgba.g, rgba.b, rgba.a),
            (0x20, 0xE0, 0x70, 255),
            "2048's colour"
        );
    }
    // From the entrance the ticks count: 0 to the last tick out of the world, then on from two
    // past it, so no frame between two ticks shows the far side early.
    let phases: Vec<f32> = flown.iter().map(|(_, phase, _)| *phase).collect();
    let out_of_the_world: Vec<f32> = (0..TRANSIT_TICKS).map(f32::from).collect();
    let back: Vec<f32> = (1..=TAIL_TICKS)
        .map(|n| LAST + 1.0 + f32::from(n))
        .collect();
    assert_eq!(phases, [out_of_the_world, back].concat());
    // 360 km/h is 100 units a second: the tunnel is flown at what the craft went in at, to
    // the end, whatever it flies once it is out.
    assert!(
        flown
            .iter()
            .all(|(.., speed)| (speed - 100.0 / 60.0).abs() < 1e-4)
    );
}

#[test]
fn the_field_of_view_rides_one_curve_with_no_step_at_the_entrance_or_the_exit() {
    let flight = whole_flight();
    let widening: Vec<f32> = flight.iter().filter_map(|t| widening_of(t)).collect();
    assert_eq!(
        widening.len(),
        flight.len() - 1 - 3,
        "from the tick after the press to the end of the tail"
    );
    let peak = widening.iter().copied().fold(0.0, f32::max);
    assert!(
        (peak - FOV_PEAK).abs() < 0.01,
        "widest in the tunnel: {peak}"
    );
    let steepest = widening
        .windows(2)
        .map(|pair| (pair[1] - pair[0]).abs())
        .fold(0.0, f32::max);
    assert!(steepest < 2.5, "no tick moves it {steepest} degrees");
    assert!(widening[0] < 1.0, "it starts from nothing: {}", widening[0]);
    assert!(
        *widening.last().unwrap() < 0.1,
        "and is back there when the tunnel has gone: {:?}",
        widening.last()
    );
    // Still wide on the exit tick: the road at the far end was drawn at this field.
    let exit = widening.len() - usize::from(TAIL_TICKS);
    assert!(widening[exit] > 8.0, "{}", widening[exit]);
    assert!((widening[exit] - widening[exit - 1]).abs() < 1.0);
}

#[test]
fn the_shaders_header_is_the_guests_scalars_in_their_places() {
    // The defaults stand in for a view a scalar has not reached yet, so they are the guest's
    // own numbers, and the tunnel is a backdrop: the craft is drawn in front of it.
    let shader = include_str!("../../fx/wormhole.wgsl");
    assert!(shader.contains("//! fill = \"backdrop\""));
    let named = |name: &str| format!("//! name = {name:?}");
    let default = |name: &str| -> f32 {
        let at = shader.find(&named(name)).expect(name);
        shader[at..]
            .lines()
            .nth(1)
            .and_then(|line| line.strip_prefix("//! default = "))
            .and_then(|n| n.trim().parse().ok())
            .expect("a default under the name")
    };
    assert_eq!(default("transit"), LAST);
    assert_eq!(default("phase"), -1.0);
    let order: Vec<usize> = ["phase", "transit", "speed"]
        .iter()
        .map(|name| shader.find(&named(name)).unwrap())
        .collect();
    assert!(order.windows(2).all(|pair| pair[0] < pair[1]));
    assert_eq!(
        [PARAM_PHASE, PARAM_TRANSIT, PARAM_SPEED],
        [0, 1, 2],
        "the scalars by their place in the header"
    );
    // The portal's radius is one number in three places.
    assert!(shader.contains(&format!("const WH_RADIUS: f32 = {RADIUS:?};")));
    let effect = include_str!("../../fx/portal_pulse.toml");
    assert!(effect.contains(&format!("[0.9, {RADIUS:?}]")));
}

#[test]
fn another_mods_transit_ending_clears_no_tunnel() {
    let me = slot(0);
    let mut race = Race::<Slipstream>::new().with_craft(me, holder(true));
    race.event(Event::TransitEnd { slot: me });
    assert_eq!(tunnel_of(&race.tick()), None);
    assert_eq!(tunnel_of(&race.tick()), None);
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
    assert_eq!(
        transits(&race.tick()),
        0,
        "the dropped fire is not remembered"
    );
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

#[test]
fn pure_has_its_own_portal_effect_and_colour() {
    let pure = Look::of(title!("wipeout-pure"));
    assert_eq!((pure.effect, pure.rgb), (4, [0x40, 0xFF, 0x40]));
    for other in [
        Look::of(title!("wipeout-2048")),
        Look::of(title!("wipeout-hd")),
        Look::of(title!("wipeout-omega-collection")),
        Look::of(title!("wipeout-pulse")),
    ] {
        assert_ne!(other.effect, pure.effect);
    }
}

/// A person at 360 km/h (100 units a second) with the pickup, `at` along a 4000 unit lap.
fn flier(at: f32) -> Race<Slipstream> {
    let me = slot(0);
    Race::<Slipstream>::new()
        .with_craft(
            me,
            CraftRecord {
                speed_kmh: 360.0,
                ..holder(true)
            },
        )
        .with_course(4000.0, me, Some(at), 1)
}

/// Moves the craft on the lap to `at` without touching anything else.
fn move_to(race: &mut Race<Slipstream>, at: f32) {
    race.set_progress(slot(0), Some(at));
}

/// Ticks of a portal's life for a person at 360 km/h on a 4000 unit lap, pressing at 3900 and
/// flying 5 units a tick to the entrance at 3930, with the player's QUALITY pick `option`, as
/// the commands that are not the view.
fn portal_commands(option: u8) -> (Vec<Command>, Vec<Command>) {
    let me = slot(0);
    let mut race = flier(3900.0).with_options([option, 0, 0, 0]);
    race.event(Event::PickupFire { slot: me });
    race.event(fits(me, true));
    let mut all = race.tick();
    let mut at = 3900.0;
    while transits(&all) == 0 {
        at += 5.0;
        move_to(&mut race, at);
        all.extend(race.tick());
        assert!(at < 4000.0, "never went through");
    }
    *race.craft_mut(me) = Some(CraftRecord {
        pickup: Pickup::None,
        in_transit: true,
        speed_kmh: 360.0,
        ..holder(true)
    });
    // The exit's `TransitEnd` comes on the tick after these.
    for _ in 1..TRANSIT_TICKS {
        all.extend(race.tick());
    }
    let (views, rest) = all
        .into_iter()
        .partition(|c| matches!(c, Command::PortalView { .. }));
    (views, rest)
}

#[test]
fn the_view_looks_from_the_entrance_to_the_exit_the_transit_will_land_at() {
    let (views, _) = portal_commands(2);
    // 30 units ahead of 3900, and four seconds of 100 units a second past that, round the lap.
    let want = |generation| Command::PortalView {
        slot: slot(0),
        entrance: 3930.0,
        exit: Distance::Progress(330.0),
        radius: VIEW_RADIUS,
        update: ViewUpdate::Continuous,
        generation,
    };
    assert!(
        views.len() >= 60,
        "every tick the portal stands: {}",
        views.len()
    );
    // At High the craft going through looks from where it stands (the entrance, which it
    // reached exactly) for its whole transit, on the portal's second generation.
    let through = views.len() - usize::from(TRANSIT_TICKS) + 1;
    let wrong = views[..through].iter().position(|v| *v != want(2));
    assert_eq!(wrong, None, "{:?}", wrong.map(|at| views[at]));
    assert!(
        views[through..].iter().all(|v| *v == want(3)),
        "{:?}",
        views.last()
    );
}

#[test]
fn the_far_end_of_the_tunnel_looks_from_where_the_craft_stands() {
    let me = slot(0);
    let mut race = flier(3900.0).with_options([1, 0, 0, 0]);
    race.event(Event::PickupFire { slot: me });
    race.event(fits(me, true));
    race.tick();
    // It overshoots the entrance at 3930 by 4 units and is frozen there.
    assert_eq!(transits(&commands_of(&mut race, 3934.0)), 1);
    let record = CraftRecord {
        pickup: Pickup::None,
        in_transit: true,
        speed_kmh: 360.0,
        ..holder(true)
    };
    *race.craft_mut(me) = Some(record);
    let view = |commands: &[Command]| {
        commands.iter().find_map(|c| match c {
            Command::PortalView {
                entrance,
                exit,
                update,
                generation,
                ..
            } => Some((*entrance, *exit, *update, *generation)),
            _ => None,
        })
    };
    let mut seen = Vec::new();
    for _ in 1..TRANSIT_TICKS {
        seen.push(view(&race.tick()).expect("a view every tick"));
    }
    // Medium: the camera is on its seat `SEATED` ticks in, and the snapshot waits for it.
    let opened = usize::from(SEATED) - 1;
    assert!(
        seen[..opened]
            .iter()
            .all(|v| *v == (3930.0, Distance::Progress(330.0), ViewUpdate::Once, 2)),
        "the snapshot the portal opened with is held: {:?}",
        seen.first()
    );
    assert!(
        seen[opened..]
            .iter()
            .all(|v| *v == (3934.0, Distance::Progress(330.0), ViewUpdate::Once, 3)),
        "one more snapshot, from where it stands: {:?}",
        seen.last()
    );
}

#[test]
fn low_asks_for_no_view_medium_a_snapshot_high_every_frame_and_the_race_is_the_same() {
    let (low, low_rest) = portal_commands(0);
    let (medium, medium_rest) = portal_commands(1);
    let (high, high_rest) = portal_commands(2);
    assert!(low.is_empty(), "Low draws the shader's own surface");
    let update = |views: &[Command]| match views.first() {
        Some(Command::PortalView { update, .. }) => Some(*update),
        _ => None,
    };
    assert_eq!(update(&medium), Some(ViewUpdate::Once));
    assert_eq!(update(&high), Some(ViewUpdate::Continuous));
    // The pick moves what is drawn and nothing else: the shader's scalar aside, every command
    // is the same at the three.
    let plain = |rest: Vec<Command>| -> Vec<Command> {
        rest.into_iter()
            .filter(|c| !matches!(c, Command::SetShaderParam { .. }))
            .collect()
    };
    assert_eq!(plain(low_rest.clone()), plain(medium_rest.clone()));
    assert_eq!(plain(medium_rest), plain(high_rest.clone()));
    let scalar = |rest: &[Command]| {
        rest.iter().find_map(|c| match c {
            Command::SetShaderParam { shader, value, .. } if *shader == SHADER_PORTAL => {
                Some(*value)
            }
            _ => None,
        })
    };
    assert_eq!(
        [scalar(&low_rest), scalar(&high_rest)],
        [Some(0.0), Some(2.0)],
        "the shader is told the quality"
    );
}

#[test]
fn every_person_sees_the_portal_a_craft_opened_and_no_view_without_a_fix() {
    let (a, b, ai) = (slot(0), slot(1), slot(2));
    let mut race = Race::<Slipstream>::new()
        .with_craft(a, holder(true))
        .with_craft(b, holder(true))
        .with_craft(ai, holder(false))
        .with_course(4000.0, a, Some(100.0), 1)
        .with_options([2, 0, 0, 0]);
    race.event(Event::PickupFire { slot: ai });
    race.event(fits(ai, true));
    race.tick();
    let sent = race.tick();
    let looking: Vec<Slot> = sent
        .iter()
        .filter_map(|c| match c {
            Command::PortalView { slot, .. } => Some(*slot),
            _ => None,
        })
        .collect();
    assert_eq!(
        looking,
        [],
        "the AI's craft has no fix, so there is nowhere to look"
    );
    let mut race = Race::<Slipstream>::new()
        .with_craft(a, holder(true))
        .with_craft(b, holder(true))
        .with_craft(ai, holder(false))
        .with_course(4000.0, ai, Some(100.0), 1)
        .with_options([2, 0, 0, 0]);
    race.event(Event::PickupFire { slot: ai });
    race.event(fits(ai, true));
    race.tick();
    let looking: Vec<Slot> = race
        .tick()
        .iter()
        .filter_map(|c| match c {
            Command::PortalView { slot, .. } => Some(*slot),
            _ => None,
        })
        .collect();
    assert_eq!(looking, [a, b], "both people, not the AI");
}

fn commands_of(race: &mut Race<Slipstream>, at: f32) -> Vec<Command> {
    move_to(race, at);
    race.tick()
}

#[test]
fn the_craft_flies_to_the_portal_and_the_transit_starts_at_the_entrance_not_on_the_press() {
    let me = slot(0);
    let mut race = flier(3900.0).with_options([2, 0, 0, 0]);
    race.event(Event::PickupFire { slot: me });
    race.event(fits(me, true));
    let press = race.tick();
    assert_eq!(transits(&press), 0, "not on the press");
    assert!(
        !press
            .iter()
            .any(|c| matches!(c, Command::ClearPickup { .. })),
        "the pickup is spent at the entrance"
    );
    assert!(press.contains(&Command::FireModEffectAhead {
        slot: me,
        index: 3,
        distance: Distance::Units(AHEAD)
    }));
    assert!(press.contains(&Command::PlayModSound {
        slot: me,
        index: SOUND_OPEN,
        mode: SoundMode::Once
    }));
    for at in [3905.0, 3915.0, 3929.0] {
        let flying = commands_of(&mut race, at);
        assert_eq!(transits(&flying), 0, "still short of the entrance at {at}");
        assert!(
            flying
                .iter()
                .any(|c| matches!(c, Command::PortalView { .. })),
            "the firer sees the window at {at}"
        );
        assert_eq!(tunnel_of(&flying), None, "no tunnel before the entrance");
        assert!(
            widening_of(&flying).is_some(),
            "the field is widening at {at}"
        );
    }
    let arrival = commands_of(&mut race, 3931.0);
    // The exit stands 400 units on from the entrance at 3930: 4330 is 330 on the lap.
    assert!(arrival.contains(&Command::Transit {
        slot: me,
        ticks: TRANSIT_TICKS,
        distance: Distance::Units(399.0),
        exit_factor: EXIT_FACTOR
    }));
    assert!(arrival.contains(&Command::ClearPickup { slot: me }));
    assert!(arrival.contains(&Command::FireModEffectAhead {
        slot: me,
        index: 3,
        distance: Distance::Units(399.0)
    }));
    assert!(arrival.contains(&Command::PlayModSound {
        slot: me,
        index: SOUND_TRANSIT,
        mode: SoundMode::Loop
    }));
}

#[test]
fn the_entrance_across_the_line_is_reached_by_wrapping_not_by_the_number() {
    let me = slot(0);
    let mut race = flier(3990.0);
    race.event(Event::PickupFire { slot: me });
    race.event(fits(me, true));
    race.tick();
    assert_eq!(transits(&commands_of(&mut race, 3995.0)), 0);
    assert_eq!(
        transits(&commands_of(&mut race, 10.0)),
        0,
        "10 is short of 20"
    );
    assert_eq!(transits(&commands_of(&mut race, 21.0)), 1);
}

#[test]
fn a_craft_that_never_arrives_closes_the_portal_and_keeps_the_pickup() {
    let me = slot(0);
    let mut race = flier(3900.0);
    race.event(Event::PickupFire { slot: me });
    race.event(fits(me, true));
    let mut all = race.tick();
    for _ in 0..usize::from(APPROACH_LIMIT) + 2 {
        all.extend(race.tick());
    }
    assert_eq!(transits(&all), 0);
    assert!(!all.iter().any(|c| matches!(c, Command::ClearPickup { .. })));
    // Closed: the next press opens a portal again.
    race.event(Event::PickupFire { slot: me });
    race.event(fits(me, true));
    race.tick();
    let again = commands_of(&mut race, 3935.0);
    assert_eq!(transits(&again), 1, "the kept pickup fires again");
}

#[test]
fn a_craft_that_loses_the_pickup_on_the_way_closes_the_portal() {
    let me = slot(0);
    let mut race = flier(3900.0);
    race.event(Event::PickupFire { slot: me });
    race.event(fits(me, true));
    race.tick();
    *race.craft_mut(me) = Some(CraftRecord {
        racing: false,
        speed_kmh: 360.0,
        ..holder(true)
    });
    assert_eq!(transits(&commands_of(&mut race, 3940.0)), 0, "wrecked");
}

#[test]
fn another_fire_waits_while_the_first_is_still_flying_to_the_portal() {
    let (a, b) = (slot(0), slot(1));
    let mut race = flier(3900.0).with_craft(b, holder(false));
    race.event(Event::PickupFire { slot: a });
    race.event(Event::PickupFire { slot: b });
    race.event(fits(a, true));
    race.event(fits(b, true));
    let press = race.tick();
    assert!(!press.iter().any(|c| matches!(c, Command::Transit { .. })));
    assert_eq!(
        transits(&commands_of(&mut race, 3935.0)),
        1,
        "the first goes"
    );
}

#[test]
fn the_manifest_declares_the_state_the_guest_keeps() {
    let manifest = include_str!("../../oag-mod.toml");
    let declared: usize = manifest
        .lines()
        .find_map(|line| line.strip_prefix("state_bytes = "))
        .and_then(|n| n.trim().parse().ok())
        .expect("a state_bytes line");
    assert_eq!(declared, <Portal as oag_ext::Blob>::BYTES);
}
