#version 300 es
precision highp float;

// cozy effect: "ripple" — rain falling on a still water surface. Every impact
// spawns an expanding concentric ring that refracts the wallpaper beneath it,
// and faint slanted streaks sell the falling rain. This is deliberately *not*
// the glass-on-window "droplet" effect: here the whole image warps like a pond
// surface rather than carrying beads of water on a pane.
// Hand-built. MIT license (same as cozy).
//
// u_wind:      slant of the falling streaks (weather)
// u_intensity: rain intensity 0 = sparse / 1 = busy (weather)

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
float hash21(vec2 p)  { return fract(sin(dot(p, vec2(41.3, 289.1))) * 43758.5453); }

// ---------------------------------------------------------------------------
// Ripple field
// ---------------------------------------------------------------------------
// A grid of raindrop impacts, each an expanding ring on a repeating cycle. We
// sum the 3x3 cell neighbourhood so rings cross cell borders. Each impact adds
// a wavefront "height" (a damped sinusoid at the ring radius) and a radial
// surface displacement; the displacement bends the wallpaper UV (refraction),
// the height drives crest highlights and trough shading.
const float RIPPLE_CELLS = 10.0; // impact density (cells across the short axis)
const float RING_MAX     = 1.7;  // how far a ring travels before it dies (cells)
const float WAVE_FREQ    = 17.0; // ring spacing
const float WAVE_TIGHT   = 4.5;  // wavefront sharpness (gaussian falloff)
const float REFRACT      = 0.035; // max wallpaper UV shift from the surface tilt

float ripples(vec2 g, float t, float intensity, out vec2 disp) {
    disp = vec2(0.0);
    float height = 0.0;
    vec2 base_cell = floor(g);

    for (int j = -1; j <= 1; j++) {
        for (int i = -1; i <= 1; i++) {
            vec2 cell = base_cell + vec2(float(i), float(j));

            float h1 = hash21(cell + 11.3);
            float h2 = hash21(cell + 27.7);
            float h3 = hash21(cell + 41.1);

            // Only a fraction of cells are active; more of them as rain picks up.
            if (h3 > mix(0.35, 0.95, intensity)) {
                continue;
            }

            // Impact centre, jittered inside the cell.
            vec2 center = cell + vec2(0.2 + 0.6 * h1, 0.2 + 0.6 * h2);

            // Repeating impact, each cell with its own period and phase.
            float period = mix(1.6, 3.0, h2);
            float age    = fract((t + h1 * period) / period); // 0 = just struck

            vec2  rel   = g - center;
            float d     = length(rel);
            float front = age * RING_MAX;  // current ring radius (cells)
            float x     = d - front;       // signed distance to the wavefront

            // Damped wavelet riding the wavefront, fading as the ring ages and
            // as its energy spreads outward.
            float env = (1.0 - age) / (1.0 + 2.2 * d);
            float w   = sin(x * WAVE_FREQ) * exp(-x * x * WAVE_TIGHT) * env;

            height += w;
            disp   += (d > 1e-4 ? rel / d : vec2(0.0)) * w;
        }
    }
    return height;
}

// ---------------------------------------------------------------------------
// Faint slanted falling streaks — the "it's raining" cue above the water.
// ---------------------------------------------------------------------------
const float STREAK_COLUMNS = 110.0;
const vec3  STREAK_TINT    = vec3(0.62, 0.74, 1.0);

float streaks(vec2 uv, float t, float wind) {
    vec2 p = uv;
    p.x += uv.y * (0.14 + wind * 0.40);

    float col = floor(p.x * STREAK_COLUMNS);
    float fx  = fract(p.x * STREAK_COLUMNS) - 0.5;

    float r0 = hash11(col);
    float r1 = hash11(col + 17.0);
    float r2 = hash11(col + 41.0);

    float present = step(0.55, r2);          // sparser than a full downpour
    float speed   = mix(1.1, 2.2, r0);
    float vrep    = mix(3.0, 5.0, r1);
    float len     = mix(0.08, 0.20, r1);

    float v    = p.y * vrep - t * speed + r0 * 10.0;
    float head = smoothstep(len, 0.0, fract(v));
    float thin = exp(-fx * fx / (2.0 * 0.05 * 0.05));
    return head * thin * present;
}

void main() {
    float intensity = clamp(u_intensity, 0.0, 1.0);
    vec2  aspect    = vec2(u_resolution.x / u_resolution.y, 1.0);

    // Aspect-corrected grid space so rings stay round on a wide output.
    vec2 g = v_uv * aspect * RIPPLE_CELLS;

    vec2  disp;
    float h = ripples(g, u_time, intensity, disp);

    // Radial displacement (aspect space) -> isotropic screen-uv offset, then
    // refract the wallpaper through the rippled surface.
    vec2 offset = vec2(disp.x / aspect.x, disp.y) * REFRACT * (0.8 + 0.5 * intensity);
    vec3 base = texture(u_wallpaper, cover_uv(v_uv + offset, u_resolution, u_tex_resolution)).rgb;

    // Crest highlight (cool glint on the leading edge) and a touch of trough
    // darkening, so each ring reads as a real dimple in the water.
    base += vec3(0.10, 0.13, 0.18) * pow(clamp(h, 0.0, 1.0), 2.0);
    base *= 1.0 - 0.12 * clamp(-h, 0.0, 1.0);

    // Faint falling streaks on top of the water.
    float s = streaks(v_uv, u_time, u_wind);
    vec3 color = base + STREAK_TINT * s * mix(0.18, 0.34, intensity);

    frag_color = vec4(color, 1.0);
}
