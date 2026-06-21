#version 300 es
precision highp float;

// cozy effect: "cumulus" — fluffy fair-weather cumulus: discrete rounded puffs
// of cloud with bright cauliflower tops and shaded undersides, separated by
// clear sky. Built from billowy turbulence (summed |noise|) thresholded into
// distinct puffs, with a cheap directional shading pass that estimates the
// cloud's slope from the density gradient so the puffs read as 3D, not flat.
// Hand-built. MIT license (same as cozy).
//
// u_wind:      horizontal drift speed of the puffs (weather)
// u_intensity: cloud cover, 0 = a few puffs / 1 = crowded sky (weather)

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

// Billowy turbulence: summed absolute noise gives rounded, cauliflower lobes.
float billow(vec2 p) {
    float total = 0.0, amplitude = 0.55;
    for (int i = 0; i < 6; i++) {
        total += abs(noise(p)) * amplitude;
        p = m * p;
        amplitude *= 0.5;
    }
    return total;
}

// Cloud density at a point: billow field, biased by coverage into puffs.
float density(vec2 sp, float intensity) {
    float b = billow(sp * 1.6);
    float bias = mix(0.62, 0.30, intensity); // higher bias = sparser, smaller puffs
    return clamp((b - bias) * 3.0, 0.0, 1.0);
}

void main() {
    float intensity = clamp(u_intensity, 0.0, 1.0);

    vec2 sp = v_uv * vec2(u_resolution.x / u_resolution.y, 1.0);
    sp.x += u_time * (0.01 + u_wind * 0.04);

    float d = density(sp, intensity);

    // Directional shading: light from the upper-left. Compare density at the
    // fragment against density toward the light; surfaces tilted into the light
    // brighten (sunlit tops), those tilted away darken (shaded undersides).
    vec2  lightdir = normalize(vec2(-0.6, -1.0)); // toward upper-left, y up = -y
    float eps = 0.015;
    float dl  = density(sp + lightdir * eps, intensity);
    float slope = (d - dl) * 8.0;
    float shade = clamp(0.55 + slope, 0.25, 1.15);

    vec3  cloud = vec3(1.0, 1.0, 0.98) * shade;
    float alpha = smoothstep(0.0, 0.35, d);

    if (u_overlay) {
        frag_color = vec4(cloud * alpha, alpha);
    } else {
        vec3 base = texture(u_wallpaper, cover_uv(v_uv, u_resolution, u_tex_resolution)).rgb;
        frag_color = vec4(mix(base, cloud, alpha), 1.0);
    }
}
