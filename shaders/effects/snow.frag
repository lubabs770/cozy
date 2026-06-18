#version 300 es
precision highp float;

// cozy effect: "snow" — multi-layer parallax snow with depth-of-field.
//
// Ported from "Just Snow" by Andrew Baldwin (twitter @baldand, www.thndl.com),
// 2013: https://www.shadertoy.com/view/ldsGDn
//
// The original is licensed Creative Commons Attribution-NonCommercial-ShareAlike
// 3.0 (CC BY-NC-SA 3.0). This ported file inherits that license and is therefore
// CC BY-NC-SA 3.0 — NOT the MIT license that covers the rest of cozy.
// Attribution: Andrew Baldwin. See README for details.
//
// Adapted to cozy's uniform contract and conventions:
//   * iChannel0 -> u_wallpaper (sampled cover-fit), iTime -> u_time
//   * u_intensity scales how much snow accumulates (weather)
//   * u_wind slants the falling flakes horizontally (weather)
//   * u_overlay outputs premultiplied alpha so the flakes can composite over an
//     external wallpaper daemon (transparent between flakes)

in vec2 v_uv;
out vec4 frag_color;

uniform vec2      u_resolution;
uniform vec2      u_tex_resolution;
uniform sampler2D u_wallpaper;
uniform float     u_time;
uniform float     u_wind;
uniform float     u_intensity;
uniform bool      u_overlay;

#define LAYERS 50
#define DEPTH  0.1
#define WIDTH  0.8
#define SPEED  0.6

vec2 cover_uv(vec2 uv, vec2 res, vec2 tex_res) {
    float scale  = max(res.x / tex_res.x, res.y / tex_res.y);
    vec2  scaled = tex_res * scale;
    vec2  offset = (scaled - res) * 0.5;
    return (uv * res + offset) / scaled;
}

void main() {
    float intensity = clamp(u_intensity, 0.0, 1.0);
    vec3  base      = texture(u_wallpaper, cover_uv(v_uv, u_resolution, u_tex_resolution)).rgb;

    // Baldwin's hashing matrix and a slowly oscillating focal depth for the DoF.
    const mat3 p = mat3(13.323122, 23.5112, 21.71123,
                        21.1212,   28.7312, 11.9312,
                        21.8112,   14.7212, 61.3934);
    float dof = 5.0 * sin(u_time * 0.1);

    // Aspect-correct so flakes stay round on a wide output.
    float aspect = u_resolution.x / u_resolution.y;
    vec2  uv     = vec2(v_uv.x * aspect, v_uv.y);

    float acc = 0.0;
    for (int i = 0; i < LAYERS; i++) {
        float fi = float(i);

        // Each layer sits at a different parallax depth and drifts sideways; the
        // wind adds a shared slant on top of the per-layer random direction.
        vec2 q = uv * (1.0 + fi * DEPTH);
        q += vec2(q.y * (WIDTH * mod(fi * 7.238917, 1.0) - WIDTH * 0.5 + u_wind * 1.1),
                  SPEED * u_time / (1.0 + fi * DEPTH * 0.03));

        vec3 n  = vec3(floor(q), 31.189 + fi);
        vec3 m  = floor(n) * 0.00001 + fract(n);
        vec3 mp = (31415.9 + m) / fract(p * m);
        vec3 r  = fract(mp);

        vec2 s = abs(mod(q, 1.0) - 0.5 + 0.9 * r.xy - 0.45);
        s += 0.01 * abs(2.0 * fract(10.0 * q.yx) - 1.0);
        float d    = 0.6 * max(s.x - s.y, s.x + s.y) + max(s.x, s.y) - 0.01;
        float edge = 0.005 + 0.05 * min(0.5 * abs(fi - 5.0 - dof), 1.0);
        acc += smoothstep(edge, -edge, d) * (r.x / (1.0 + 0.02 * fi * DEPTH));
    }

    // Weather scales the overall accumulation; tint the flakes a cool white.
    float snow  = clamp(acc * mix(0.45, 1.15, intensity), 0.0, 1.0);
    vec3  flake = vec3(0.92, 0.96, 1.0) * snow;

    if (u_overlay) {
        // Premultiplied alpha: rgb is already the flake colour scaled by coverage.
        frag_color = vec4(flake, snow);
    } else {
        frag_color = vec4(base + flake, 1.0);
    }
}
