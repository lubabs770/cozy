#version 300 es
precision highp float;

// cozy effect: "snow" — softly drifting snowflakes.
// Hand-built. MIT license (same as cozy).
//
// u_wind:      lateral drift per unit of fall (weather)
// u_intensity: flake density [0..1] (weather)

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

float hash11(float n) { return fract(sin(n * 127.1) * 43758.5453); }

// Soft snowflake coverage for one depth layer.
//   cells: grid cells spanning screen width — larger = smaller / denser flakes
//   speed: fall rate in cells per second
//   r:     soft radius in cell-local units [0..1]
float snowLayer(vec2 uv, float t, float cells, float speed, float r) {
    float aspect = u_resolution.x / u_resolution.y;
    vec2  p      = uv * vec2(cells, cells / aspect); // square cells in screen space
    vec2  g      = fract(p);
    float acc    = 0.0;

    for (int xi = -1; xi <= 1; xi++) {
        for (int yi = -1; yi <= 1; yi++) {
            vec2  off  = vec2(float(xi), float(yi));
            vec2  cell = floor(p) + off;
            vec2  loc  = g - off; // fragment position relative to this cell's origin

            float s0     = hash11(cell.x * 127.1 + cell.y * 311.7);
            float s1     = hash11(s0 * 7919.0);
            float active = step(0.3, hash11(s1 * 3571.0)); // ~70 % spawn rate

            // Sawtooth fall phase, phase-shifted per flake so they're not in sync.
            float phase = fract(s0 + t * speed * (0.7 + s1 * 0.6));

            // Lateral position: cell jitter + wind drift + gentle wobble.
            float cx = 0.5 + (s1 - 0.5) * 0.55
                       + u_wind * phase * 0.35
                       + 0.07 * sin(t * 1.1 + s0 * 6.2831);
            float cy = phase;

            float d = length(loc - vec2(cx, cy));
            acc = max(acc, smoothstep(r, r * 0.3, d) * active);
        }
    }
    return acc;
}

void main() {
    vec3  base    = texture(u_wallpaper, cover_uv(v_uv, u_resolution, u_tex_resolution)).rgb;
    float density = mix(0.4, 1.0, clamp(u_intensity, 0.0, 1.0));

    // Three depth layers: near (large / slow), mid, far (tiny / quick).
    float near = snowLayer(v_uv, u_time, 12.0, 0.08, 0.22) * density;
    float mid  = snowLayer(v_uv, u_time, 22.0, 0.15, 0.16) * density;
    float far  = snowLayer(v_uv, u_time, 40.0, 0.24, 0.12) * density;

    float snow  = clamp(near + mid * 0.65 + far * 0.4, 0.0, 1.0);
    vec3  color = base + vec3(0.92, 0.96, 1.0) * snow * 0.88;

    frag_color = vec4(color, 1.0);
}
