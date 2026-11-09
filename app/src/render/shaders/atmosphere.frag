#version 300 es
// Battlemap atmosphere overlay, animated and world-anchored per chunk:
// 1 mist, 2 fog, 3 fireflies, 4 snowfall, 5 blowing sand, 6 embers, 7 heat haze,
// 8 falling leaves, 9 drizzle.

in vec2 vUV;
in vec2 vLocal;

out vec4 finalColor;

uniform highp sampler2D uNoise;
uniform float uKind;
uniform float uTime;
uniform float uAlpha;
uniform vec2 uNoiseOff;
/** 1 where the W, E, N, S neighbour chunk shares this atmosphere; else fade out toward it. */
uniform vec4 uEdges;

float hash21(vec2 p) {
    p = fract(p * vec2(123.34, 456.21));
    p += dot(p, p + 45.32);
    return fract(p.x * p.y);
}

// Value noise from the shared lattice texture (period 512): linear filtering at
// smoothstep-warped coordinates interpolates exactly like the smoothstep blend of 4 corners.
float vnoise(vec2 p) {
    vec2 i = floor(p);
    vec2 f = fract(p);
    f = f * f * (3.0 - 2.0 * f);
    return texture(uNoise, (i + f + 0.5) / 512.0).r;
}

// Particles on a grid of cells drifting with velocity v; returns coverage.
float motes(vec2 w, vec2 v, float cell, float size, float density) {
    vec2 p = (w + v * uTime) / cell;
    vec2 c = floor(p);
    vec2 h = vec2(hash21(mod(c, 512.0)), hash21(mod(c + 3.7, 512.0)));
    if (h.x > density) return 0.0;
    vec2 center = 0.2 + 0.6 * h;
    return 1.0 - smoothstep(size * 0.6, size, length(fract(p) - center) * cell);
}

// (World-anchored: uNoiseOff wraps at 65536 squares, and every lattice frequency below is a multiple
// of 1/128, so a wrap shifts each lookup by whole lattice periods (512) and leaves no seam.)
void main() {
    vec2 w = vLocal * 128.0 + uNoiseOff;
    int k = int(uKind + 0.5);
    vec4 c = vec4(0.0);
    if (k == 1 || k == 2) {
        float dens = k == 2 ? 0.55 : 0.32;
        float n = 0.6 * vnoise(w * 0.0703125 + vec2(uTime * 0.02, uTime * 0.01)) + 0.4 * vnoise(w * 0.1875 - vec2(uTime * 0.03, 0.0));
        c = vec4(vec3(0.88, 0.9, 0.9), smoothstep(0.35, 0.85, n) * dens);
    } else if (k == 3) {
        float blink = 0.5 + 0.5 * sin(uTime * 2.0 + hash21(floor(w / 7.0)) * 20.0);
        c = vec4(vec3(1.0, 0.95, 0.5), motes(w, vec2(0.3, -0.2), 7.0, 0.35, 0.25) * blink);
    } else if (k == 4) {
        c = vec4(vec3(1.0), max(motes(w, vec2(0.6, 2.2), 3.0, 0.16, 0.5), motes(w + 11.0, vec2(0.4, 1.6), 5.0, 0.22, 0.4)) * 0.85);
    } else if (k == 5) {
        float streak = vnoise(vec2(w.x * 0.1484375 - uTime * 3.0, w.y * 2.0));
        c = vec4(vec3(0.85, 0.72, 0.5), smoothstep(0.6, 0.9, streak) * 0.35);
    } else if (k == 6) {
        c = vec4(vec3(1.0, 0.55, 0.15), motes(w, vec2(0.3, -1.4), 4.0, 0.12, 0.3));
    } else if (k == 7) {
        c = vec4(vec3(1.0, 0.95, 0.85), 0.05 * vnoise(w * 0.5 + uTime));
    } else if (k == 8) {
        c = vec4(vec3(0.75, 0.45, 0.15), motes(w, vec2(0.8, 1.0), 6.0, 0.22, 0.2));
    } else if (k == 9) {
        float streak = vnoise(vec2(w.x * 3.0 + w.y * 0.296875, w.y * 0.203125 - uTime * 6.0));
        c = vec4(vec3(0.75, 0.82, 0.9), smoothstep(0.75, 0.95, streak) * 0.35);
    }
    vec2 l = vLocal;
    float edge = min(min(mix(smoothstep(0.0, 0.3, l.x), 1.0, uEdges.x), mix(smoothstep(0.0, 0.3, 1.0 - l.x), 1.0, uEdges.y)),
                     min(mix(smoothstep(0.0, 0.3, l.y), 1.0, uEdges.z), mix(smoothstep(0.0, 0.3, 1.0 - l.y), 1.0, uEdges.w)));
    float a = c.a * uAlpha * edge;
    finalColor = vec4(c.rgb * a, a);
}
