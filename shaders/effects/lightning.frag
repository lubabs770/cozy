#version 300 es
precision highp float;

// cozy effect: "lightning" — a dark, brooding storm sky that periodically cracks
// with lightning: the whole frame flares as a strike fires, and a jagged bolt
// forks down from the cloud base. Built from a dark billowy cloud field (a
// gloomier cumulonimbus), a time-seeded flash envelope (sharp attack, fast decay,
// with a flicker), and a noise-perturbed near-vertical bolt SDF. Hand-built.
// MIT license (same as cozy).
//
// u_wind:      horizontal drift speed of the storm clouds (weather)
// u_intensity: storm activity, 0 = occasional distant flashes / 1 = frequent,
//              close, bright strikes (weather)

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
const vec3 BOLT_TINT  = vec3(0.82, 0.88, 1.0); // cold electric white-blue
const vec3 FLASH_TINT = vec3(0.78, 0.84, 1.0);

vec2 cover_uv(vec2 uv, vec2 res, vec2 tex_res) {
    float scale  = max(res.x / tex_res.x, res.y / tex_res.y);
    vec2  scaled = tex_res * scale;
    vec2  offset = (scaled - res) * 0.5;
    return (uv * res + offset) / scaled;
}

float hash11(float n) { return fract(sin(n * 78.233) * 43758.5453); }

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

// Strike scheduling: chop time into windows; some windows fire a strike at a
// jittered moment. Returns the ambient flash level and reports the strike's
// horizontal position and a fast bolt-only envelope via out params.
float strike(float t, float intensity, out float xpos, out float bolt_env) {
    float period = mix(3.4, 1.2, intensity); // busier storms strike more often
    float idx    = floor(t / period);
    float seed   = hash11(idx);
    xpos         = 0.15 + 0.7 * hash11(idx * 1.7 + 3.1);

    // Not every window strikes; calmer storms skip more.
    float armed  = step(mix(0.6, 0.2, intensity), seed);
    float start  = idx * period + hash11(idx * 2.3) * period * 0.5;
    float dt     = t - start;
    float gate   = step(0.0, dt);

    // Ambient flash: sharp attack, exponential decay, with a quick flicker so it
    // reads as a real strike rather than a fade.
    float flash = exp(-dt * 5.0) * (0.55 + 0.45 * sin(dt * 55.0));
    flash = max(flash, 0.0) * gate * armed;

    // The visible bolt only exists for the first instant of the strike.
    bolt_env = exp(-dt * 16.0) * gate * armed;

    return clamp(flash, 0.0, 1.0);
}

// Distance-based glow for a jagged near-vertical bolt at column `xpos`.
float bolt(vec2 uv, float xpos, float seed) {
    // Perturb the bolt's x as it descends: coarse zig-zag + fine jitter.
    float jag = noise(vec2(seed * 13.0, uv.y * 7.0)) * 0.05
              + noise(vec2(seed * 31.0, uv.y * 23.0)) * 0.018;
    float bx  = xpos + jag;
    float d   = abs(uv.x - bx);

    float core = exp(-d * d * 22000.0); // tight bright filament
    float glow = exp(-d * 45.0) * 0.3;  // soft halo
    // Bolt forks from the cloud base (top) and fades toward the ground.
    float falloff = smoothstep(1.05, 0.15, uv.y);
    return (core + glow) * falloff;
}

void main() {
    float intensity = clamp(u_intensity, 0.0, 1.0);

    // Dark storm cloud field.
    vec2 sp = v_uv * vec2(u_resolution.x / u_resolution.y, 1.0);
    sp.x += u_time * (0.008 + u_wind * 0.035);
    float b = billow(sp * 1.3);
    float cloud_a = smoothstep(0.0, 0.3, clamp((b - 0.22) * 2.4, 0.0, 1.0));

    // Strike state.
    float xpos, bolt_env;
    float flash = strike(u_time, intensity, xpos, bolt_env);
    float seed  = floor(u_time / mix(3.4, 1.2, intensity)); // matches strike()
    float bolt_glow = bolt(v_uv, xpos, hash11(seed)) * bolt_env;

    // Cloud colour: near-black base, lifting briefly when a flash lights it from
    // within/behind.
    vec3 cloud_col = mix(vec3(0.08, 0.09, 0.12), FLASH_TINT, flash * 0.7);

    if (u_overlay) {
        // Storm clouds + ambient flash + the bolt itself, all premultiplied.
        float a = clamp(cloud_a + flash * 0.5 + bolt_glow, 0.0, 1.0);
        vec3  col = cloud_col * cloud_a + FLASH_TINT * flash * 0.5 + BOLT_TINT * bolt_glow;
        frag_color = vec4(col, a);
    } else {
        vec3 base = texture(u_wallpaper, cover_uv(v_uv, u_resolution, u_tex_resolution)).rgb;
        // Compose dark clouds over the wallpaper.
        vec3 sky = mix(base, cloud_col, cloud_a);
        // Flash lights the whole scene; the bolt blows out to white-blue.
        sky += FLASH_TINT * flash * 0.6;
        sky += BOLT_TINT * bolt_glow;
        frag_color = vec4(min(sky, vec3(1.0)), 1.0);
    }
}
