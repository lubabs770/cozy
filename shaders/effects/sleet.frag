#version 300 es
precision highp float;

// cozy effect: "sleet" — fast icy pellets with diagonal streaks.
// Hand-built. MIT license (same as cozy).
//
// u_wind:      diagonal angle — sleet is inherently angled (weather)
// u_intensity: density [0..1] (weather)

in vec2 v_uv;
out vec4 frag_color;

uniform vec2      u_resolution;
uniform vec2      u_tex_resolution;
uniform sampler2D u_wallpaper;
uniform float     u_time;
uniform float     u_wind;
uniform float     u_intensity;
// When true, output premultiplied alpha (transparent where the effect leaves
// the wallpaper unchanged) so cozy can composite over an external wallpaper.
uniform bool      u_overlay;

vec2 cover_uv(vec2 uv, vec2 res, vec2 tex_res) {
    float scale  = max(res.x / tex_res.x, res.y / tex_res.y);
    vec2  scaled = tex_res * scale;
    vec2  offset = (scaled - res) * 0.5;
    return (uv * res + offset) / scaled;
}

float hash11(float n) { return fract(sin(n * 78.233) * 43758.5453); }

// ---------------------------------------------------------------------------
// Fast diagonal streaks — short and icy (pellets in motion, not long rain tails)
// ---------------------------------------------------------------------------
const float SLEET_COLS = 150.0;

float sleetStreaks(vec2 uv, float t) {
    // Sleet always leans even without weather wind (base angle 0.30)
    float angle = 0.30 + u_wind * 0.45;
    vec2  p     = uv;
    p.x        += uv.y * angle;

    float col = floor(p.x * SLEET_COLS);
    float fx  = fract(p.x * SLEET_COLS) - 0.5;

    float r0 = hash11(col);
    float r1 = hash11(col + 17.0);
    float r2 = hash11(col + 41.0);

    float present = step(0.20, r2);       // ~80 % of columns active
    float speed   = mix(2.5, 4.5, r0);   // faster than rain
    float vrep    = mix(4.0, 7.0, r1);
    float len     = mix(0.04, 0.10, r1); // short streaks: icy, pellet-like

    float v    = p.y * vrep - t * speed + r0 * 10.0;
    float head = smoothstep(len, 0.0, fract(v));
    float thin = exp(-fx * fx / (2.0 * 0.030 * 0.030)); // narrower than rain

    return head * thin * present;
}

// ---------------------------------------------------------------------------
// Hard ice pellets — small solid discs falling on the same diagonal
// ---------------------------------------------------------------------------
const float PELLET_CELLS = 20.0;
const float PELLET_SPEED = 3.2;
const float PELLET_R     = 0.09; // cell-local radius

float icePellets(vec2 uv, float t) {
    float aspect = u_resolution.x / u_resolution.y;
    float angle  = 0.30 + u_wind * 0.45;
    vec2  p      = uv * vec2(PELLET_CELLS, PELLET_CELLS / aspect);
    vec2  g      = fract(p);
    float acc    = 0.0;

    for (int xi = -1; xi <= 1; xi++) {
        for (int yi = -1; yi <= 1; yi++) {
            vec2  off  = vec2(float(xi), float(yi));
            vec2  cell = floor(p) + off;
            vec2  loc  = g - off;

            float s0     = hash11(cell.x * 127.1 + cell.y * 311.7);
            float s1     = hash11(s0 * 7919.0);
            float present = step(0.45, s0); // ~55 % spawn rate

            float phase = fract(s0 + t * PELLET_SPEED * (0.8 + s1 * 0.4));
            float cx    = 0.5 + (s1 - 0.5) * 0.5 + angle * phase * 0.35;
            float cy    = phase;

            float d = length(loc - vec2(cx, cy));
            acc = max(acc, smoothstep(PELLET_R, PELLET_R * 0.8, d) * present);
        }
    }
    return acc;
}

void main() {
    vec3  base    = texture(u_wallpaper, cover_uv(v_uv, u_resolution, u_tex_resolution)).rgb;
    float density = clamp(u_intensity, 0.3, 1.0);

    float s = sleetStreaks(v_uv, u_time);
    float p = icePellets(v_uv, u_time);

    // Slightly cool the base (icy weather)
    float grey = dot(base, vec3(0.299, 0.587, 0.114));
    vec3  cool = mix(base, vec3(grey) * vec3(0.84, 0.87, 0.96), 0.14);

    vec3 color = cool
        + vec3(0.75, 0.82, 0.96) * s * 0.55 * density
        + vec3(0.88, 0.93, 1.00) * p * 0.75 * density;

    if (u_overlay) {
        vec3 plain = texture(u_wallpaper, cover_uv(v_uv, u_resolution, u_tex_resolution)).rgb;
        float a = clamp(length(color - plain) * 4.0, 0.0, 1.0);
        frag_color = vec4(color * a, a);
    } else {
        frag_color = vec4(color, 1.0);
    }
}
