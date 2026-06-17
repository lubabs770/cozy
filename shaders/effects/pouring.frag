#version 300 es
precision highp float;

// cozy effect: "pouring" — heavy continuous downpour with fog and large droplets.
// Hand-built. MIT license (same as cozy).
//
// u_wind:      rain angle (weather)
// u_intensity: storm intensity 0 = light / 1 = downpour (weather)

in vec2 v_uv;
out vec4 frag_color;

uniform vec2      u_resolution;
uniform vec2      u_tex_resolution;
uniform sampler2D u_wallpaper;
uniform float     u_time;
uniform float     u_wind;
uniform float     u_intensity;

vec2 cover_uv(vec2 uv, vec2 res, vec2 tex_res) {
    float scale  = max(res.x / tex_res.x, res.y / tex_res.y);
    vec2  scaled = tex_res * scale;
    vec2  offset = (scaled - res) * 0.5;
    return (uv * res + offset) / scaled;
}

float hash11(float n) { return fract(sin(n * 78.233) * 43758.5453); }

// ---------------------------------------------------------------------------
// Very dense fast streaks — the backbone of a heavy downpour
// ---------------------------------------------------------------------------
const float POUR_COLS = 200.0;
const vec3  POUR_TINT = vec3(0.58, 0.70, 1.0);

float heavyStreaks(vec2 uv, float t) {
    vec2 p = uv;
    p.x   += uv.y * (0.18 + u_wind * 0.45);

    float col = floor(p.x * POUR_COLS);
    float fx  = fract(p.x * POUR_COLS) - 0.5;

    float r0 = hash11(col);
    float r1 = hash11(col + 17.0);
    float r2 = hash11(col + 41.0);

    float present = step(0.12, r2);         // ~88 % of columns active
    float speed   = mix(1.8, 3.5, r0);
    float vrep    = mix(3.0, 6.0, r1);
    float len     = mix(0.20, 0.42, r1);   // long tails → heavy motion blur

    float v    = p.y * vrep - t * speed + r0 * 10.0;
    float head = smoothstep(len, 0.0, fract(v));
    float thin = exp(-fx * fx / (2.0 * 0.048 * 0.048));

    return head * thin * present;
}

// ---------------------------------------------------------------------------
// Large refracting glass droplets (heavier build-up than classic)
// ---------------------------------------------------------------------------
const float SLIDE_COLS = 14.0;
const vec3  DROP_LIGHT = vec3(-0.5, -0.7, 1.0);
const float REFRACT    = 0.09;

vec3 shade_glass(vec3 inside, vec2 nrm) {
    float dome = sqrt(max(0.0, 1.0 - dot(nrm, nrm)));
    vec3  norm = normalize(vec3(nrm, dome));
    float spec = pow(max(dot(norm, normalize(DROP_LIGHT)), 0.0), 24.0);
    float rim  = 1.0 - smoothstep(0.55, 1.0, length(nrm));
    return inside * mix(0.78, 1.05, rim) + vec3(spec);
}

vec3 refracted_base(vec2 nrm, float ax) {
    vec2 slope  = nrm / max(1.0, length(nrm));
    vec2 offset = vec2(slope.x / ax, slope.y) * REFRACT;
    return texture(u_wallpaper, cover_uv(v_uv + offset, u_resolution, u_tex_resolution)).rgb;
}

float slidingDroplets(vec2 uv, float t, out vec2 nrm) {
    nrm = vec2(0.0);
    float aspect = u_resolution.x / u_resolution.y;
    float col    = floor(uv.x * SLIDE_COLS);
    float fx     = fract(uv.x * SLIDE_COLS) - 0.5;

    float present = step(0.18, hash11(col + 19.0)); // ~82 % of columns
    if (present < 0.5) return 0.0;

    float r0    = hash11(col + 1.3);
    float r1    = hash11(col + 7.7);
    float speed = mix(0.14, 0.32, r0); // faster than classic

    float head_y = fract(t * speed + r1);
    float wiggle = 0.08 * sin(head_y * 6.2831 + col * 2.0);

    float dxe    = (fx - wiggle) / SLIDE_COLS * aspect;
    float dye    = uv.y - head_y;
    float radius = mix(0.070, 0.115, r0); // larger drops in heavy rain

    float head  = smoothstep(radius, radius * 0.6, length(vec2(dxe, dye)));
    float above = smoothstep(0.0, -0.38, dye);
    float taper = smoothstep(radius * 0.55, 0.0, abs(dxe));
    float trail = taper * above * 0.7;

    nrm = vec2(dxe, dye) / radius;
    return clamp(head + trail, 0.0, 1.0);
}

void main() {
    float intensity = clamp(u_intensity, 0.0, 1.0);
    float aspect_x  = u_resolution.x / u_resolution.y;

    // Sample the wallpaper, then apply a rain-fog veil that thickens with intensity.
    vec3  base = texture(u_wallpaper, cover_uv(v_uv, u_resolution, u_tex_resolution)).rgb;
    float fog  = mix(0.05, 0.22, intensity);
    base = mix(base, vec3(0.50, 0.55, 0.65), fog);

    // Dense streaks
    float s     = heavyStreaks(v_uv, u_time);
    vec3  color = base + POUR_TINT * s * mix(0.55, 0.88, intensity);

    // Large refracting droplets on top
    vec2  nrm;
    float m = slidingDroplets(v_uv, u_time, nrm);
    if (m > 0.0) {
        color = mix(color, shade_glass(refracted_base(nrm, aspect_x), nrm), m);
    }

    frag_color = vec4(color, 1.0);
}
