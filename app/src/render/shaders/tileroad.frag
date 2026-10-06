#version 300 es
// Road capsule: an ink line at map scale; once the bed is a few px wide, a packed-earth or
// paved band with inked edges.

in vec2 vP;
flat in vec2 vSA;
flat in vec2 vSB;
flat in float vHalf;
flat in float vClass;
flat in float vFade;
flat in float vBed;

out vec4 finalColor;

flat in float vPpf;

uniform float uAlpha;

const vec3 INK = vec3(0.36, 0.22, 0.14);
const vec3 INK_TRACK = vec3(0.50, 0.36, 0.24);
const vec3 PAVED = vec3(0.62, 0.58, 0.52);
const vec3 EARTH = vec3(0.64, 0.52, 0.36);

void main() {
    vec2 pa = vP - vSA;
    vec2 ba = vSB - vSA;
    float h = clamp(dot(pa, ba) / max(dot(ba, ba), 1e-6), 0.0, 1.0);
    float d = length(pa - ba * h);
    float cover = 1.0 - smoothstep(vHalf - 0.5, vHalf + 0.5, d);
    bool street = vClass > 2.5;
    bool paved = vClass < 0.5 || street;
    vec3 line = street ? (vClass > 3.5 ? INK : PAVED) : (vClass > 1.5 ? INK_TRACK : INK);
    // Bed: surface in the middle, ink along the edges (town streets meet blocks, no ink).
    vec3 surface = paved ? PAVED : EARTH;
    if (paved && vPpf > 1.0) {
        // Cobbles: offset rows of setts along the street, in feet.
        vec2 dir = ba / max(length(ba), 1e-6);
        vec2 g = vec2(dot(pa, dir) / 1.6, (pa.x * dir.y - pa.y * dir.x) / 1.2) / vPpf;
        g.x += 0.5 * mod(floor(g.y), 2.0);
        vec2 f = fract(g) - 0.5;
        float sett = smoothstep(0.5, 0.34, max(abs(f.x), abs(f.y)));
        float n = fract(sin(dot(floor(g), vec2(12.9898, 78.233))) * 43758.5453);
        vec3 cob = mix(PAVED * 0.74, PAVED * (0.9 + 0.18 * n), sett);
        surface = mix(surface, cob, smoothstep(1.0, 3.0, vPpf));
    }
    float edge = street ? 0.0 : smoothstep(vHalf - 1.4, vHalf - 0.6, d);
    vec3 bed = mix(surface, INK, edge * 0.8);
    vec3 col = mix(line, bed, vBed);
    float a = cover * vFade * uAlpha * mix(0.9, 1.0, vBed);
    // A ferry's line: dashes of track ink across the water.
    if (vClass > 4.5) {
        float s = dot(pa, ba) / max(length(ba), 1e-6);
        float dash = step(0.45, fract(s / max(6.0, 4.0 * vHalf)));
        col = INK_TRACK;
        a = cover * vFade * uAlpha * 0.95 * dash;
    }
    if (a <= 0.001) discard;
    finalColor = vec4(col * a, a);
}
