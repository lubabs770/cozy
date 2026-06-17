#version 300 es
precision highp float;

// cozy effect: "droplet" — rain on glass with refraction.
//
// Ported from "Heartfelt" by Martijn Steinrucken (aka BigWings), 2017:
//     https://www.shadertoy.com/view/ltffzl
//
// The original is licensed Creative Commons Attribution-NonCommercial-ShareAlike
// 3.0 Unported (CC BY-NC-SA 3.0). This ported file inherits that license and is
// therefore CC BY-NC-SA 3.0 — NOT the MIT license that covers the rest of cozy.
// Attribution: Martijn Steinrucken (BigWings). See README for details.
//
// Adapted to cozy's uniform contract and coordinate conventions:
//   * iResolution -> u_resolution, iTime -> u_time, iChannel0 -> u_wallpaper
//   * the mouse-driven "rain amount" is replaced by u_intensity (weather)
//   * u_wind skews the falling rain horizontally (weather)
//   * the wallpaper is sampled cover-fit (u_tex_resolution) with mip-level blur
//     for the through-the-glass depth of field

in vec2 v_uv;
out vec4 frag_color;

uniform vec2 u_resolution;     // output size in pixels
uniform vec2 u_tex_resolution; // wallpaper size in pixels
uniform sampler2D u_wallpaper;
uniform float u_time;          // seconds since start
uniform float u_wind;          // horizontal skew of the rain (weather)
uniform float u_intensity;     // 0..1 rain amount (weather)

#define S(a, b, t) smoothstep(a, b, t)

vec3 N13(float p) {
    vec3 p3 = fract(vec3(p) * vec3(.1031, .11369, .13787));
    p3 += dot(p3, p3.yzx + 19.19);
    return fract(vec3((p3.x + p3.y) * p3.z, (p3.x + p3.z) * p3.y, (p3.y + p3.z) * p3.x));
}

float N(float t) {
    return fract(sin(t * 12345.564) * 7658.76);
}

float Saw(float b, float t) {
    return S(0., b, t) * S(1., b, t);
}

vec2 DropLayer2(vec2 uv, float t) {
    vec2 UV = uv;
    uv.y += t * 0.75;
    vec2 a = vec2(6., 1.);
    vec2 grid = a * 2.;
    vec2 id = floor(uv * grid);
    float colShift = N(id.x);
    uv.y += colShift;
    id = floor(uv * grid);
    vec3 n = N13(id.x * 35.2 + id.y * 2376.1);
    vec2 st = fract(uv * grid) - vec2(.5, 0);
    float x = n.x - .5;
    float y = UV.y * 20.;
    float wiggle = sin(y + sin(y));
    x += wiggle * (.5 - abs(x)) * (n.z - .5);
    x *= .7;
    float ti = fract(t + n.z);
    y = (Saw(.85, ti) - .5) * .9 + .5;
    vec2 p = vec2(x, y);
    float d = length((st - p) * a.yx);
    float mainDrop = S(.4, .0, d);
    float r = sqrt(S(1., y, st.y));
    float cd = abs(st.x - x);
    float trail = S(.23 * r, .15 * r * r, cd);
    float trailFront = S(-.02, .02, st.y - y);
    trail *= trailFront * r * r;
    y = UV.y;
    float trail2 = S(.2 * r, .0, cd);
    float droplets = max(0., (sin(y * (1. - y) * 120.) - st.y)) * trail2 * trailFront * n.z;
    y = fract(y * 10.) + (st.y - .5);
    float dd = length(st - vec2(x, y));
    droplets = S(.3, 0., dd);
    float m = mainDrop + droplets * r * trailFront;
    return vec2(m, trail);
}

float StaticDrops(vec2 uv, float t) {
    uv *= 40.;
    vec2 id = floor(uv);
    uv = fract(uv) - .5;
    vec3 n = N13(id.x * 107.45 + id.y * 3543.654);
    vec2 p = (n.xy - .5) * .7;
    float d = length(uv - p);
    float fade = Saw(.025, fract(t + n.z));
    float c = S(.3, 0., d) * fract(n.z * 10.) * fade;
    return c;
}

vec2 Drops(vec2 uv, float t, float l0, float l1, float l2) {
    float s = StaticDrops(uv, t) * l0;
    vec2 m1 = DropLayer2(uv, t) * l1;
    vec2 m2 = DropLayer2(uv * 1.85, t) * l2;
    float c = s + m1.x + m2.x;
    c = S(.3, 1., c);
    return vec2(c, max(m1.y * l0, m2.y * l1));
}

// cozy cover-fit: scale the wallpaper to fill the output, preserving aspect.
vec2 cover_uv(vec2 uv, vec2 res, vec2 tex_res) {
    float scale = max(res.x / tex_res.x, res.y / tex_res.y);
    vec2 scaled = tex_res * scale;
    vec2 offset = (scaled - res) * 0.5;
    return (uv * res + offset) / scaled;
}

void main() {
    // Shadertoy works in a y-up, pixel-based frame; reconstruct it from cozy's
    // top-left v_uv so the ported drop physics stay identical to the original.
    vec2 frag = vec2(v_uv.x, 1.0 - v_uv.y) * u_resolution;
    vec2 uv = (frag - 0.5 * u_resolution) / u_resolution.y;

    // Wind skews the falling rain horizontally (weather-reactive; 0 == none).
    uv.x += (uv.y + 0.5) * u_wind;

    float T = u_time;
    float t = T * .2;

    float rainAmount = clamp(u_intensity, 0.0, 1.0);
    float maxBlur = mix(3., 6., rainAmount);
    float minBlur = 2.;

    float staticDrops = S(-.5, 1., rainAmount) * 2.;
    float layer1 = S(.25, .75, rainAmount);
    float layer2 = S(.0, .5, rainAmount);

    vec2 c = Drops(uv, t, staticDrops, layer1, layer2);

    // Surface slope of the drop field, by finite differences — this is the lens
    // normal that bends the wallpaper underneath each drop.
    vec2 e = vec2(.001, 0.);
    float cx = Drops(uv + e, t, staticDrops, layer1, layer2).x;
    float cy = Drops(uv + e.yx, t, staticDrops, layer1, layer2).x;
    vec2 n = vec2(cx - c.x, cy - c.x);

    float focus = mix(maxBlur - c.y, minBlur, S(.1, .2, c.x));

    // Sample the wallpaper cover-fit, offset by the drop normal (refraction) and
    // blurred by `focus` via mip levels. n is in y-up space, so flip y back into
    // cozy's top-left uv before offsetting.
    vec2 base = cover_uv(v_uv + vec2(n.x, -n.y), u_resolution, u_tex_resolution);
    vec3 col = textureLod(u_wallpaper, base, focus).rgb;

    frag_color = vec4(col, 1.0);
}
