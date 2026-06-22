#version 300 es
precision highp float;

// cozy effect: "cumulonimbus" — heavy, towering storm clouds. A dense, dark
// billowing mass that sits low and broods, brightest at the high cauliflower
// tops and almost black along the rain-heavy base. The natural companion to the
// "lightning" effect. Built from billowy turbulence with a vertical density
// bias (more cloud toward the bottom of the frame) and a dark storm palette.
// Hand-built. MIT license (same as cozy).
//
// u_wind:      horizontal drift speed of the storm mass (weather)
// u_intensity: storm density, 0 = breaking up / 1 = solid overcast (weather)

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

float billow(vec2 p) {
    float total = 0.0, amplitude = 0.55;
    for (int i = 0; i < 7; i++) {
        total += abs(noise(p)) * amplitude;
        p = m * p;
        amplitude *= 0.5;
    }
    return total;
}

float density(vec2 sp, float intensity, float vbias) {
    float b = billow(sp * 1.3);
    // Heavier toward the bottom of the frame: storms hang low.
    float bias = mix(0.55, 0.18, intensity) - vbias * 0.25;
    return clamp((b - bias) * 2.6, 0.0, 1.0);
}

void main() {
    float intensity = clamp(u_intensity, 0.0, 1.0);

    vec2 sp = v_uv * vec2(u_resolution.x / u_resolution.y, 1.0);
    sp.x += u_time * (0.008 + u_wind * 0.035);

    float vbias = v_uv.y;           // 0 at top, 1 at bottom
    float d = density(sp, intensity, vbias);

    // Shading from above: tops catch the last light, the base sits in shadow
    // (but never crushes to black — that read as a flat grey slab).
    vec2  lightdir = normalize(vec2(-0.3, -1.0));
    float eps = 0.018;
    float dl  = density(sp + lightdir * eps, intensity, vbias);
    float slope = (d - dl) * 7.0;
    float shade = clamp(0.52 + slope, 0.34, 1.0);

    // Storm palette: moody slate-blue shadows lifting to a bright dirty white at
    // the lit cauliflower tops — heavy, but with light still in it.
    vec3  base_col = vec3(0.34, 0.37, 0.43);
    vec3  lit_col  = vec3(0.90, 0.91, 0.94);
    vec3  cloud    = mix(base_col, lit_col, shade);
    // Bottom of the frame stays a touch gloomier, gently.
    cloud *= mix(1.0, 0.82, vbias);

    float alpha = smoothstep(0.0, 0.25, d);

    if (u_overlay) {
        frag_color = vec4(cloud * alpha, alpha);
    } else {
        vec3 base = texture(u_wallpaper, cover_uv(v_uv, u_resolution, u_tex_resolution)).rgb;
        frag_color = vec4(mix(base, cloud, alpha), 1.0);
    }
}
