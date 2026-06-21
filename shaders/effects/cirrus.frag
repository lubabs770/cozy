#version 300 es
precision highp float;

// cozy effect: "cirrus" — high, thin, wispy cirrus clouds. Long horizontal
// filaments stretched across the sky, drifting fast and high. Built from a
// heavily anisotropic (x-stretched) fbm field carved into streaks, so it reads
// as feathery mares'-tails rather than the soft blobs of the "clouds" effect.
// Hand-built. MIT license (same as cozy).
//
// u_wind:      horizontal drift speed of the wisps (weather)
// u_intensity: how much sky the wisps cover, 0 = sparse / 1 = streaky (weather)

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

float fbm(vec2 n) {
    float total = 0.0, amplitude = 0.5;
    for (int i = 0; i < 6; i++) {
        total += noise(n) * amplitude;
        n = m * n;
        amplitude *= 0.5;
    }
    return total;
}

void main() {
    float intensity = clamp(u_intensity, 0.0, 1.0);

    vec2 sp = v_uv * vec2(u_resolution.x / u_resolution.y, 1.0);
    // Cirrus rides high and fast; drift even with no wind set.
    sp.x += u_time * (0.02 + u_wind * 0.06);

    // Strong anisotropy: wide in x, compressed in y -> long thin streaks. A
    // domain warp shears the streaks so they sweep diagonally like mares'-tails.
    vec2 q  = vec2(sp.x * 0.7, sp.y * 3.4);
    float warp = fbm(q * 0.5 + vec2(0.0, u_time * 0.01));
    q.x += warp * 1.5;

    float n = fbm(q);
    // Carve filaments: bias by coverage, then sharpen so only the ridges show.
    float bias = mix(0.35, -0.05, intensity);
    float wisp = clamp(n - bias, 0.0, 1.0);
    wisp = pow(wisp, 2.2);

    // Fine high-frequency feathering along the streaks.
    float feather = 0.5 + 0.5 * noise(q * vec2(2.0, 6.0) + warp);
    wisp *= feather;

    float alpha = clamp(wisp * 2.0, 0.0, 1.0);
    vec3  tint  = vec3(1.0, 1.0, 1.02); // cool, near-white ice cloud

    if (u_overlay) {
        frag_color = vec4(tint * alpha, alpha);
    } else {
        vec3 base = texture(u_wallpaper, cover_uv(v_uv, u_resolution, u_tex_resolution)).rgb;
        frag_color = vec4(mix(base, tint, alpha), 1.0);
    }
}
