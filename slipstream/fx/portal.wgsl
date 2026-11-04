//! blend = "alpha"
//! texture = "fx/portal.png"
//!
//! [[param]]
//! name = "quality"
//! default = 2.0

// The portal's surface: the half disc of fx/portal.png with the road at the exit seen through
// it, or, where the engine has no view for this craft (Low, the second seat of a split, a
// portal not yet in sight), a surface that moves by itself in the title's colour.
// `quality` is 0, 1 or 2 (Low, Medium, High): Medium adds a slow shimmer to the held view so a
// snapshot does not sit dead on the road.

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
    let art = mod_texture(input.uv);
    // The art's flat edge is on the middle row: the disc's centre is the middle of the quad.
    let p = (input.uv - vec2<f32>(0.5)) * 2.0;
    let r = length(p);
    let shape = smoothstep(0.0, 0.3, art.a) * input.colour.a;
    let tint = input.colour.rgb;
    if !engine_texture_bound() {
        let n = surface(p, input.time);
        let glow = n * n * n * n * 0.5;
        let rgb = tint * (0.45 + 0.9 * n) + vec3<f32>(glow);
        return vec4<f32>(rgb, shape * (0.6 + 0.35 * n));
    }
    let medium = clamp(1.0 - abs(param_quality() - 1.0), 0.0, 1.0);
    let shimmer = 1.0 + 0.06 * medium * sin(r * 30.0 - input.time * 4.0);
    let seen = engine_texture(engine_uv(input.world)).rgb * shimmer;
    let edge = smoothstep(0.78, 0.98, r);
    return vec4<f32>(mix(seen, tint * 1.2, edge * 0.8), shape);
}
