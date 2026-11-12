#version 300 es
// Hex crawl: every hex the players haven't explored washed over. In the players' window
// (`uMode` 1) a soft land/water wash (`uWash`, made once from the whole-map tile and blurred)
// opaque enough to hide roads and settlements; in the DM's (2) a light hatch. Hexes are looked
// up in `uMask` (one texel per hex, by column and row) and blended across a soft band at their
// edges: each hex's weight falls off with the point's distance to it, the same on both sides.

in vec2 vW;

out vec4 finalColor;

uniform float uFlat;   // 1 flat-topped, 0 pointy
uniform float uS;      // ft across the flats
uniform float uFpp;    // ft per px
uniform float uAlpha;
uniform float uMode;   // 1 wash, 2 hatch
uniform float uBand;   // the soft edge (ft, each side)
uniform vec4 uMap;     // the map (ft from the reference hex centre): x0, y0, x1, y1
uniform vec2 uRef;     // the reference hex, axial in the shader's (pointy) frame
uniform vec4 uMaskBox; // the mask's first column and row, its columns and rows
uniform vec2 uWorld;   // the reference hex centre (world ft)
uniform float uDomain; // ft across the wash texture
uniform float uHasWash;
uniform highp sampler2D uWash;
uniform highp sampler2D uMask;

const float SQRT3 = 1.7320508;
const float SQRT2 = 1.4142136;
const vec3 PAPER = vec3(0.90, 0.87, 0.79);

// Whether the hex at axial (q, r) of the shader's frame (from the reference) is explored.
float seen(vec2 a) {
    ivec2 h = ivec2(floor(a + uRef + 0.5));
    // Back to the grid's own axial, then to its offset column and row (odd-q / odd-r).
    int q = uFlat > 0.5 ? h.y : h.x;
    int r = uFlat > 0.5 ? h.x : h.y;
    int i = uFlat > 0.5 ? q : q + (r - (r & 1)) / 2;
    int j = uFlat > 0.5 ? r + (q - (q & 1)) / 2 : r;
    ivec2 m = ivec2(i + 1, j + 1) - ivec2(uMaskBox.xy);
    if (m.x < 0 || m.y < 0 || m.x >= int(uMaskBox.z) || m.y >= int(uMaskBox.w)) return 0.0;
    return texelFetch(uMask, m, 0).r;
}

// How far (ft) the point `p` is outside the hex centred at `c` (negative: inside).
float outside(vec2 p, vec2 c) {
    vec2 d = abs(p - c);
    return max(d.x, dot(d, vec2(0.5, SQRT3 / 2.0))) - uS * 0.5;
}

float hash(vec2 a) {
    return fract(sin(dot(a, vec2(12.9898, 78.233))) * 43758.5453);
}

void main() {
    if (vW.x < uMap.x || vW.y < uMap.y || vW.x > uMap.z || vW.y > uMap.w) discard;
    vec2 p = uFlat > 0.5 ? vec2(vW.y, vW.x) : vW;
    float R = uS / SQRT3;
    float q = (SQRT3 / 3.0 * p.x - p.y / 3.0) / R;
    float r = (2.0 / 3.0 * p.y) / R;
    vec3 c = vec3(q, r, -q - r);
    vec3 rc = floor(c + 0.5);
    vec3 dc = abs(rc - c);
    if (dc.x > dc.y && dc.x > dc.z) rc.x = -rc.y - rc.z;
    else if (dc.y > dc.z) rc.y = -rc.x - rc.z;
    vec2 a = rc.xy;
    vec2 centre = vec2(uS * (a.x + a.y * 0.5), 1.5 * R * a.y);
    float own = seen(a);
    float v = own;
    float dOwn = outside(p, centre);
    if (dOwn > -uBand) {
        // Near the edge: this hex and its six neighbours, each weighted by how near it is.
        float w = 1.0 - smoothstep(-uBand, uBand, dOwn);
        float sum = w;
        float acc = w * own;
        vec2 dirs[6] = vec2[6](vec2(1.0, 0.0), vec2(0.0, 1.0), vec2(-1.0, 1.0), vec2(-1.0, 0.0), vec2(0.0, -1.0), vec2(1.0, -1.0));
        for (int k = 0; k < 6; k++) {
            vec2 n = a + dirs[k];
            float wk = 1.0 - smoothstep(-uBand, uBand, outside(p, vec2(uS * (n.x + n.y * 0.5), 1.5 * R * n.y)));
            if (wk <= 0.0) continue;
            sum += wk;
            acc += wk * seen(n);
        }
        v = acc / max(sum, 1e-4);
    }
    float hidden = 1.0 - v;
    if (hidden <= 0.002) discard;
    if (uMode < 1.5) {
        // The wash: the land and water as seen from far off, light, with a little more pigment
        // where it meets the explored hexes and a slight tone of its own per hex.
        vec2 uv = (uWorld + vW) / uDomain;
        vec3 col = uHasWash > 0.5 ? texture(uWash, (0.5 + uv * 256.0) / 257.0).rgb : PAPER;
        col *= 0.97 + 0.05 * hash(a + uRef);
        col *= 1.0 - 0.22 * hidden * (1.0 - hidden);
        float alpha = hidden * uAlpha;
        finalColor = vec4(col * alpha, alpha);
    } else {
        // The DM's hatch: a faint tint and thin diagonal lines (screen px).
        float s = fract((gl_FragCoord.x + gl_FragCoord.y) / 9.0);
        float line = 1.0 - smoothstep(0.5, 1.3, abs(s - 0.5) * 9.0 / SQRT2);
        float alpha = hidden * uAlpha * (0.07 + 0.13 * line);
        vec3 col = vec3(0.17, 0.14, 0.11);
        finalColor = vec4(col * alpha, alpha);
    }
}
