//! blend = "alpha"
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
//!
//! [[param]]
//! name = "tail"
//! default = 24.0

// The short tunnel between the two portals, over the view of the craft flying it. Going through
// the entrance draws the portal's border out behind the craft, as fast as the craft flies, into
// a tunnel's wall; the exit stands at the far end of it, the road there seen through it at its
// own size, like a window that is only further off. Then the craft closes on the far end and
// flies out through it.
//
// The guest (guest/src/lib.rs, `Flight`) says where the flight is:
//
// - `phase`: ticks since the entrance, 0 to `transit` while the craft is out of the world and
//   `transit + 2` on once it is out (no tick is `transit + 1`), for `tail` ticks.
// - `speed`: what the craft flies, units a tick. Everything spatial here is that speed: how
//   fast the far end draws away, how fast the wall runs past, how fast the far end comes up.
//
// The far end shows the engine's view through the portal pair where this view has one, and
// light where it has none (PORTAL VIEW Low, the second seat of a split). The engine seats the
// camera of a craft in a transit on the framing it has at the exit, so that picture drawn at its
// own size is the one the camera has on the exit tick: there is no cut to cover.
//
// What moves with the ticks reads `phase + input.between`, so it does not step at the tick on a
// display faster than the simulation.

const WH_TAU: f32 = 6.2831853;
// The tunnel's middle, in half heights from the view's, y down: a little above it, where the
// road ahead is.
const WH_CENTRE: vec2<f32> = vec2<f32>(0.0, -0.1);
// The far end's radius when it is as near as a portal is to the camera of a craft at it, half
// heights: at the entrance, and again on the last tick out of the world.
const WH_MOUTH: f32 = 0.9;
// Its radius once the tunnel is drawn out: it gets no smaller than this, only further.
const WH_HOLD: f32 = 0.5;
// How far a chase camera stands behind its craft, units: the far end is at `WH_MOUTH` from there.
const WH_CAMERA: f32 = 14.0;
// Ticks the tunnel takes to come up over the view at the entrance.
const WH_IN: f32 = 3.0;
// Units between two of the rings that run down the wall.
const WH_RINGS: f32 = 12.0;
// How much light stands in the far end as the craft reaches it, most in its middle, where the
// craft comes out of it.
const WH_FLASH: f32 = 0.6;

fn wh_smooth(x: f32) -> f32 {
    let t = clamp(x, 0.0, 1.0);
    return t * t * (3.0 - 2.0 * t);
}

fn wh_hash(cell: vec2<f32>) -> f32 {
    return fract(sin(dot(cell, vec2<f32>(127.1, 311.7))) * 43758.5453);
}

// Value noise that closes on itself every `wrap` cells along x: round the tunnel.
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

// The wall at `angle` round the tunnel, `depth` units ahead of the eye and `run` units along
// the border as it flies past, `far` being 1 at the far end and less towards the eye: the
// portal's rim (fx/portal.wgsl's band and arcs) drawn out along the tunnel, with rings and
// streaks to fly by.
fn wh_wall(angle: f32, depth: f32, run: f32, far: f32, time: f32, tint: vec3<f32>) -> vec3<f32> {
    let arcs = 0.5 + 0.5 * sin(angle * 7.0 + time * 1.5 + depth * 0.04);
    let band = mix(tint * 1.1, vec3<f32>(1.0), 0.5 * arcs * arcs);
    let ring = pow(0.5 + 0.5 * cos(WH_TAU * run / WH_RINGS), 10.0);
    let streak = wh_noise(vec2<f32>(angle / WH_TAU * 48.0, run / 36.0), 48.0);
    let fine = wh_noise(vec2<f32>(angle / WH_TAU * 96.0 + 7.0, run / 14.0), 96.0);
    let shade = 0.1 + 0.22 * streak * streak + 0.16 * fine * fine * fine + 0.8 * ring;
    // The light is at the far end.
    let lit = band * shade * mix(0.5, 1.0, far);
    return lit + mix(tint, vec3<f32>(1.0), 0.6) * 0.25 * pow(far, 10.0);
}

fn mod_fragment(input: ModInput) -> vec4<f32> {
    // The view's shape, from how fast `uv` crosses a texel each way.
    let aspect = abs(dpdy(input.uv.y)) / max(abs(dpdx(input.uv.x)), 0.000001);
    let q = (input.uv - vec2<f32>(0.5)) * 2.0;
    let off = vec2<f32>(q.x * aspect, q.y) - WH_CENTRE;
    let r = length(off);

    let tint = input.colour.rgb;
    let hot = mix(tint, vec3<f32>(1.0), 0.8);
    let transit = param_transit();
    let speed = max(param_speed(), 0.05);
    let tail = max(param_tail(), 1.0);
    let ticks = max(param_phase() + input.between, 0.0);
    // Ticks out of the world, and ticks since coming out.
    let gone = min(ticks, transit);
    let since = max(ticks - transit - 1.0, 0.0);
    let is_out = ticks > transit + 0.5;

    // The far end's radius. The border is drawn out behind the craft at the craft's speed, so
    // the far end draws away as a ring does that one flies from; it stops at `WH_HOLD`; and it
    // comes up as a ring does that one flies at, to `WH_MOUTH` on the last tick out of the world
    // and on past the screen's edge after it.
    let drawn = speed * gone;
    let to_go = speed * (transit - gone - since);
    let leaving = WH_MOUTH / (1.0 + drawn / WH_CAMERA);
    let coming = WH_MOUTH / max(1.0 + to_go / WH_CAMERA, 0.02);
    let mouth = pow(pow(leaving, 6.0) + pow(coming, 6.0) + pow(WH_HOLD, 6.0), 1.0 / 6.0);
    let edge = 0.02 + 0.03 * mouth;
    let inside = 1.0 - smoothstep(mouth - edge, mouth, r);

    // The tunnel comes up over the world at the entrance and is gone in the tail.
    let up = wh_smooth(gone / WH_IN);
    let fading = 1.0 - smoothstep(0.15, 0.8, since / tail);

    // The wall, from the far end out to the eye. The far end is as far off as the border has
    // been drawn out, or as there is left to fly: its size holds while that grows, and the
    // rings crowding towards it are what says it is further.
    let ahead = pow(pow(drawn + 0.01, -4.0) + pow(max(to_go, 0.0) + 0.01, -4.0), -0.25);
    let far = mouth / max(r, mouth);
    let depth = (WH_CAMERA + ahead) * far;
    let run = depth + speed * (gone + since);
    let walls_rgb = wh_wall(atan2(off.y, off.x), depth, run, far, input.time, tint)
        * mix(1.0, 0.6, smoothstep(1.2, 2.4, r));
    let walls_a = up * fading;

    // What stands in the far end.
    var middle_rgb = hot;
    var middle_a = up;
    let flash = WH_FLASH * (0.15 + 0.85 * exp(-r * r * 7.0));
    let nearing = smoothstep(6.0, 0.0, transit - gone);
    if is_out {
        // The craft is through: the world is there, and the light it came out of, dying.
        middle_a = flash * exp(-since / 5.0) * (1.0 - wh_smooth(since / tail));
    } else if engine_texture_bound() {
        // The road at the exit, at its own size.
        middle_rgb = mix(engine_texture(input.screen_uv).rgb, hot, flash * nearing);
    } else {
        // No picture of the exit: light at the end of the tunnel, whiter as it comes up.
        let glow = 0.5 + 0.5 * exp(-r * r / (mouth * mouth) * 1.5);
        middle_rgb = mix(hot * glow, vec3<f32>(1.0), 0.25 + 0.75 * nearing);
    }

    // The two side by side, then the far end's bright rim over both.
    let base_a = walls_a * (1.0 - inside) + middle_a * inside;
    let base_rgb = (walls_rgb * walls_a * (1.0 - inside) + middle_rgb * middle_a * inside)
        / max(base_a, 0.0001);
    let width = 0.025 + 0.035 * mouth;
    let ring = exp(-(r - mouth) * (r - mouth) / (width * width));
    let rim_a = ring * up * (1.0 - wh_smooth(since / (tail * 0.7))) * 0.85;
    let alpha = rim_a + base_a * (1.0 - rim_a);
    let rgb = (hot * rim_a + base_rgb * base_a * (1.0 - rim_a)) / max(alpha, 0.0001);
    return vec4<f32>(rgb, alpha);
}
