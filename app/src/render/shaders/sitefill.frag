#version 300 es
// Flat watabou-style fills with a faint paper grain, and the outlines of buildings and blocks
// inked along their real edges (distance to them in px from the triangle's edge coordinates),
// once buildings are a few pixels across.

in vec4 vColor;
in vec2 vTile;
in vec3 vEdge;

out vec4 finalColor;

uniform float uAlpha;
uniform float uPpf;

const vec3 INK = vec3(0.17, 0.14, 0.11);
/** Outline width inside the edge (px). */
const float LINE_PX = 0.9;

float hash21(vec2 p) {
    p = fract(p * vec2(123.34, 456.21));
    p += dot(p, p + 45.32);
    return fract(p.x * p.y);
}

void main() {
    float grain = 0.97 + 0.06 * hash21(floor(gl_FragCoord.xy * 0.5));
    vec3 px = vEdge / max(fwidth(vEdge), vec3(1e-5));
    float d = min(px.x, min(px.y, px.z));
    float ink = (1.0 - smoothstep(LINE_PX - 0.5, LINE_PX + 0.5, d)) * smoothstep(0.04, 0.12, uPpf);
    float a = max(vColor.a, ink) * uAlpha;
    if (a <= 0.001) discard;
    vec3 col = mix(vColor.rgb * grain, INK, ink);
    finalColor = vec4(col * a, a);
}
