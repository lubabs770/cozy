#version 300 es
precision highp float;

// cozy effect: "clouds" — soft fractal clouds drifting across the wallpaper.
//
// Ported from "2D Clouds" by drift, 2015:
//     https://www.shadertoy.com/view/4tdSWr
//
// The original is licensed Creative Commons Attribution-NonCommercial-ShareAlike
// 3.0 (CC BY-NC-SA 3.0). This ported file inherits that license and is therefore
// CC BY-NC-SA 3.0 — NOT the MIT license that covers the rest of cozy.
// Attribution: drift. See README for details.
//
// Adapted to cozy's uniform contract and conventions:
//   * drift's procedural blue sky is dropped — the wallpaper IS the sky, and the
//     clouds composite on top of it (cover-fit via u_tex_resolution)
//   * iTime -> u_time
//   * u_wind scrolls the cloud field horizontally (weather)
//   * u_intensity drives cloud cover (sparse wisps -> heavy overcast) (weather)
//   * u_overlay outputs premultiplied alpha (clear sky stays transparent) so the
//     clouds can drift over an external wallpaper daemon

in vec2 v_uv;
out vec4 frag_color;

uniform vec2      u_resolution;
uniform vec2      u_tex_resolution;
uniform sampler2D u_wallpaper;
uniform float     u_time;
uniform float     u_wind;
uniform float     u_intensity;
uniform bool      u_overlay;

// drift's constants (sky-colour constants dropped — the wallpaper is the sky).
const float cloudscale = 1.1;
const float speed      = 0.03;
const float clouddark  = 0.5;
const float cloudlight = 0.3;
const float cloudalpha = 8.0;
const mat2  m          = mat2(1.6, 1.2, -1.2, 1.6);

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
    const float K1 = 0.366025404; // (sqrt(3)-1)/2
    const float K2 = 0.211324865; // (3-sqrt(3))/6
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
    float total = 0.0, amplitude = 0.1;
    for (int i = 0; i < 7; i++) {
        total += noise(n) * amplitude;
        n = m * n;
        amplitude *= 0.4;
    }
    return total;
}

void main() {
    float intensity = clamp(u_intensity, 0.0, 1.0);

    // Aspect-corrected field, scrolled horizontally by the wind.
    vec2 p  = v_uv;
    vec2 sp = p * vec2(u_resolution.x / u_resolution.y, 1.0);
    sp.x += u_wind * u_time * 0.03;

    float time = u_time * speed;
    float q    = fbm(sp * cloudscale * 0.5);

    // Ridged cloud detail.
    float r  = 0.0;
    vec2  uv = sp * cloudscale - (q - time);
    float weight = 0.8;
    for (int i = 0; i < 8; i++) {
        r += abs(weight * noise(uv));
        uv = m * uv + time;
        weight *= 0.7;
    }

    // Soft cloud body.
    float f = 0.0;
    uv = sp * cloudscale - (q - time);
    weight = 0.7;
    for (int i = 0; i < 8; i++) {
        f += weight * noise(uv);
        uv = m * uv + time;
        weight *= 0.6;
    }
    f *= r + f;

    // Two more octaves at higher speed for colour/shading detail.
    float c = 0.0;
    time = u_time * speed * 2.0;
    uv = sp * cloudscale * 2.0 - (q - time);
    weight = 0.4;
    for (int i = 0; i < 7; i++) {
        c += weight * noise(uv);
        uv = m * uv + time;
        weight *= 0.6;
    }
    float c1 = 0.0;
    time = u_time * speed * 3.0;
    uv = sp * cloudscale * 3.0 - (q - time);
    weight = 0.4;
    for (int i = 0; i < 7; i++) {
        c1 += abs(weight * noise(uv));
        uv = m * uv + time;
        weight *= 0.6;
    }
    c += c1;

    // Weather: intensity sets how much sky the clouds cover.
    float cover = mix(0.05, 0.45, intensity);

    vec3  cloudcolour = vec3(1.1, 1.1, 0.9) * clamp(clouddark + cloudlight * c, 0.0, 1.0);
    f = cover + cloudalpha * f * r;
    float alpha = clamp(f + c, 0.0, 1.0);

    if (u_overlay) {
        // Premultiplied: clouds carry alpha, clear sky is transparent.
        frag_color = vec4(cloudcolour * alpha, alpha);
    } else {
        // The wallpaper is the sky; clouds drift across it.
        vec3 base = texture(u_wallpaper, cover_uv(v_uv, u_resolution, u_tex_resolution)).rgb;
        frag_color = vec4(mix(base, cloudcolour, alpha), 1.0);
    }
}
