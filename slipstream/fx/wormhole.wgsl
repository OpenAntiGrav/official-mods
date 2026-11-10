//! blend = "alpha"
//! fill = "backdrop"
//!
//! [[param]]
//! name = "phase"
//! default = -1.0
//!
//! [[param]]
//! name = "transit"
//! default = 59.0
//!
//! [[param]]
//! name = "speed"
//! default = 2.0

// The portal is a tunnel: its half disc, standing on the road, drawn out along the way the
// craft flies. The craft goes in through the mouth and the camera after it, the track still
// there round the mouth until the camera is through; the exit moves back down the tunnel as
// the craft goes in, then the craft closes on it and flies out through it.
//
// A backdrop: drawn behind the view's own craft, which the engine keeps drawing in front of
// its camera while it is out of the world, and in place of the world wherever the tunnel is
// at least half there. Its glow, and what is left of it once the craft is out, are under that
// and hide nothing.
// The tunnel is real geometry round that craft (`craft_frame`), cut by each texel's ray
// (`eye_ray`): an arch of `WH_RADIUS` over a floor `WH_FLOOR` under the craft, from the mouth
// behind it to the far end ahead of it.
//
// The guest (guest/src/lib.rs, `Flight`) says where the flight is:
//
// - `phase`: ticks since the entrance, 0 to `transit` while the craft is out of the world and
//   `transit + 2` on once it is out (no tick is `transit + 1`), while what is left of the
//   tunnel falls behind.
// - `speed`: what the craft flies, units a tick: how fast the mouth falls behind, the far end
//   moves back, and the wall runs past.
//
// The far end shows the engine's view through the portal pair where this view has one, at its
// own size, and light where it has none (PORTAL VIEW Low, the second seat of a split). The
// engine frames that view from the camera's seat at the exit, so on the last tick out of the
// world it is the picture the camera has a tick later, and the far end is then where the exit
// portal stands round the craft: there is no cut.
//
// What moves with the ticks reads `phase + input.between`, so it does not step at the tick on a
// display faster than the simulation.

const WH_TAU: f32 = 6.2831853;
// The portal's radius, units: `RADIUS` of the guest and `size` of fx/portal_<title>.toml.
const WH_RADIUS: f32 = 12.0;
// How far under the craft the tunnel's floor is, units: a craft flies 3 off the road and the
// portal stands half a unit off it.
const WH_FLOOR: f32 = 2.5;
// Ticks the exit moves back for, at the craft's speed, before the craft starts to close on it.
const WH_BACK: f32 = 10.0;
// Units between two of the rings that run down the wall.
const WH_RINGS: f32 = 12.0;

fn wh_hash(cell: vec2<f32>) -> f32 {
    return fract(sin(dot(cell, vec2<f32>(127.1, 311.7))) * 43758.5453);
}

// Value noise that closes on itself every `wrap` cells along x.
fn wh_noise(at: vec2<f32>, wrap: f32) -> f32 {
    let cell = floor(at);
    let f = fract(at);
    let u = f * f * (3.0 - 2.0 * f);
    let x0 = cell.x - wrap * floor(cell.x / wrap);
    let x1 = (x0 + 1.0) - wrap * floor((x0 + 1.0) / wrap);
    let a = wh_hash(vec2<f32>(x0, cell.y));
    let b = wh_hash(vec2<f32>(x1, cell.y));
    let c = wh_hash(vec2<f32>(x0, cell.y + 1.0));
    let d = wh_hash(vec2<f32>(x1, cell.y + 1.0));
    return mix(mix(a, b, u.x), mix(c, d, u.x), u.y);
}

// The smaller of two lengths, with no corner where they cross.
fn wh_least(a: f32, b: f32) -> f32 {
    return pow(pow(max(a, 0.0) + 0.01, -4.0) + pow(max(b, 0.0) + 0.01, -4.0), -0.25);
}

// How far outside the portal's outline a point of its plane is, units: negative inside the
// half disc, `at` being across the road and up from the floor.
fn wh_outside(at: vec2<f32>) -> f32 {
    return max(length(at) - WH_RADIUS, -at.y);
}

// The tunnel's wall or floor at `at` (across, up from the floor, ahead of the craft), `run`
// being how far along the border that point is as it flies past: the portal's rim
// (fx/portal.wgsl's band and arcs) drawn out along the tunnel, with rings to fly by.
fn wh_wall(at: vec3<f32>, on_floor: bool, run: f32, time: f32, tint: vec3<f32>) -> vec3<f32> {
    let ring = pow(0.5 + 0.5 * cos(WH_TAU * run / WH_RINGS), 10.0);
    // The seam where the arch stands on the floor.
    let seam = exp(-(WH_RADIUS - abs(at.x)) * (WH_RADIUS - abs(at.x)) / 0.5) * exp(-at.y * at.y / 0.5);
    if on_floor {
        let lane = exp(-at.x * at.x / 0.08) + 0.5 * exp(-(abs(at.x) - 6.0) * (abs(at.x) - 6.0) / 0.05);
        let grain = wh_noise(vec2<f32>(at.x * 1.5 + 64.0, run / 9.0), 128.0);
        let shade = 0.05 + 0.1 * grain * grain + 0.4 * ring + 0.35 * lane;
        // The craft's engines light the floor behind it.
        let wake = exp(-at.x * at.x / 3.0) * smoothstep(-16.0, -3.0, at.z) * (1.0 - smoothstep(-3.0, 1.0, at.z));
        return tint * (shade + 0.5 * wake) + vec3<f32>(seam * 0.6 + 0.25 * wake);
    }
    return wh_arch(at, ring, seam, run, time, tint);
}

// The arch's share of `wh_wall`.
fn wh_arch(at: vec3<f32>, ring: f32, seam: f32, run: f32, time: f32, tint: vec3<f32>) -> vec3<f32> {
    let turn = atan2(at.y, at.x) / WH_TAU;
    let arcs = 0.5 + 0.5 * sin(turn * WH_TAU * 7.0 + time * 1.5 + run * 0.04);
    let band = mix(tint * 1.1, vec3<f32>(1.0), 0.5 * arcs * arcs);
    let streak = wh_noise(vec2<f32>(turn * 96.0, run / 30.0), 96.0);
    let fine = wh_noise(vec2<f32>(turn * 192.0 + 7.0, run / 11.0), 192.0);
    let shade = 0.1 + 0.22 * streak * streak + 0.16 * fine * fine * fine + 0.8 * ring;
    return band * shade + vec3<f32>(seam * 0.6);
}

fn mod_fragment(input: ModInput) -> vec4<f32> {
    let tint = input.colour.rgb;
    let hot = mix(tint, vec3<f32>(1.0), 0.8);
    let transit = param_transit();
    let speed = max(param_speed(), 0.05);
    let ticks = max(param_phase() + input.between, 0.0);
    // Ticks out of the world, and ticks since coming out.
    let gone = min(ticks, transit);
    let since = max(ticks - transit - 1.0, 0.0);
    let is_out = ticks > transit + 0.5;

    // The tunnel's own axes, from the craft: across, up from the floor, ahead.
    let craft = craft_frame();
    let across = craft[0].xyz;
    let up = craft[1].xyz;
    let ahead = -craft[2].xyz;
    let ray = eye_ray(input.screen_uv);
    let eye = vec3<f32>(-dot(craft[3].xyz, across), WH_FLOOR - dot(craft[3].xyz, up), -dot(craft[3].xyz, ahead));
    let dir = vec3<f32>(dot(ray, across), dot(ray, up), dot(ray, ahead));

    // The mouth falls behind the craft as fast as the craft flies. The exit moves back ahead
    // of it at that speed for `WH_BACK` ticks, then the craft closes on it at a steady rate and
    // is at it on the last tick out of the world; after that it falls behind too.
    let flown = speed * (gone + since);
    // The frame is the craft's on its exit tick carried back, a tick's flight past the portal.
    let mouth = -flown - speed;
    let closing = speed * WH_BACK * (transit - gone) / max(transit - WH_BACK, 1.0);
    let far = select(wh_least(speed * gone, closing) - 0.01, -speed * since, is_out);

    // Where the ray comes into the tunnel: through the mouth, for an eye still before it.
    var start = eye;
    var entry = -1.0;
    if eye.z < mouth {
        if dir.z <= 0.0 {
            return vec4<f32>(0.0);
        }
        start = eye + dir * ((mouth - eye.z) / dir.z);
        entry = wh_outside(start.xy);
        // The mouth's rim stands a little outside the outline too.
        if entry > 1.0 {
            return vec4<f32>(0.0);
        }
    }
    let rim = exp(-entry * entry / 0.12) * step(-0.5, entry);
    let through = 1.0 - smoothstep(-0.08, 0.08, entry);

    // Then where it ends: on the floor, on the arch, or at the far end.
    let base = start.xy;
    let slope = dir.xy;
    let a = max(dot(slope, slope), 0.000001);
    let b = dot(base, slope);
    let c = dot(base, base) - WH_RADIUS * WH_RADIUS;
    let to_arch = (-b + sqrt(max(b * b - a * c, 0.0))) / a;
    let to_floor = select(1.0e9, start.y / max(-dir.y, 0.000001), dir.y < 0.0);
    let to_wall = min(to_arch, to_floor);
    let to_far = select(1.0e9, max(far - start.z, 0.0) / max(dir.z, 0.000001), dir.z > 0.0);

    var rgb = hot;
    var alpha = through;
    if to_far <= to_wall {
        // The far end: the exit.
        let at = start + dir * to_far;
        let edge = exp(-wh_outside(at.xy) * wh_outside(at.xy) / 0.25);
        if is_out {
            // The craft is through and the world is there: only the rim it flew out of, going.
            alpha = through * edge * 0.45 * exp(-since * 0.5);
        } else if engine_texture_bound() {
            rgb = mix(engine_texture(input.screen_uv).rgb, hot, edge * 0.85);
        } else {
            let glow = 0.55 + 0.45 * exp(-dot(at.xy, at.xy) / (WH_RADIUS * WH_RADIUS) * 2.0);
            rgb = mix(hot * glow, hot, edge * 0.85);
        }
    } else {
        let at = start + dir * to_wall;
        // The wall ahead of the craft is drawn as long as the flight still is: the exit holds
        // its size while that is long, and the rings crowding towards it say how far it is.
        let whole = max(far, wh_least(speed * gone, speed * (transit - gone)));
        let drawn = select(at.z, at.z / max(far, 0.01) * whole, at.z > 0.0 && !is_out);
        let wall = wh_wall(at, to_floor < to_arch, drawn + flown, input.time, tint);
        // Light from the far end along the last of the wall, while the exit is ahead.
        let lit = exp(-(far - at.z) * (far - at.z) / 6.0) * exp(-since);
        rgb = wall * (0.75 + 0.5 * lit) + hot * lit * 0.4;
        // What is left of the tunnel behind the craft once it is out goes as it falls behind.
        alpha = through * select(1.0, 0.45 * exp(-since * 0.4), is_out);
    }
    let seen = max(alpha, rim * 0.9);
    return vec4<f32>(mix(rgb, hot, rim * 0.9 * (1.0 - alpha * 0.2)), seen);
}
