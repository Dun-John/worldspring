#version 300 es
// Flat watabou-style fills with a faint paper grain, and the outlines of buildings and blocks
// inked along their real edges (distance to them in px from the triangle's edge coordinates),
// once buildings are a few pixels across.

in vec4 vColor;
in vec2 vTile;
in vec3 vEdge;
// v63 countryside: a field's furrow coordinate (ft across its furrows) and style (0 not a field, else
// 1 + use (0 arable, 1 pasture, 2 fallow, 3 orchard) + 4 bound (1 hedge, 2 wall) + 16 strips).
in vec2 vField;

out vec4 finalColor;

uniform float uAlpha;
uniform float uPpf;

const vec3 INK = vec3(0.17, 0.14, 0.11);
/** Outline width inside the edge (px). */
const float LINE_PX = 0.9;
// v63 countryside: furrows' and open-field strips' spacing (ft), field bounds' ink.
const float FURROW_FT = 14.0;
const float STRIP_FT = 38.0;
const vec3 HEDGE = vec3(0.24, 0.36, 0.19);
const vec3 WALL = vec3(0.47, 0.46, 0.43);

float hash21(vec2 p) {
    p = fract(p * vec2(123.34, 456.21));
    p += dot(p, p + 45.32);
    return fract(p.x * p.y);
}

// v63 countryside: a field's furrows (soft darker bands, gone once they crowd under ~4 px), an open
// field's strips (each its own shade, a pale balk between), and the ink and width of its bound.
vec3 fieldLook(vec3 col, int code, float fw, out vec3 ink, out float lw) {
    int use = code & 3;
    int bound = (code >> 2) & 3;
    ink = bound == 1 ? HEDGE : WALL;
    lw = bound == 1 ? clamp(4.0 * uPpf, 1.4, 4.0) : clamp(2.5 * uPpf, 1.1, 2.5);
    float c = vField.x;
    if (use == 0 || use == 2) {
        float period = FURROW_FT / fw;
        float dpx = abs(fract(c / FURROW_FT) - 0.5) * period;
        float band = 1.0 - smoothstep(0.18 * period - 0.5, 0.18 * period + 0.5, dpx);
        col *= 1.0 - (use == 2 ? 0.06 : 0.12) * band * smoothstep(3.0, 6.0, period);
    }
    if ((code & 16) != 0) {
        float s = c / STRIP_FT;
        float period = STRIP_FT / fw;
        float show = smoothstep(4.0, 8.0, period);
        col *= 1.0 + (0.12 * hash21(vec2(floor(s), 7.3)) - 0.06) * show;
        float dpx = min(fract(s), 1.0 - fract(s)) * period;
        col = mix(col, vec3(0.62, 0.68, 0.45), (1.0 - smoothstep(0.4, 1.2, dpx)) * 0.7 * show);
    }
    return col;
}

void main() {
    float grain = 0.97 + 0.06 * hash21(floor(gl_FragCoord.xy * 0.5));
    vec3 px = vEdge / max(fwidth(vEdge), vec3(1e-5));
    float d = min(px.x, min(px.y, px.z));
    vec3 inkCol = INK;
    float lw = LINE_PX;
    vec3 fill = vColor.rgb;
    // (Derivatives taken outside the branch.)
    float fw = max(fwidth(vField.x), 1e-4);
    if (vField.y > 0.5) fill = fieldLook(fill, int(vField.y + 0.5) - 1, fw, inkCol, lw);
    float ink = (1.0 - smoothstep(lw - 0.5, lw + 0.5, d)) * smoothstep(0.04, 0.12, uPpf);
    float a = max(vColor.a, ink) * uAlpha;
    if (a <= 0.001) discard;
    vec3 col = mix(fill * grain, inkCol, ink);
    finalColor = vec4(col * a, a);
}
