#version 300 es
precision highp float;

// cozy effect: "stratus" — a low, flat, featureless overcast layer. No discrete
// clouds, just a soft uniform grey haze with faint slow-moving mottling, the way
// a properly dreary day looks. Built from a low-frequency, low-contrast fbm so
// the whole sky reads as one continuous sheet rather than puffs. Hand-built.
// MIT license (same as cozy).
//
// u_wind:      how fast the haze creeps sideways (weather)
// u_intensity: thickness of the overcast, 0 = thin veil / 1 = solid grey (weather)

in vec2 v_uv;
out vec4 frag_color;

uniform vec2      u_resolution;
uniform vec2      u_tex_resolution;
uniform sampler2D u_wallpaper;
uniform float     u_time;
uniform float     u_wind;
uniform float     u_intensity;
uniform bool      u_overlay;

const mat2 m = mat2(1.6, 1.2, -1.2, 1.6);

vec2 cover_uv(vec2 uv, vec2 res, vec2 tex_res) {
    float scale  = max(res.x / tex_res.x, res.y / tex_res.y);
    vec2  scaled = tex_res * scale;
    vec2  offset = (scaled - res) * 0.5;
    return (uv * res + offset) / scaled;
}

vec2 hash(vec2 p) {
    p = vec2(dot(p, vec2(127.1, 311.7)), dot(p, vec2(269.5, 183.3)));
    return -1.0 + 2.0 * fract(sin(p) * 43758.5453123);
}

float noise(vec2 p) {
    const float K1 = 0.366025404;
    const float K2 = 0.211324865;
    vec2  i = floor(p + (p.x + p.y) * K1);
    vec2  a = p - i + (i.x + i.y) * K2;
    vec2  o = (a.x > a.y) ? vec2(1.0, 0.0) : vec2(0.0, 1.0);
    vec2  b = a - o + K2;
    vec2  c = a - 1.0 + 2.0 * K2;
    vec3  h = max(0.5 - vec3(dot(a, a), dot(b, b), dot(c, c)), 0.0);
    vec3  n = h * h * h * h * vec3(dot(a, hash(i + 0.0)), dot(b, hash(i + o)), dot(c, hash(i + 1.0)));
    return dot(n, vec3(70.0));
}

float fbm(vec2 n) {
    float total = 0.0, amplitude = 0.5;
    for (int i = 0; i < 5; i++) {
        total += noise(n) * amplitude;
        n = m * n;
        amplitude *= 0.5;
    }
    return total;
}

void main() {
    float intensity = clamp(u_intensity, 0.0, 1.0);

    vec2 sp = v_uv * vec2(u_resolution.x / u_resolution.y, 1.0);
    // Low spatial frequency + slow creep: the sheet barely moves.
    sp.x += u_time * (0.004 + u_wind * 0.02);

    // Gentle low-contrast mottling around a mid value.
    float n = fbm(sp * 0.8);
    float mottle = 0.5 + 0.35 * n;

    // Overcast coverage: a near-solid veil whose opacity scales with intensity.
    float alpha = clamp(mix(0.25, 0.92, intensity) * mottle, 0.0, 1.0);

    // Flat, slightly cool grey; faintly brighter where the layer thins.
    vec3 cloud = mix(vec3(0.55, 0.57, 0.60), vec3(0.80, 0.81, 0.83), mottle);

    if (u_overlay) {
        frag_color = vec4(cloud * alpha, alpha);
    } else {
        vec3 base = texture(u_wallpaper, cover_uv(v_uv, u_resolution, u_tex_resolution)).rgb;
        frag_color = vec4(mix(base, cloud, alpha), 1.0);
    }
}
