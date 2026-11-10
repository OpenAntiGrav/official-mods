//! blend = "alpha"
//!
//! [[param]]
//! name = "quality"
//! default = 2.0

// The portal's surface: a half disc standing on the road with the road at the exit seen
// through it, or, where the engine has no view for this craft (Low, the second seat of a
// split, a portal not yet in sight), a surface that moves by itself in the title's colour.
// `quality` is 0, 1 or 2 (Low, Medium, High): Medium adds a slow shimmer to the held view so a
// snapshot does not sit dead on the road.
//
// The particle's colour is the title's; the effect file starts it white, and the white in it
// is the flash the portal opens with.

fn surface(p: vec2<f32>, t: f32) -> f32 {
    let r = length(p);
    let a = atan2(p.y, p.x);
    let warp = sin(a * 3.0 + t * 1.3 + r * 5.0) * 0.35;
    let ring = sin(r * 16.0 - t * 5.0 + warp * 4.0);
    let arm = sin(a * 5.0 - r * 7.0 + t * 2.2 + warp * 3.0);
    let grain = sin(p.x * 23.0 + t * 3.1) * sin(p.y * 19.0 - t * 2.3);
    return 0.5 + 0.28 * ring + 0.17 * arm + 0.08 * grain;
}

fn mod_fragment(input: ModInput) -> vec4<f32> {
    // The quad is centred on the road: the disc's upper half stands above it, y down.
    let p = (input.uv - vec2<f32>(0.5)) * 2.0;
    let r = length(p);
    let shape = (1.0 - smoothstep(0.96, 1.0, r)) * (1.0 - smoothstep(-0.03, 0.0, p.y));
    let lowest = min(input.colour.r, min(input.colour.g, input.colour.b));
    let flash = clamp((lowest - 0.3) / 0.7, 0.0, 1.0);
    let tint = input.colour.rgb;
    // The rim: a band of light with brighter arcs running round it.
    let arcs = 0.5 + 0.5 * sin(atan2(p.y, p.x) * 7.0 + input.time * 6.0 + r * 9.0);
    let band = mix(tint * 1.25, vec3<f32>(1.0), 0.55 * arcs * arcs);
    let rim = smoothstep(0.8, 0.94, r);
    var rgb: vec3<f32>;
    var alpha = shape * input.colour.a;
    if engine_texture_bound() {
        // Rings run out across the surface and bend the view a little as they pass.
        let wave = sin(r * 26.0 - input.time * 7.0) * 0.004 * smoothstep(0.25, 0.9, r);
        let medium = clamp(1.0 - abs(param_quality() - 1.0), 0.0, 1.0);
        let shimmer = 1.0 + 0.06 * medium * sin(r * 30.0 - input.time * 4.0);
        let seen = engine_texture(engine_uv(input.world) + p / max(r, 0.001) * wave).rgb * shimmer;
        rgb = mix(seen, tint, 0.2 * smoothstep(0.45, 0.85, r));
    } else {
        let n = surface(p, input.time);
        rgb = tint * (0.45 + 0.9 * n) + vec3<f32>(n * n * n * n * 0.5);
        alpha = alpha * (0.6 + 0.35 * n);
    }
    rgb = mix(rgb, band, rim * 0.95);
    alpha = mix(alpha, shape * input.colour.a, rim);
    return vec4<f32>(mix(rgb, vec3<f32>(1.0), flash * (0.35 + 0.5 * rim)), alpha);
}
