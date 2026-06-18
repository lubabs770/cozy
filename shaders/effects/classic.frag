#version 300 es
precision highp float;

// cozy fragment shader.
//
// The pipeline is built up across milestones; each effect is a self-contained
// stage so it can be developed and debugged independently:
//
//   M3  base       — sample the wallpaper, cover-fit to the output.
//   M4a streaks    — additive falling rain streaks.
//   M4b droplets   — static glass beads (distance field + specular).
//   M4c droplets   — sliding / merging beads.
//   M4d refraction — droplets offset the wallpaper UV they sample.      <-- here
//
// Composite (final): refracted_base + streaks * tint.

in vec2 v_uv;
out vec4 frag_color;

uniform vec2 u_resolution;     // output size in pixels
uniform vec2 u_tex_resolution; // wallpaper size in pixels
uniform sampler2D u_wallpaper;
uniform float u_time;          // seconds since start, drives all motion
// When true, output premultiplied alpha (transparent where the effect leaves
// the wallpaper unchanged) so cozy can composite over an external wallpaper.
uniform bool u_overlay;

// A shared wind skew (x-shift per unit of y) applied to every effect, so the
// streaks and (later) droplets read as one coherent storm. Promoted to a config
// uniform in M6.
const float WIND = 0.18;

float hash11(float n) {
    return fract(sin(n * 78.233) * 43758.5453);
}

// ---------------------------------------------------------------------------
// base: cover-fit sampling
// ---------------------------------------------------------------------------
// Map a screen UV to a wallpaper UV with "cover" behavior: scale the image to
// fill the output while preserving its aspect ratio, cropping the overflow
// equally on both sides of the longer axis.
vec2 cover_uv(vec2 uv, vec2 res, vec2 tex_res) {
    float scale = max(res.x / tex_res.x, res.y / tex_res.y);
    vec2 scaled = tex_res * scale;
    vec2 offset = (scaled - res) * 0.5;
    return (uv * res + offset) / scaled;
}

// ---------------------------------------------------------------------------
// M4a: additive falling streaks
// ---------------------------------------------------------------------------
// Screen is divided into vertical columns. Each column gets hashed parameters
// (speed, density, length, activity) so the rain looks irregular. Within a
// column a thin bright head scrolls downward, elongated into a fading tail for
// motion blur. The whole field is skewed by WIND so the rain slants.
const float STREAK_COLUMNS = 90.0;
const vec3 STREAK_TINT = vec3(0.62, 0.74, 1.0);
const float STREAK_STRENGTH = 0.65;

float streaks(vec2 uv, float t) {
    vec2 p = uv;
    p.x += uv.y * WIND; // slant the columns with the wind

    float col = floor(p.x * STREAK_COLUMNS);
    float fx = fract(p.x * STREAK_COLUMNS) - 0.5; // [-0.5, 0.5] across the column

    float r0 = hash11(col);
    float r1 = hash11(col + 17.0);
    float r2 = hash11(col + 41.0);

    float present = step(0.35, r2);    // ~65% of columns carry rain
    float speed = mix(0.7, 1.8, r0);   // varied fall speed
    float vrep = mix(2.0, 4.0, r1);    // streaks stacked along the column
    float len = mix(0.10, 0.28, r1);   // elongation == motion blur

    float v = p.y * vrep - t * speed + r0 * 10.0;
    float head = smoothstep(len, 0.0, fract(v));         // bright head, fading tail
    float thin = exp(-fx * fx / (2.0 * 0.06 * 0.06));    // narrow vertical line

    return head * thin * present;
}

// ---------------------------------------------------------------------------
// droplet shading (shared by the static and sliding fields)
// ---------------------------------------------------------------------------
// `nrm` is the bead-surface slope in [-1,1] (xy of a unit hemisphere). Beyond
// the unit disk (e.g. a trail) the dome flattens and the region just darkens.
const vec3 DROP_LIGHT = vec3(-0.5, -0.7, 1.0); // highlight from upper-left

vec3 shade_glass(vec3 inside, vec2 nrm) {
    float dome = sqrt(max(0.0, 1.0 - dot(nrm, nrm)));
    vec3 normal = normalize(vec3(nrm, dome));
    float spec = pow(max(dot(normal, normalize(DROP_LIGHT)), 0.0), 28.0);
    float rim = 1.0 - smoothstep(0.55, 1.0, length(nrm)); // 1 center -> 0 edge
    return inside * mix(0.78, 1.05, rim) + vec3(spec);
}

// ---------------------------------------------------------------------------
// M4b: static glass droplets
// ---------------------------------------------------------------------------
// A hashed grid of beads. Work in aspect-corrected space so cells (and beads)
// stay round on a wide output. Each cell may host one jittered bead. Returns
// the coverage mask and outputs the bead-surface normal for shading/refraction.
const float DROP_CELLS = 13.0;

float static_droplets(vec2 uv, out vec2 nrm) {
    nrm = vec2(0.0);
    vec2 aspect = vec2(u_resolution.x / u_resolution.y, 1.0);
    vec2 grid = uv * aspect * DROP_CELLS;
    vec2 cell = floor(grid);
    vec2 f = fract(grid) - 0.5; // [-0.5, 0.5] within the cell

    float present = step(0.45, hash11(dot(cell, vec2(13.1, 71.7))));
    if (present < 0.5) {
        return 0.0;
    }

    float rx = hash11(dot(cell, vec2(7.3, 13.9)));
    float ry = hash11(dot(cell, vec2(23.1, 3.7)));
    float rr = hash11(dot(cell, vec2(41.3, 29.1)));

    vec2 center = (vec2(rx, ry) - 0.5) * 0.5; // jitter within the cell
    float radius = mix(0.16, 0.32, rr);

    vec2 rel = f - center;
    float d = length(rel);
    nrm = rel / radius;
    return smoothstep(radius, radius * 0.9, d);
}

// ---------------------------------------------------------------------------
// M4c: sliding / merging droplets
// ---------------------------------------------------------------------------
// Per-column heavy drops that slide downward on a time sawtooth, wiggling
// slightly, with a thin tapering trail above the head. They glide over the
// static condensation field, reading as drops that pick up smaller beads as
// they fall.
const float SLIDE_COLUMNS = 8.0;

float sliding_droplets(vec2 uv, float t, out vec2 nrm) {
    nrm = vec2(0.0);
    vec2 aspect = vec2(u_resolution.x / u_resolution.y, 1.0);

    float col = floor(uv.x * SLIDE_COLUMNS);
    float fx = fract(uv.x * SLIDE_COLUMNS) - 0.5; // [-0.5, 0.5] within column

    float present = step(0.30, hash11(col + 19.0));
    if (present < 0.5) {
        return 0.0;
    }

    float r0 = hash11(col + 1.3);
    float r1 = hash11(col + 7.7);
    float speed = mix(0.08, 0.18, r0);

    float head_y = fract(t * speed + r1);              // slides down (y increases down)
    float wiggle = 0.10 * sin(head_y * 6.2831 + col * 2.0);

    // Compare distances in uv.y units (multiply the x offset by the aspect).
    float dxe = (fx - wiggle) / SLIDE_COLUMNS * aspect.x;
    float dye = uv.y - head_y;
    float radius = mix(0.050, 0.090, r0);              // heavier than static beads

    float head = smoothstep(radius, radius * 0.6, length(vec2(dxe, dye)));

    // Tapering wet trail above the head (dye < 0 == above).
    float above = smoothstep(0.0, -0.30, dye);
    float taper = smoothstep(radius * 0.55, 0.0, abs(dxe));
    float trail = taper * above * 0.7;

    nrm = vec2(dxe, dye) / radius;
    return clamp(head + trail, 0.0, 1.0);
}

// ---------------------------------------------------------------------------
// M4d: refraction
// ---------------------------------------------------------------------------
// A droplet is a lens: the wallpaper seen through it is offset along the bead's
// surface slope (`nrm`). Sampling the wallpaper at a slope-shifted UV bends the
// image underneath — straight lines curve, the view magnifies toward center.
const float REFRACT = 0.08; // max UV shift at a bead's rim (screen-uv units)

vec3 refracted_base(vec2 nrm, float aspect_x) {
    // Clamp the slope to the unit disk so trails don't sample wildly far away,
    // and undo the aspect on x so the shift is isotropic on screen.
    vec2 slope = nrm / max(1.0, length(nrm));
    vec2 offset = vec2(slope.x / aspect_x, slope.y) * REFRACT;
    return texture(u_wallpaper, cover_uv(v_uv + offset, u_resolution, u_tex_resolution)).rgb;
}

void main() {
    float aspect_x = u_resolution.x / u_resolution.y;
    vec3 base = texture(u_wallpaper, cover_uv(v_uv, u_resolution, u_tex_resolution)).rgb;

    // streaks: additive over the wallpaper.
    float s = streaks(v_uv, u_time);
    vec3 color = base + STREAK_TINT * (s * STREAK_STRENGTH);

    // droplets: glass beads composited on top, each refracting the wallpaper.
    // Static field first, then the sliding drops over it (so a sliding drop
    // covers the beads it passes).
    vec2 n_static;
    float m_static = static_droplets(v_uv, n_static);
    if (m_static > 0.0) {
        color = mix(color, shade_glass(refracted_base(n_static, aspect_x), n_static), m_static);
    }

    vec2 n_slide;
    float m_slide = sliding_droplets(v_uv, u_time, n_slide);
    if (m_slide > 0.0) {
        color = mix(color, shade_glass(refracted_base(n_slide, aspect_x), n_slide), m_slide);
    }

    if (u_overlay) {
        // Transparent where cozy left the wallpaper untouched; opaque where it
        // added streaks/droplets. Premultiplied alpha for the compositor.
        vec3 plain = texture(u_wallpaper, cover_uv(v_uv, u_resolution, u_tex_resolution)).rgb;
        float a = clamp(length(color - plain) * 4.0, 0.0, 1.0);
        frag_color = vec4(color * a, a);
    } else {
        frag_color = vec4(color, 1.0);
    }
}
