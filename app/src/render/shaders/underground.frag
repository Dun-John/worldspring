#version 300 es
// Underground levels, painterly (the battlemap's style): smooth floor outlines from a blurred
// floor field (rock walls roughened by noise in caves and mines, near-straight dressed stone in
// built sites), a lit inked rim where the wall meets the floor, deep textured rock beyond,
// ambient occlusion and NW-light shadows on the floor, floors by material (cave rock, basalt,
// mine earth, sewer brick, flagstones), animated sewage or lava channels, ledges as smooth
// contours with their shadows, terrain (water with slow ripples, chasms, rubble slopes), and the
// 5-ft grid.
//
// uField: 4 texels per square, linear: r floor (blurred mask), g liquid (blurred), b raised
//         floor (blurred, raise / 10 ft). Built sites (uSquare): nearest, the floor unblurred;
//         their walls are measured from the squares instead, keeping square corners.
// uCells: one texel per square, nearest: r material, g shape (0 none, a shaped room's slot + 1
//         in uShapes, 255 a cavern breaking in), b variation, a floor.
// uShapes: shaped rooms (round, octagon, apses, chamfered: `roomShapes.ts`), one column each:
//         row 0 the box round its squares (x0, y0, x1, y1), row 1 shape, material, variation.
//         Their squares are left out of uField and the squares' walls; the floor is drawn from
//         the shape near them (it may stand up to half a square off the squares).
// uCave: caverns breaking into a built level, linear: their floor (and the squares opening into
//         them) blurred as a cave's, drawn with rough rock walls.
// uTerrain: terrain (`kit::TERRAIN`), linear, 4 texels per square: r water (shallow, and under
//         deep water), g deep water, b chasm, a rubble slope; each blurred, spread into the rock
//         beside it (so it meets the walls). Read only where uHasTerrain.

in vec2 vUV;
in vec2 vLocal;

out vec4 finalColor;

uniform sampler2D uNoise;
uniform sampler2D uField;
uniform sampler2D uCells;
uniform sampler2D uShapes;
uniform sampler2D uCave;
uniform sampler2D uTerrain;

uniform vec2 uN;
uniform vec2 uLight;
uniform float uTime;
uniform float uRough;
uniform float uSquare;
uniform float uLiquid;
uniform float uGhost;
uniform float uGridAlpha;
// Where this grid sits among its neighbours (squares; a city's sewer sections, so their
// textures run on across the edges), else 0.
uniform vec2 uOrigin;
// Shaped rooms on the level (0: none), caverns breaking in (0 or 1).
uniform float uShapeCount;
uniform float uCaverns;
uniform float uHasTerrain;
uniform vec4 uColor;

const vec3 INK = vec3(0.10, 0.085, 0.07);

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

float fbm(vec2 p) {
    return 0.55 * vnoise(p) + 0.3 * vnoise(p * 2.13 + 7.1) + 0.15 * vnoise(p * 4.37 + 3.3);
}

vec4 field(vec2 q) {
    return texture(uField, q / uN);
}

// Thin dark seams where noise crosses a level (cracks in rock and stone): about `w` of the
// noise's range wide, never wider than ~1.5 px (so close up they stay ink lines, not bands).
float seam(float v, float w) {
    float px = fwidth(v) * 1.5;
    return 1.0 - smoothstep(0.0, max(min(w, px), 1e-4), abs(v - 0.5));
}

// (`p`: position for noise, continuous from one section of sewers to the next.)
vec3 rockMass(vec2 p, float depth) {
    vec3 c = vec3(0.25, 0.23, 0.21);
    c *= 0.72 + 0.5 * vnoise(p * 0.55) + 0.12 * (vnoise(p * 2.4 + 5.0) - 0.5);
    // Fractures.
    c = mix(c, c * 0.6, seam(vnoise(p * 1.1 + 3.0), 0.03));
    // Deeper into the rock is darker.
    return c * mix(1.0, 0.5, smoothstep(0.0, 2.5, depth));
}

// A square drawn from the squares: no shape on it (0, or 254: one next to it).
bool fromSquares(float g) {
    float c = g * 255.0;
    return c < 0.5 || abs(c - 254.0) < 0.5;
}

// Built sites: signed distance (squares, + on the floor) to the nearest square of the other
// kind (rock or floor; off the grid is rock), measured square-wise (the larger of the x and
// y gaps), so the wall, its coping and its shadow all keep square corners; and the way from
// the rock toward the floor. Exact within two squares (deeper rock reads as two).
void boxField(vec2 q, out float d, out vec2 n) {
    ivec2 s = clamp(ivec2(floor(q)), ivec2(0), ivec2(uN) - 1);
    vec4 own = texelFetch(uCells, s, 0);
    bool fl = own.a > 0.5 && fromSquares(own.g);
    float best = 2.0;
    vec2 way = vec2(0.0);
    for (int j = -2; j <= 2; j++) {
        for (int i = -2; i <= 2; i++) {
            // (The outer ring is no nearer than one square: only needed when nothing is.)
            if (max(abs(i), abs(j)) == 2 && best < 1.0) continue;
            ivec2 c = s + ivec2(i, j);
            bool inside = all(greaterThanEqual(c, ivec2(0))) && all(lessThan(c, ivec2(uN)));
            // (A shaped room's squares are drawn from its shape.)
            vec4 t = inside ? texelFetch(uCells, c, 0) : vec4(0.0);
            bool f = t.a > 0.5 && fromSquares(t.g);
            if (f == fl) continue;
            vec2 lo = vec2(c);
            vec2 gap = max(max(lo - q, q - lo - 1.0), 0.0);
            float m = max(gap.x, gap.y);
            if (m < best) {
                best = m;
                way = q - clamp(q, lo, lo + 1.0);
            }
        }
    }
    d = fl ? best : -best;
    n = length(way) > 1e-5 ? normalize(fl ? way : -way) : vec2(0.0);
}

// Shapes: signed distance (squares, + inside) to a box, an ellipse (about its middle; close to
// the edge, as the walls need), an apse (its round end at low y), and to each shape in its box.
float boxIn(vec2 p, vec2 lo, vec2 hi) {
    vec2 d = min(p - lo, hi - p);
    if (d.x >= 0.0 && d.y >= 0.0) return min(d.x, d.y);
    return -length(max(-d, 0.0));
}

float ellipseIn(vec2 p, vec2 r) {
    float k0 = length(p / r);
    float k1 = length(p / (r * r));
    return k0 < 1e-4 ? min(r.x, r.y) : (1.0 - k0) * k0 / k1;
}

float apseIn(vec2 p, vec2 lo, vec2 hi) {
    vec2 sz = hi - lo;
    float r = min(sz.x * 0.5, sz.y - 1.0);
    if (r < 1.0) return boxIn(p, lo, hi);
    float rect = boxIn(p, vec2(lo.x, lo.y + r), hi);
    float end = min(ellipseIn(p - vec2(lo.x + sz.x * 0.5, lo.y + r), vec2(sz.x * 0.5, r)), lo.y + r - p.y);
    return max(rect, end);
}

// 1 round, 2 octagon, 3-6 apse north, east, south, west, 7 chamfered (`interior/shape.rs`).
float shapeIn(int code, vec2 p, vec2 lo, vec2 hi) {
    vec2 c = 0.5 * (lo + hi);
    vec2 hs = 0.5 * (hi - lo);
    if (code == 1) {
        float r = min(hs.x, hs.y);
        vec2 e = abs(p - c) - (hs - r);
        return -(length(max(e, 0.0)) + min(max(e.x, e.y), 0.0) - r);
    }
    if (code == 2 || code == 7) {
        float side = 2.0 * min(hs.x, hs.y);
        float cut = code == 2 ? 0.29289322 * side : (side >= 3.0 ? 0.75 : 0.5);
        vec2 d = abs(p - c);
        return min(boxIn(p, lo, hi), (hs.x + hs.y - cut - d.x - d.y) * 0.70710678);
    }
    if (code == 3) return apseIn(p, lo, hi);
    if (code == 5) return apseIn(vec2(p.x, -p.y), vec2(lo.x, -hi.y), vec2(hi.x, -lo.y));
    if (code == 6) return apseIn(p.yx, lo.yx, hi.yx);
    if (code == 4) return apseIn(vec2(p.y, -p.x), vec2(lo.y, -hi.x), vec2(hi.y, -lo.x));
    return boxIn(p, lo, hi);
}

// Shaped room `slot` (1-based): its box and look (shape, material, variation).
void shapeSlot(int slot, out vec2 lo, out vec2 hi, out vec3 look) {
    vec4 b = floor(texelFetch(uShapes, ivec2(slot - 1, 0), 0) * 255.0 + 0.5);
    lo = b.xy;
    hi = b.zw;
    look = floor(texelFetch(uShapes, ivec2(slot - 1, 1), 0).rgb * 255.0 + 0.5);
}

vec3 floorColor(int mat, vec2 q, vec2 p, float var) {
    vec3 c;
    if (mat == 0) {
        // Cave floor: mottled rock in broad patches, fine cracks, grit (relief and pebbles
        // are added for every natural floor).
        c = vec3(0.49, 0.45, 0.40) * (0.84 + 0.24 * vnoise(p * 0.35));
        c = mix(c, c * vec3(0.94, 0.96, 1.0), smoothstep(0.4, 0.7, vnoise(p * 0.25 + 20.0)) * 0.4);
        c = mix(c, c * 0.68, seam(vnoise(p * 0.9 + 4.0), 0.02));
        c = mix(c, c * 0.82, smoothstep(0.8, 0.92, vnoise(p * 6.3 + 9.0)) * 0.5);
    } else if (mat == 1) {
        // Basalt: dark, columnar cracks, glints.
        c = vec3(0.30, 0.28, 0.27) * (0.85 + 0.25 * fbm(p * 0.8));
        c = mix(c, c * 0.45, seam(vnoise(p * 1.9 + 2.0), 0.04));
        c += vec3(0.12) * smoothstep(0.93, 0.98, vnoise(p * 9.0));
    } else if (mat == 2) {
        // Mine floor: packed earth and gravel.
        c = vec3(0.47, 0.39, 0.30) * (0.82 + 0.3 * fbm(p * 0.6));
        c = mix(c, vec3(0.40, 0.38, 0.36), smoothstep(0.75, 0.88, vnoise(p * 6.0)) * 0.6);
        c = mix(c, c * 0.75, smoothstep(0.4, 0.6, vnoise(p * vec2(0.6, 3.0))) * 0.3);
    } else if (mat == 3) {
        // Sewer walkway: running-bond brick, grimy.
        vec2 b = q * vec2(2.0, 4.0);
        b.x += 0.5 * mod(floor(b.y), 2.0);
        vec2 g = fract(b);
        float mortar = 1.0 - smoothstep(0.0, 0.08, min(min(g.x, 1.0 - g.x) * 0.5, min(g.y, 1.0 - g.y)));
        c = vec3(0.50, 0.40, 0.33) * (0.82 + 0.3 * hash21(floor(b)));
        c = mix(c, vec3(0.30, 0.29, 0.26), mortar * 0.8);
        c = mix(c, c * vec3(0.7, 0.75, 0.6), smoothstep(0.45, 0.8, fbm(p * 0.5 + 2.0)) * 0.5);
    } else if (mat == 7) {
        // Marble: a square slab per square, light and dark in turn, veined.
        vec2 cell = floor(q);
        vec2 g = fract(q);
        c = mix(vec3(0.80, 0.78, 0.74), vec3(0.60, 0.58, 0.56), mod(cell.x + cell.y, 2.0)) * (0.94 + 0.08 * hash21(cell));
        c = mix(c, c * 0.72, seam(fbm(p * 0.9 + cell * 0.37), 0.015));
        float joint = 1.0 - smoothstep(0.0, 0.03, min(min(g.x, 1.0 - g.x), min(g.y, 1.0 - g.y)));
        c = mix(c, INK * 1.8, joint * 0.5);
    } else if (mat == 8) {
        // Hewn rock: big blocks two squares long in running bond, tight joints.
        vec2 b = q * vec2(0.5, 1.0);
        b.x += 0.5 * mod(floor(b.y), 2.0);
        vec2 g = fract(b);
        c = vec3(0.52, 0.49, 0.45) * (0.85 + 0.2 * hash21(floor(b))) * (0.92 + 0.12 * vnoise(p * 2.0));
        float joint = 1.0 - smoothstep(0.0, 0.04, min(min(g.x, 1.0 - g.x) * 2.0, min(g.y, 1.0 - g.y)));
        c = mix(c, INK * 1.5, joint * 0.7);
    } else if (mat == 9) {
        // Ice: pale blue, cracked, glinting.
        c = vec3(0.74, 0.84, 0.90) * (0.88 + 0.16 * fbm(p * 0.5));
        c = mix(c, vec3(0.50, 0.64, 0.78), seam(vnoise(p * 1.3 + 2.0), 0.02));
        c += vec3(0.15) * smoothstep(0.94, 0.99, vnoise(p * 8.0));
    } else {
        // Flagstones: rows of irregular stones; boss chambers larger and darker, crypts dusty.
        float scale = mat == 5 ? 0.7 : mat == 10 ? 0.85 : 1.0;
        vec2 f = q * vec2(1.0, 1.35) * scale;
        float row = floor(f.y);
        f.x += 0.5 * mod(row, 2.0) + 0.17 * sin(row * 3.1);
        vec2 cell = floor(f);
        vec2 g = fract(f);
        float edge = min(min(g.x, 1.0 - g.x) * 1.35, min(g.y, 1.0 - g.y));
        edge += 0.04 * (vnoise(p * 6.0) - 0.5);
        float mortar = 1.0 - smoothstep(0.02, 0.07, edge);
        vec3 stone = mat == 5 ? vec3(0.44, 0.39, 0.37) : mat == 6 ? vec3(0.60, 0.56, 0.48) : mat == 10 ? vec3(0.30, 0.28, 0.30) : vec3(0.58, 0.55, 0.50);
        c = stone * (0.8 + 0.32 * hash21(cell + var)) * (0.9 + 0.15 * vnoise(p * 3.0));
        c = mix(c, INK * 1.6, mortar * 0.75);
    }
    return c;
}

// Slow ripples on water (0-1): light streaks wavering where two drifting noises cross, and now
// and then a drip's rings spreading. `px`: squares a pixel.
float ripples(vec2 p, float px) {
    float n1 = vnoise(p * 1.9 + vec2(uTime * 0.11, uTime * 0.05));
    float n2 = vnoise(p * 2.3 + vec2(-uTime * 0.07, uTime * 0.09) + 17.0);
    float streak = (1.0 - smoothstep(0.0, 0.035, abs(n1 - n2))) * smoothstep(0.35, 0.7, vnoise(p * 0.6 + uTime * 0.02));
    // A drip in some cells of two squares, its two rings spreading and fading.
    vec2 sc = p * 0.5;
    vec2 id = floor(sc);
    float ph = fract(uTime * 0.16 + hash21(id + 1.9));
    float dist = length(fract(sc) - (0.35 + 0.3 * vec2(hash21(id + 3.7), hash21(id + 8.1))));
    float lw = max(0.018, px * 0.6);
    float rings = (1.0 - smoothstep(0.0, lw, abs(dist - ph * 0.3))) + 0.6 * (1.0 - smoothstep(0.0, lw, abs(dist - ph * 0.18)));
    rings *= (1.0 - ph) * step(0.55, hash21(id + 5.3));
    return clamp(streak * 0.55 + rings * 0.8, 0.0, 1.0);
}

// One stone of scree to each cell of a grid `s` a square (where a cell's hash is under
// `cover`): two tones, its NW half lit, its shadow to the SE, inked.
vec3 screeStone(vec3 col, vec2 p, float s, float seed, float cover, float px) {
    vec2 sc = p * s;
    vec2 id = floor(sc);
    float h = hash21(id + seed);
    if (h >= cover) return col;
    vec2 dv = fract(sc) - (0.4 + 0.2 * vec2(hash21(id + seed + 1.3), hash21(id + seed + 2.7)));
    float a = atan(dv.y, dv.x);
    float r = (0.21 + 0.1 * hash21(id + seed + 4.1)) * (1.0 + 0.16 * sin(a * 3.0 + h * 40.0) + 0.07 * sin(a * 5.0 + h * 17.0));
    float len = length(dv);
    float pw = min(px * s, 0.06);
    col = mix(col, col * 0.5, (1.0 - smoothstep(r - pw, r + 0.07, length(dv + uLight * 0.1))) * 0.4);
    vec3 stone = vec3(0.70, 0.67, 0.62) * (0.82 + 0.3 * hash21(id + seed + 5.5));
    stone *= mix(0.66, 1.12, smoothstep(-0.2, 0.2, dot(dv, uLight) / max(r, 1e-3) + 0.3));
    col = mix(col, stone, 1.0 - smoothstep(r - pw, r + pw, len));
    return mix(col, INK, (1.0 - smoothstep(0.0, max(pw * 1.2, 0.05), abs(len - r))) * 0.55);
}

// Terrain on the floor (uTerrain): shallow water over the floor, deep water, a chasm, a rubble
// slope, each from its mask's iso-0.5 bank (roughened by noise, more in rock than in dressed stone).
vec3 terrain(vec3 col, vec2 q, vec2 p, float px) {
    vec4 t = texture(uTerrain, q / uN);
    if (max(max(t.r, t.g), max(t.b, t.a)) < 0.02) return col;
    float e = 0.25;
    vec4 gx = (texture(uTerrain, (q + vec2(e, 0.0)) / uN) - texture(uTerrain, (q - vec2(e, 0.0)) / uN)) / (2.0 * e);
    vec4 gy = (texture(uTerrain, (q + vec2(0.0, e)) / uN) - texture(uTerrain, (q - vec2(0.0, e)) / uN)) / (2.0 * e);
    vec4 gl = sqrt(gx * gx + gy * gy);
    float wob = (fbm(p * 1.1 + 11.0) - 0.5) * mix(0.12, 0.34, uRough);
    vec4 d = (t - 0.5) / max(gl, vec4(0.35)) + wob * vec4(1.0, 1.0, 0.8, 1.6);
    float aa = max(px * 0.5, 1e-3);
    if (t.r > 0.02) {
        // Shallow water: the floor seen through it, tinted and dimmer toward the middle; wet
        // stone round it, a light line at the edge, inked.
        float ds = d.x;
        float inside = smoothstep(-aa, aa, ds);
        col *= mix(1.0, 0.78, (1.0 - smoothstep(0.0, 0.2, -ds)) * (1.0 - inside));
        vec3 w = col * vec3(0.55, 0.74, 0.76) + vec3(0.04, 0.10, 0.11);
        w *= mix(1.0, 0.84, smoothstep(0.05, 0.7, ds));
        w += ripples(p, px) * vec3(0.30, 0.36, 0.36);
        col = mix(col, w, inside);
        col = mix(col, vec3(0.80, 0.90, 0.88), (1.0 - smoothstep(0.0, max(aa * 2.0, 0.025), abs(ds - 0.05))) * 0.4 * inside);
        col = mix(col, INK, (1.0 - smoothstep(0.0, aa * 2.0, abs(ds))) * 0.5);
    }
    if (t.g > 0.02) {
        // Deep water: dark, lighter where it shelves up to its edge, shadowed under the NW
        // bank; ripples; a wet glint along its rim, inked.
        float dd = d.y;
        float inside = smoothstep(-aa, aa, dd);
        vec3 w = vec3(0.06, 0.17, 0.25) * (0.85 + 0.3 * vnoise(p * 0.7 + vec2(uTime * 0.03, 0.0)));
        w = mix(vec3(0.17, 0.36, 0.41), w, smoothstep(0.0, 0.45, dd));
        w *= mix(0.62, 1.0, smoothstep(0.3, 0.6, texture(uTerrain, (q + uLight * 0.4) / uN).g));
        w += ripples(p, px) * vec3(0.30, 0.45, 0.50);
        col = mix(col, w, inside);
        col = mix(col, vec3(0.62, 0.80, 0.84), (1.0 - smoothstep(0.0, max(aa * 2.0, 0.03), abs(dd - 0.08))) * 0.55);
        col = mix(col, INK, (1.0 - smoothstep(0.0, aa * 2.4, abs(dd))) * 0.8);
    }
    if (t.b > 0.02) {
        // A chasm: a lip of rock round it lit as it faces the NW light, the rock face below it
        // (strata along the edge) falling into black; inked bold at the drop.
        float dc = d.z;
        float inside = smoothstep(-aa, aa, dc);
        vec2 nc = vec2(gx.b, gy.b);
        nc = length(nc) > 1e-4 ? normalize(nc) : vec2(0.0);
        float face = clamp(0.55 + 0.7 * dot(nc, uLight), 0.2, 1.2);
        float lipW = 0.11 + 0.08 * vnoise(p * 2.7 + 5.0);
        float lip = (1.0 - smoothstep(lipW - aa, lipW + aa, -dc)) * (1.0 - inside);
        vec3 lipCol = vec3(0.60, 0.57, 0.52) * (0.85 + 0.3 * vnoise(p * 4.0)) * mix(0.8, 1.15, smoothstep(-0.5, 0.5, dot(nc, uLight)));
        col = mix(col, lipCol, lip * 0.9);
        col = mix(col, INK, (1.0 - smoothstep(0.0, aa * 1.6, abs(dc + lipW))) * 0.55);
        vec3 rf = rockMass(p * 1.7, 0.0) * 2.0 * face;
        rf = mix(rf, rf * 0.5, seam(fract(dc * 4.0 + 0.6 * vnoise(p * 1.5)), 0.06));
        col = mix(col, mix(rf, vec3(0.012, 0.01, 0.008), smoothstep(0.08, 0.75, dc)), inside);
        col = mix(col, INK, (1.0 - smoothstep(0.0, aa * 2.4, abs(dc))) * 0.95);
    }
    if (t.a > 0.02) {
        // A rubble slope: dust over the floor, then scree, small stones spilling out past the
        // big ones at its edge.
        float dr = d.w;
        float cover = smoothstep(-0.25, 0.3, dr);
        col = mix(col, vec3(0.56, 0.53, 0.48) * (0.85 + 0.25 * vnoise(p * 5.0)), cover * 0.3);
        col = screeStone(col, p + 0.37, 2.6, 9.0, smoothstep(-0.45, 0.1, dr) * 0.3, px);
        col = screeStone(col, p, 1.5, 0.0, cover * 0.4, px);
    }
    return col;
}

void main() {
    vec2 q = vUV * uN;
    vec2 p = q + uOrigin;
    // (Squares a pixel.)
    float px = length(fwidth(q));
    vec4 here = field(q);
    float d;
    vec2 n;
    if (uSquare > 0.5) {
        boxField(q, d, n);
    } else {
        // Signed distance to the wall (squares, + on the floor) from the blurred floor field.
        float e = 0.25;
        vec2 grad = vec2(field(q + vec2(e, 0.0)).r - field(q - vec2(e, 0.0)).r, field(q + vec2(0.0, e)).r - field(q - vec2(0.0, e)).r) / (2.0 * e);
        d = (here.r - 0.5) / max(length(grad), 0.35);
        // Rock juts out and bites in (caves); dressed stone stays nearly straight.
        d += uRough * ((fbm(p * 0.8 + 3.0) - 0.5) * 0.75 + (vnoise(p * 3.7) - 0.5) * 0.25 + (vnoise(p * 9.0) - 0.5) * 0.08);
        n = length(grad) > 1e-4 ? normalize(grad) : vec2(0.0);
    }
    // Near shaped rooms and caverns: the floor drawn from them too (the farther in wins).
    int won = 0;
    vec3 look = vec3(0.0);
    float rough = uRough;
    // (Where the floor's edge casts its shadow from: the floor there.)
    vec2 qs = q + uLight * 0.45;
    float lit = field(qs).r;
    ivec2 s0 = clamp(ivec2(floor(q)), ivec2(0), ivec2(uN) - 1);
    // (Only on a shape's squares or those next to one: uCells.g is 0 elsewhere.)
    if ((uShapeCount > 0.5 || uCaverns > 0.5) && texelFetch(uCells, s0, 0).g > 0.5 / 255.0) {
        // The shapes round (four at most), a cavern among them.
        int slots[4] = int[4](0, 0, 0, 0);
        int found = 0;
        bool cave = false;
        for (int j = -1; j <= 1; j++) {
            for (int i = -1; i <= 1; i++) {
                ivec2 c = s0 + ivec2(i, j);
                if (any(lessThan(c, ivec2(0))) || any(greaterThanEqual(c, ivec2(uN)))) continue;
                int g = int(texelFetch(uCells, c, 0).g * 255.0 + 0.5);
                if (g == 255) cave = true;
                if (g == 0 || g >= 254 || found == 4) continue;
                bool seen = false;
                for (int m = 0; m < 4; m++) seen = seen || (m < found && slots[m] == g);
                if (!seen) {
                    slots[found] = g;
                    found++;
                }
            }
        }
        for (int k = 0; k < 4; k++) {
            if (k >= found) break;
            int slot = slots[k];
            vec2 lo;
            vec2 hi;
            vec3 lk;
            shapeSlot(slot, lo, hi, lk);
            int code = int(lk.x);
            float ds = shapeIn(code, q, lo, hi);
            lit = max(lit, smoothstep(-0.05, 0.05, shapeIn(code, qs, lo, hi)));
            if (ds > d) {
                float e = 0.03;
                vec2 gs = vec2(shapeIn(code, q + vec2(e, 0.0), lo, hi) - shapeIn(code, q - vec2(e, 0.0), lo, hi), shapeIn(code, q + vec2(0.0, e), lo, hi) - shapeIn(code, q - vec2(0.0, e), lo, hi));
                d = ds;
                n = length(gs) > 1e-6 ? normalize(gs) : vec2(0.0);
                won = slot;
                look = lk;
            }
        }
        if (cave) {
            float e = 0.25;
            float c0 = texture(uCave, q / uN).r;
            vec2 gc = vec2(texture(uCave, (q + vec2(e, 0.0)) / uN).r - texture(uCave, (q - vec2(e, 0.0)) / uN).r, texture(uCave, (q + vec2(0.0, e)) / uN).r - texture(uCave, (q - vec2(0.0, e)) / uN).r) / (2.0 * e);
            float dc = (c0 - 0.5) / max(length(gc), 0.35);
            dc += (fbm(p * 0.8 + 3.0) - 0.5) * 0.75 + (vnoise(p * 3.7) - 0.5) * 0.25 + (vnoise(p * 9.0) - 0.5) * 0.08;
            lit = max(lit, smoothstep(0.35, 0.6, texture(uCave, qs / uN).r));
            if (dc > d) {
                d = dc;
                n = length(gc) > 1e-4 ? normalize(gc) : vec2(0.0);
                won = 255;
                rough = 1.0;
            }
        }
    }
    float aa = max(fwidth(d), 1e-4);

    vec3 col;
    float alpha;
    float rim = mix(0.16, 0.3, rough);
    if (d < 0.0) {
        // Rock: the wall's top edge lit from the NW, then the rock mass.
        vec3 r = rockMass(p, -d);
        float face = clamp(0.55 + 0.6 * dot(n, uLight), 0.25, 1.15);
        vec3 lip = rockMass(p * 1.3, 0.0) * 1.55 * face;
        if (rough < 0.2) {
            // Dressed stone coping on built walls.
            vec2 b = q * vec2(1.6, 1.6);
            vec2 g = fract(b + 0.5 * mod(floor(b.yx), 2.0));
            float joint = 1.0 - smoothstep(0.0, 0.06, min(min(g.x, 1.0 - g.x), min(g.y, 1.0 - g.y)));
            lip = mix(vec3(0.56, 0.53, 0.48) * (0.85 + 0.25 * hash21(floor(b))) * face, INK, joint * 0.5);
        }
        col = mix(lip, r, smoothstep(-rim, -rim - 0.12, d));
        alpha = mix(1.0, 0.9, smoothstep(-rim, -rim - 0.5, d));
    } else {
        ivec2 s = clamp(ivec2(floor(q)), ivec2(0), ivec2(uN) - 1);
        vec4 cell = texelFetch(uCells, s, 0) * 255.0;
        if (won > 0 && won < 255) {
            // A shaped room's floor, wherever its shape reaches.
            cell = vec4(look.y, 0.0, look.z, 255.0);
        } else if (won == 255) {
            cell = vec4(0.0, 255.0, cell.b, 255.0);
        } else if (cell.a < 0.5) {
            // A smoothed edge reaching into a rock square: the floor next to it.
            cell = texelFetch(uCells, clamp(s + ivec2(round(n)), ivec2(0), ivec2(uN) - 1), 0) * 255.0;
        }
        int mat = int(cell.r + 0.5);
        col = floorColor(mat, q, p, cell.b);
        if (mat <= 2) {
            // Natural floors: uneven rock lit from the NW, loose stones, rubble at the wall foot.
            float b0 = vnoise(p * 0.7);
            vec2 bg = vec2(vnoise(p * 0.7 + vec2(0.12, 0.0)) - b0, vnoise(p * 0.7 + vec2(0.0, 0.12)) - b0);
            col *= 1.0 + clamp(dot(bg, uLight) * 3.2, -0.15, 0.15);
            vec2 sc = q * 1.7;
            vec2 cid = floor(sc);
            vec2 off = vec2(hash21(cid), hash21(cid + 7.3)) * 0.6 + 0.2;
            float pr = 0.09 + 0.08 * hash21(cid + 3.1);
            vec2 dv = fract(sc) - off;
            if (hash21(cid + 1.7) > 0.72) {
                float stone = 1.0 - smoothstep(pr - 0.03, pr, length(dv));
                float shade = 1.0 - smoothstep(pr * 0.4, pr + 0.06, length(dv + uLight * pr * 0.6));
                col = mix(col, col * 0.55, (1.0 - smoothstep(pr, pr + 0.07, length(dv - uLight * 0.05))) * 0.6);
                col = mix(col, col * (1.05 + 0.25 * shade), stone);
            }
            float foot = (1.0 - smoothstep(0.05, 0.6, d)) * rough;
            float rub = smoothstep(0.45, 0.75, vnoise(p * 4.5 + 13.0));
            col = mix(col, mix(col * 0.6, col * 1.2, vnoise(p * 9.0)), foot * rub * 0.8);
        }
        // Raised floors (ledges): lighter tops, inked lips, shadows below them.
        float t = here.b;
        col *= 1.0 + 0.22 * t;
        float tw = max(fwidth(t), 1e-4);
        float lips = (1.0 - smoothstep(0.0, tw * 1.5, abs(t - 0.25))) + (1.0 - smoothstep(0.0, tw * 1.5, abs(t - 0.75)));
        col = mix(col, INK, clamp(lips, 0.0, 1.0) * 0.8);
        col *= mix(1.0, 0.72, smoothstep(0.1, 0.3, field(q + uLight * 0.35).b - t));
        // Water, chasms, rubble slopes.
        if (uHasTerrain > 0.5) col = terrain(col, q, p, px);
        // Against the walls: ambient occlusion and the shadow cast from the NW.
        col *= 0.6 + 0.4 * smoothstep(0.0, 0.75, d);
        col *= mix(0.66, 1.0, smoothstep(0.35, 0.6, lit));
        // Liquid down the channel.
        float l = here.g;
        if (uLiquid > 0.5) {
            float lw = max(fwidth(l), 1e-3);
            float inside = smoothstep(0.5 - lw, 0.5 + lw, l);
            vec3 liq;
            if (uLiquid < 1.5) {
                float n1 = vnoise(p * 1.4 + vec2(uTime * 0.22, uTime * 0.09));
                float n2 = vnoise(p * 4.2 - vec2(uTime * 0.35, 0.0));
                liq = vec3(0.21, 0.24, 0.14) * (0.78 + 0.4 * n1);
                liq += vec3(0.14, 0.15, 0.07) * smoothstep(0.72, 0.86, n2);
                liq += vec3(0.08) * smoothstep(0.9, 0.97, vnoise(p * 7.0 + uTime * 0.5));
                // Wet grime on the walkway's edge.
                col *= mix(1.0, 0.8, smoothstep(0.15, 0.45, l));
            } else {
                float n1 = vnoise(p * 1.2 + vec2(uTime * 0.05, uTime * 0.03));
                liq = mix(vec3(0.92, 0.26, 0.05), vec3(1.0, 0.78, 0.25), n1);
                float crust = smoothstep(0.58, 0.7, vnoise(p * 2.3 + uTime * 0.04));
                liq = mix(liq, vec3(0.16, 0.08, 0.05), crust * 0.85);
                // Glow on the floor beside it.
                col += vec3(0.5, 0.17, 0.03) * smoothstep(0.08, 0.5, l) * 0.7;
            }
            col = mix(col, liq, inside);
            col = mix(col, INK, (1.0 - smoothstep(0.0, lw * 2.0, abs(l - 0.5))) * 0.55);
        }
        // The 5-ft grid.
        if (uGhost < 0.5) {
            vec2 gq = min(fract(q), 1.0 - fract(q));
            float gw = length(fwidth(q));
            // (Light over what is nearly black: a chasm, deep water.)
            vec3 gc = mix(vec3(0.45, 0.43, 0.40), INK, smoothstep(0.05, 0.16, dot(col, vec3(0.3, 0.59, 0.11))));
            col = mix(col, gc, (1.0 - smoothstep(0.0, gw, min(gq.x, gq.y))) * uGridAlpha);
        }
        alpha = 1.0;
    }
    // The wall's inked outline.
    col = mix(col, INK, (1.0 - smoothstep(0.0, aa * 1.6, abs(d))) * 0.85);
    // A neighbouring section: its floors and their rims only, faint.
    if (uGhost > 0.5) alpha *= d < -rim ? 0.0 : 0.42;
    finalColor = vec4(col * alpha, alpha) * uColor;
}
