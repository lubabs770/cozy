#version 300 es
precision highp float;

// cozy effect: "sunrays" — volumetric god rays (crepuscular rays) fanning out
// from a sun high in the frame, broken into shafts by a drifting cloud occluder.
// Implemented as a radial light-scattering march: from each fragment we step
// toward the sun, accumulating how much light gets through the occluder along
// the way, with distance decay. A warm bloom sits around the sun itself.
// Hand-built. MIT license (same as cozy).
//
// u_wind:      drift speed of the occluding clouds that chop the shafts (weather)
// u_intensity: strength of the rays, 0 = faint / 1 = blazing (weather)

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

// Sun position in uv space (origin top-left): high and a touch left of centre.
const vec2  SUN_UV   = vec2(0.42, 0.18);
const vec3  SUN_TINT = vec3(1.0, 0.86, 0.62); // warm sunlight
const int   STEPS    = 56;

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

// How much light is blocked at a uv point: 0 = clear, 1 = opaque cloud.
float occluder(vec2 uv) {
    vec2 sp = uv * vec2(u_resolution.x / u_resolution.y, 1.0);
    sp.x += u_time * (0.02 + u_wind * 0.05);
    float c = fbm(sp * 2.2);
    return smoothstep(0.05, 0.55, c);
}

void main() {
    float intensity = clamp(u_intensity, 0.0, 1.0);

    // March from the fragment toward the sun, gathering transmitted light.
    vec2  delta = (SUN_UV - v_uv) / float(STEPS);
    vec2  pos   = v_uv;
    float decay = 1.0;
    float light = 0.0;
    for (int i = 0; i < STEPS; i++) {
        pos += delta;
        float transmit = 1.0 - occluder(pos);
        light += transmit * decay;
        decay *= 0.955; // closer-to-sun samples weigh more
    }
    light /= float(STEPS);

    // Warm bloom around the sun disc itself (aspect-corrected distance).
    vec2  aspect = vec2(u_resolution.x / u_resolution.y, 1.0);
    float dsun   = length((v_uv - SUN_UV) * aspect);
    float bloom  = exp(-dsun * 6.0) * (1.0 - occluder(SUN_UV));

    float strength = mix(0.5, 1.8, intensity);
    vec3  rays = SUN_TINT * (light * strength + bloom * 1.2);

    if (u_overlay) {
        // Additive light: alpha carries the glow so it brightens whatever is
        // beneath without darkening the clear sky.
        float a = clamp(light * strength * 0.9 + bloom, 0.0, 1.0);
        frag_color = vec4(SUN_TINT * a, a);
    } else {
        vec3 base = texture(u_wallpaper, cover_uv(v_uv, u_resolution, u_tex_resolution)).rgb;
        // Screen-blend the rays over the wallpaper so highlights bloom, not blow.
        vec3 lit = 1.0 - (1.0 - base) * (1.0 - rays);
        frag_color = vec4(lit, 1.0);
    }
}
