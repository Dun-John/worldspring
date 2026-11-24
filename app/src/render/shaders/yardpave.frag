#version 300 es
// A coaching inn's yard paving under its gallery and gate lane, in a building's interior view.
// MIRRORS ground.frag's street cobbles (surface 14, not the fan pattern): hash21 / vnoise / fbm3,
// stoneLook, the non-fan branch of cobbles(), the SURF[14] tint with uStone, and the 5-ft grid over
// paving. Keep it in step with ground.frag. The stones come from WORLD coordinates (the ground's
// own lattice, `w` = global square coordinates, noise period 512), so they run on unbroken from the
// open yard's real cobbles.
//
// vLocal: the interior's grid coordinates (squares of the building's frame).
// uOff: the world's square coordinates (mod 512) of the interior grid's origin; uAxis / uAcross:
//       the world step of one grid square along x and y (unit vectors, as `Interior.axis` / `across`).
// uPxPerSq: pixels a square, as the ground's. uStone: the town's stone tone, the ground's.
// uShade: the ground's own light on flat paving (0.6: as bright as the street's stones outside, no roofed shade added).
precision highp float;
in vec2 vUV;
in vec2 vLocal;

out vec4 finalColor;

uniform highp sampler2D uNoise;
uniform vec2 uOff;
uniform vec2 uAxis;
uniform vec2 uAcross;
uniform float uPxPerSq;
uniform vec3 uStone;
uniform float uShade;
uniform float uGrid;

const vec3 INK = vec3(0.12, 0.10, 0.08);
const vec3 COBBLE = vec3(0.66, 0.62, 0.55);

float hash21(vec2 p) {
    p = fract(p * vec2(123.34, 456.21));
    p += dot(p, p + 45.32);
    return fract(p.x * p.y);
}

float vnoise(vec2 p) {
    vec2 i = floor(p);
    vec2 f = fract(p);
    f = f * f * (3.0 - 2.0 * f);
    return texture(uNoise, (i + f + 0.5) / 512.0).r;
}

float fbm3(vec2 p) {
    return 0.55 * vnoise(p) + 0.3 * vnoise(p * 2.13 + 7.1) + 0.15 * vnoise(p * 4.37 + 3.3);
}

vec3 stoneLook(vec3 col, vec2 v, float hs, float joint, float aa) {
    vec3 st = col * (0.955 + 0.045 * floor(hs * 2.999));
    float side = dot(v, vec2(-0.7071));
    float lit = smoothstep(0.42 - aa, 0.42 + aa, side);
    float shade = smoothstep(0.38 - aa, 0.38 + aa, -side);
    st *= 1.0 + (0.06 * lit - 0.12 * shade) * mix(0.75, 1.0, smoothstep(20.0, 25.0, uPxPerSq));
    joint *= mix(0.8, 1.0, smoothstep(20.0, 28.0, uPxPerSq));
    vec3 c = mix(st, col * 0.86, joint);
    float stroke = step(0.667, fract(hs * 13.7)) * smoothstep(0.2, 0.6, -side);
    return mix(c, INK, joint * stroke * 0.55);
}

// (Fade 1: the paving's own middle, never its edge.)
vec3 cobbles(vec3 col, vec2 w) {
    vec3 flat_ = col * 0.95;
    float px = 0.5714 * uPxPerSq;
    float show = smoothstep(5.0, 10.0, px);
    if (show <= 0.0) return flat_;
    float jw = max(0.5, 0.035 * px);
    vec2 p = w * 1.75;
    float rb = floor(p.y - 0.5);
    float s1 = -9.0;
    float s2 = -9.0;
    vec2 v1 = vec2(0.0);
    float h1 = 0.0;
    for (int k = 0; k < 4; k++) {
        float r = rb + float(k >> 1);
        float off = 0.5 * mod(r, 2.0);
        vec2 c = vec2(floor(p.x - off - 0.5) + float(k & 1), r);
        float h = hash21(mod(c, 896.0));
        float cx = c.x + 0.5 + off;
        float wy = 0.12 * sin(cx * (2.0862 / 1.75) + 6.2832 * hash21(vec2(mod(r, 896.0), 3.0)));
        vec2 o = p - vec2(cx, r + 0.5 + wy) - (vec2(h, fract(h * 37.1)) - 0.5) * vec2(0.24, 0.1);
        float ang = (fract(h * 41.0) - 0.5) * 0.7;
        vec2 ax = vec2(cos(ang), sin(ang));
        float isx = 1.0 / (1.0 + 0.15 * fract(h * 7.3));
        vec2 m = vec2(dot(o, ax) * isx, dot(o, vec2(-ax.y, ax.x)));
        float rad = 0.5 * (0.85 + 0.3 * fract(h * 3.1));
        float s = rad - length(m);
        if (s > s1) {
            s2 = s1;
            s1 = s;
            v1 = (ax * (m.x * isx) + vec2(-ax.y, ax.x) * m.y) / rad;
            h1 = h;
        } else {
            s2 = max(s2, s);
        }
    }
    float a2 = 0.5 * (s1 - s2);
    float hk = max(0.12 - abs(s1 - a2), 0.0) / 0.12;
    float gap = min(s1, a2) - hk * hk * 0.03;
    float joint = 1.0 - smoothstep(jw - 0.5, jw + 0.5, gap * px);
    vec3 stone = stoneLook(col, v1, fract(h1 * 19.7), joint, 2.5 / px);
    return mix(flat_, stone, show);
}

void main() {
    vec2 w = uOff + uAxis * vLocal.x + uAcross * vLocal.y;
    vec3 col = cobbles(COBBLE * (0.94 + 0.1 * fbm3(w * 0.45)) * uStone, w);
    col *= uShade;
    // 5-ft grid, as the ground draws it over cobbles (world-axis lattice).
    float show = smoothstep(5.0, 10.0, 0.5714 * uPxPerSq);
    vec2 f = fract(w);
    vec2 g = min(f, 1.0 - f) * uPxPerSq;
    float line = 1.0 - smoothstep(0.4, 1.2, min(g.x, g.y));
    col = mix(col, INK, line * mix(0.28, 0.45, show) * uGrid);
    finalColor = vec4(col, 1.0);
}
