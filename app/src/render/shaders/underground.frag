#version 300 es
// Underground levels, painterly (the battlemap's style): smooth floor outlines from a blurred
// floor field (rock walls roughened by noise in caves and mines, near-straight dressed stone in
// built sites), a lit inked rim where the wall meets the floor, deep textured rock beyond,
// ambient occlusion and NW-light shadows on the floor, floors by material (cave rock, basalt,
// mine earth, sewer brick, flagstones), animated sewage or lava channels, ledges and steps (an
// inked lip, a hatched cliff face, a shadow below), terrain (water with slow ripples, chasms, rubble slopes), and the
// 5-ft grid.
//
// uField: 4 texels per square, linear: r floor (blurred mask), g liquid (blurred), b raised
//         floor (blurred, raise / 20 ft), a the way sewage runs (`flowAngle` in underField.ts). Built sites (uSquare): nearest, the floor unblurred;
//         their walls are measured from the squares instead, keeping square corners.
// uCells: one texel per square, nearest: r material, g shape (0 none, a shaped room's slot + 1
//         in uShapes, 255 a cavern breaking in), b variation (a sewer's brick: foam), a floor.
// uShapes: shaped rooms (round, octagon, apses, chamfered: `roomShapes.ts`), one column each:
//         row 0 the box round its squares (x0, y0, x1, y1), row 1 shape, material, variation.
//         Their squares are left out of uField and the squares' walls; the floor is drawn from
//         the shape near them (it may stand up to half a square off the squares).
// uCave: caverns breaking into a built level, linear: their floor (and the squares opening into
//         them) blurred as a cave's, drawn with rough rock walls.
// uTerrain: terrain (`kit::TERRAIN`), linear, 4 texels per square: r water (shallow, and under
//         deep water), g deep water, b chasm, a rubble slope; each blurred, spread into the rock
//         beside it (so it meets the walls). Read only where uHasTerrain.
// uLift: ledges and steps (`liftField` in underField.ts), linear, 8 texels a square in built sites
//         (else 4, blurred as the walls): r the signed distance to the nearest edge between floor
//         heights (squares, + on its high side, 128 + 63.5 d), g its rise (/ 20 ft), b how strongly it
//         is drawn (0 on ramps, tapering up within 0.4 sq of one), a on and by ramps 1 + 254 t (t how
//         far up the ramp, 0 at its low floor; its rise in g there; 0 elsewhere). Read only where uHasLift.
// uDist: squares to the nearest floor (`rockDistance.ts`), linear, 4 texels per square, DIST_PAD
//         squares past the grid each way, DIST_MAX at 1: the rock is hatched beside the walls by
//         it and fades out beyond (one fetch; the walls themselves come from the fields above).

in vec2 vUV;
in vec2 vLocal;

out vec4 finalColor;

uniform highp sampler2D uNoise;
uniform highp sampler2D uField;
uniform highp sampler2D uCells;
uniform highp sampler2D uShapes;
uniform highp sampler2D uCave;
uniform highp sampler2D uTerrain;
uniform highp sampler2D uLift;
uniform highp sampler2D uDist;

uniform vec2 uN;
uniform vec2 uLight;
uniform float uTime;
uniform float uRough;
uniform float uSquare;
uniform float uLiquid;
uniform float uGhost;
uniform float uGridAlpha;
uniform float uGrid;
// Where this grid sits among its neighbours (squares; a city's sewer sections, so their
// textures run on across the edges), else 0.
uniform vec2 uOrigin;
// Squares of uField beyond the grid each way (a sewer section's, read past its edge so that its
// walls and channels run on), else 0.
uniform float uPad;
// Shaped rooms on the level (0: none), caverns breaking in (0 or 1).
uniform float uShapeCount;
uniform float uCaverns;
uniform float uHasTerrain;
uniform float uHasLift;
// Squares the quad is drawn past the grid each way (a site's, so its rock fades out round it;
// sewer sections meet edge to edge: 0).
uniform float uGrow;
uniform vec4 uColor;
// The site's stone (`underTheme.ts`): its wall style (0 plain, 1 dwarven rune-cut blocks with
// thick walls, 2 rough ashlar, 3 dark basalt, 4 pale dusty stone, 5 wet brick, 6 marble, 7 packed
// earth, 8 natural rock: one branch for the whole site), flagstones, the coping on built walls
// and the hatched rock's tone.
uniform float uTheme;
uniform vec3 uStone;
uniform vec3 uCoping;
uniform vec3 uRock;
// A cave's look (`underTheme.ts`): 0 none, 1 green, 2 dry, 3 cold, 4 ice; and its cast on the floors.
uniform float uLook;
uniform vec3 uTint;

const vec3 INK = vec3(0.10, 0.085, 0.07);

// Squares a pixel (set first thing in `main`, for the functions below).
float gPx;
// How much of this pixel is a ramp (`ledges`): the rubble slope's dust and stones thin out over it.
float gRamp = 0.0;
// A built site's raised floors: the dressed stone their faces are cut from, in one flat tone (the
// floor's own flags would carry their joints and light and dark stones down the face).
vec3 gFace = vec3(0.5);
// How much of this pixel is a built ledge's face or its shadow (`ledgeEdge`): there the wall's shade
// is lifted as on the gallery's top (else a gallery by the wall has its face in the wall's cast
// shadow, near black, and the drop reads as a trough).
float gLedge = 0.0;
// How far `d` (squares to the wall) is a true distance here, 0-1: the floor field's slope is too flat for it
// out in a wide floor's middle.
float gTrue = 1.0;
// How much of this pixel `terrain` covers (water, a chasm, a slope): 0 where it has none.
float gTerr = 0.0;

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
    vec2 u = (q + uPad) / (uN + 2.0 * uPad);
    // (Past the texture, round a site's grid: rock.)
    if (any(lessThan(u, vec2(0.0))) || any(greaterThan(u, vec2(1.0)))) return vec4(0.0, 0.0, 0.0, 0.5);
    return texture(uField, u);
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

const float DIST_PAD = 4.0;
const float DIST_MAX = 8.0;

// The rock beside the walls, inked as on a hand-drawn dungeon map: short parallel strokes in
// small clusters, each cluster at its own angle, densest at the wall and thinning out, on a
// stone tone. `depth`: squares into the rock; `px`: squares a pixel. Strokes under ~2 px apart
// give way to their average tone (no shimmer). Clusters are the cells of a jittered grid
// (nearest centre), from hashes alone: one noise lookup, for the tone.
vec3 hatchedRock(vec2 p, float depth, float px, out float cut, out float ra) {
    cut = 0.0;
    // (Ice rock flat: one pale tone, no mottling.)
    vec3 c = uLook > 3.5 ? uRock : uRock * (0.92 + 0.16 * vnoise(p * 0.55));
    // (Ice rock is inked in a deep blue-grey, not black.)
    vec3 inkc = uLook > 3.5 ? vec3(0.36, 0.48, 0.6) : mix(INK, uRock, 0.18);
    // (The flat tone only close to the wall, ragged per cluster, so the rock is never paler than the ground.)
    float toneA = 0.55;
    float inkA = 0.0;
    float out1 = smoothstep(0.3, 2.0, depth);
    // Strokes this far apart (squares), half this thick (squares: about a pixel, bolder close up).
    const float GAP = 0.20;
    float hw = min(max(mix(0.03, 0.022, out1), 0.55 * px), 0.3 * GAP);
    float sharp = smoothstep(1.8, 3.0, GAP / max(px, 1e-5));
    float dens = mix(0.95, 0.15, out1);
    if (sharp > 0.0) {
        vec2 hc = p * 1.1;
        vec2 base = floor(hc);
        float best = 9.0;
        float second = 9.0;
        vec2 id = base;
        vec2 rel = vec2(0.0);
        for (int j = -1; j <= 1; j++) {
            for (int i = -1; i <= 1; i++) {
                vec2 cc = base + vec2(float(i), float(j));
                vec2 dv = hc - cc - 0.15 - 0.7 * vec2(hash21(cc), hash21(cc + 17.3));
                float dd = dot(dv, dv);
                if (dd < best) {
                    second = best;
                    best = dd;
                    id = cc;
                    rel = dv;
                } else if (dd < second) {
                    second = dd;
                }
            }
        }
        // (A thin gap between clusters; whole clusters drop out farther from the wall.)
        float edge = (sqrt(second) - sqrt(best)) / 1.1;
        // (Each cluster in or out as a whole; the band ends in ink, not in a haze.)
        float keep = step(hash21(id + 9.1), dens) * step(hash21(id + 11.9), 1.0 - smoothstep(1.2, 3.0, depth));
        // (Straight, parallel strokes: one angle for the whole cluster. The cluster's own
        // shift of where the tone fades out breaks up the outer edge.)
        float a = 3.14159265 * hash21(id + 3.7);
        cut = (hash21(id + 4.9) - 0.5) * 0.8;
        toneA = 0.55 * (1.0 - smoothstep(0.8, 1.6, depth + cut));
        vec2 dir = vec2(cos(a), sin(a));
        vec2 r = rel / 1.1;
        float t = dot(r, vec2(-dir.y, dir.x)) / GAP + hash21(id + 5.3);
        float row = floor(t);
        float h = hash21(vec2(row, 1.3) + id * 7.1);
        // Each stroke a little shorter or longer, a little off the cluster's middle.
        float u = dot(r, dir) - (h - 0.5) * 0.16;
        float half1 = (0.30 + 0.12 * fract(h * 13.7)) * mix(1.0, 0.6, out1);
        float aa = 0.8 * px;
        // (Width varies a quarter either way per stroke; ends round off.)
        float hws = hw * (0.75 + 0.5 * fract(h * 7.3));
        float dy = abs(fract(t) - 0.5) * GAP;
        float end = abs(u) - (half1 - hws);
        float shape = length(vec2(dy, max(end, 0.0)));
        float ink = 1.0 - smoothstep(hws - aa, hws + aa, shape);
        // (Clear of ink just by the wall's lip.)
        ink *= smoothstep(0.1, 0.15 + aa, depth);
        ink *= smoothstep(0.09, 0.12 + aa, edge) * keep;
        inkA = ink * 0.88 * sharp;
    } else {
        toneA = 0.55 * (1.0 - smoothstep(0.8, 1.6, depth));
    }
    // (Too fine to draw: their tone.)
    inkA = max(inkA, (1.0 - sharp) * 2.0 * hw / GAP * dens * 0.45);
    // (Ice: the pale tone solid over the whole rock, so the dark ground dimmed below never shows through
    // between the rooms as smoky dark stone.)
    if (uLook > 3.5) toneA = 0.88;
    ra = mix(toneA, max(toneA, 0.7), inkA);
    return mix(c, inkc, inkA);
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
    // (Off the grid, round a site: rock.)
    bool on = all(greaterThanEqual(q, vec2(0.0))) && all(lessThan(q, uN));
    bool fl = on && own.a > 0.5 && fromSquares(own.g);
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
    // (The whole ellipse, cut only by the box's far side: cut along its middle as well, the seam
    // where it meets the rectangle read as a wall, and drew a band of wall shade across the room.)
    float end = min(ellipseIn(p - vec2(lo.x + sz.x * 0.5, lo.y + r), vec2(sz.x * 0.5, r)), hi.y - p.y);
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

float segment(vec2 p, vec2 a, vec2 b) {
    vec2 pa = p - a;
    vec2 ba = b - a;
    return length(pa - ba * clamp(dot(pa, ba) / dot(ba, ba), 0.0, 1.0));
}

// A rune cut into a dwarven block (0-1: how much groove): a stave and one or two twigs off it,
// which and where by `h`. `g`: from the block's middle (squares). Gone below ~2 px a stroke.
float rune(vec2 g, float h) {
    float sx = sign(fract(h * 5.3) - 0.5);
    float y1 = 0.25 - 0.3 * fract(h * 3.1);
    float y2 = -0.05 - 0.2 * fract(h * 9.7);
    float r = segment(g, vec2(0.0, -0.3), vec2(0.0, 0.3));
    r = min(r, segment(g, vec2(0.0, y1), vec2(0.2 * sx, y1 + (fract(h * 2.3) > 0.5 ? 0.2 : -0.2))));
    if (fract(h * 7.7) > 0.45) r = min(r, segment(g, vec2(0.0, y2), vec2(-0.2 * sx, y2 - 0.18)));
    float w = 0.03;
    float aa = max(gPx * 0.8, 0.004);
    return (1.0 - smoothstep(w - aa, w + aa, r)) * (1.0 - smoothstep(0.02, 0.035, gPx));
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
        // Sewer walkway by the city (`under::sewer_stone`, in a section's low bits of `var`):
        // running-bond brick, dressed flags or rough rubble, warm (buff, sandstone) or cool;
        // each piece one of three flat tones, mid-tone joints, the NW edge of flags and rubble
        // lit. Pieces are numbered from the world (one across a section's edge is one piece).
        float st = uPad > 0.5 ? mod(var + 0.5, 8.0) - 0.5 : 0.0;
        float warm = step(3.5, st);
        float kind = st - 4.0 * warm;
        vec2 sc = kind < 0.5 ? vec2(2.0, 4.0) : kind < 1.5 ? vec2(1.0, 1.5) : vec2(2.4, 2.8);
        vec2 b = q * sc;
        float row = floor(b.y);
        float shift = kind < 1.5 ? 0.5 * mod(row, 2.0) : hash21(vec2(row + mod(uOrigin.y * sc.y, 1024.0), 4.1));
        b.x += shift;
        vec2 id = mod(floor(b) + mod(uOrigin * sc, 1024.0), 1024.0);
        vec2 g = fract(b);
        float tone = floor(hash21(id) * 2.999) * 0.5;
        // (Square units from the piece's edge, and the way out to it.)
        vec2 eg = min(g, 1.0 - g) / sc;
        vec2 out2 = eg.x < eg.y ? vec2(sign(g.x - 0.5), 0.0) : vec2(0.0, sign(g.y - 0.5));
        float edge = min(eg.x, eg.y);
        float jw = 0.035;
        if (kind > 1.5) {
            // Rubble: rounded stones of a few sizes, jittered in their cells.
            vec2 v = (g - 0.5 - 0.12 * (vec2(hash21(id + 2.2), hash21(id + 5.9)) - 0.5)) / sc;
            vec2 hs = (0.5 - 0.06 - 0.05 * hash21(id + 8.1)) / sc;
            float rr = 0.12;
            vec2 a = abs(v) - hs + rr;
            edge = rr - length(max(a, 0.0)) - min(max(a.x, a.y), 0.0);
            out2 = normalize(v + 1e-4);
            jw = 0.03;
        }
        vec3 base = kind < 0.5 ? mix(vec3(0.52, 0.41, 0.34), vec3(0.62, 0.52, 0.38), warm)
                  : kind < 1.5 ? mix(vec3(0.56, 0.55, 0.52), vec3(0.63, 0.56, 0.44), warm)
                  : mix(vec3(0.54, 0.52, 0.48), vec3(0.62, 0.55, 0.44), warm);
        c = base * (kind < 0.5 ? 0.9 + 0.14 * tone : 0.87 + 0.2 * tone);
        // Patches (one block of 3 x 3 squares in a few): repaired in another tone, following the
        // pieces; cracks across others.
        vec2 pc = (floor(b) + 0.5 - vec2(shift, 0.0)) / sc;
        vec2 pw = floor((q + uOrigin) / 3.0);
        float ph = hash21(mod(pw, 1024.0) + 11.3);
        vec2 pm = pw * 3.0 - uOrigin + 1.5 + (vec2(hash21(pw + 3.3), hash21(pw + 6.1)) - 0.5) * 0.8;
        vec2 pd = abs(pc - pm) - vec2(0.5, 0.45) - 0.5 * vec2(hash21(pw + 9.4), hash21(pw + 1.8));
        if (ph > 0.78 && max(pd.x, pd.y) < 0.0) {
            c = mix(c, (kind < 0.5 ? vec3(0.63, 0.56, 0.47) : vec3(0.49, 0.46, 0.42)) * (0.92 + 0.12 * tone), 0.85);
        }
        if (edge > 0.0) {
            // The edge toward the light lit, the far edge shaded (flags and rubble).
            float bev = (1.0 - smoothstep(0.02, 0.08, edge)) * step(0.5, kind);
            c *= 1.0 - 0.1 * bev * dot(out2, uLight) / max(length(uLight), 1e-4);
        }
        if (ph > 0.62 && ph <= 0.78) {
            float ca = 6.2832 * hash21(pw + 4.7);
            vec2 ct = vec2(cos(ca), sin(ca));
            vec2 cv = q - pm;
            float along = dot(cv, ct);
            float cd = dot(cv, vec2(-ct.y, ct.x)) + 0.12 * sin(along * 4.0 + ca) + 0.06 * sin(along * 9.0);
            float cw = 0.018 + fwidth(cd);
            c = mix(c, c * 0.68, (1.0 - smoothstep(cw * 0.5, cw, abs(cd))) * (1.0 - smoothstep(0.8, 1.3, abs(along))));
        }
        float joint = 1.0 - smoothstep(jw * 0.5, jw, edge);
        c = mix(c, mix(base * 0.6, vec3(0.34, 0.32, 0.28), 0.5), joint * (kind < 0.5 ? 0.85 : 0.6));
        c = mix(c, c * vec3(0.72, 0.77, 0.62), smoothstep(0.5, 0.8, fbm(p * 0.5 + 2.0)) * 0.4);
    } else if (mat == 7) {
        // Marble: a square slab per square, light and dark in turn, veined.
        vec2 cell = floor(q);
        vec2 g = fract(q);
        c = mix(vec3(0.80, 0.78, 0.74), vec3(0.60, 0.58, 0.56), mod(cell.x + cell.y, 2.0)) * (0.94 + 0.08 * hash21(cell));
        c = mix(c, c * 0.72, seam(fbm(p * 0.9 + cell * 0.37), 0.015));
        float joint = 1.0 - smoothstep(0.0, 0.02, min(min(g.x, 1.0 - g.x), min(g.y, 1.0 - g.y)));
        c = mix(c, INK * 1.8, joint * 0.3);
    } else if (mat == 8) {
        // Hewn rock: big blocks two squares long in running bond, tight joints. A dwarven hall's
        // two squares each way, a rune cut in some.
        bool dw = uTheme > 0.5 && uTheme < 1.5;
        vec2 b = q * (dw ? vec2(0.5) : vec2(0.5, 1.0));
        b.x += 0.5 * mod(floor(b.y), 2.0);
        vec2 g = fract(b);
        c = uStone * 0.9 * (0.85 + 0.2 * hash21(floor(b))) * (0.92 + 0.12 * vnoise(p * 2.0));
        float joint = 1.0 - smoothstep(0.0, dw ? 0.06 : 0.025, min(min(g.x, 1.0 - g.x) * 2.0, min(g.y, 1.0 - g.y) * (dw ? 2.0 : 1.0)));
        c = mix(c, INK * 1.5, joint * (dw ? 0.75 : 0.4));
        if (dw && hash21(floor(b) + 2.1) > 0.6) c = mix(c, c * 0.6, rune((g - 0.5) * 2.0 * vec2(1.0, 0.8), hash21(floor(b) + 6.3)));
    } else if (mat == 9) {
        // Ice: pale blue, cracked, glinting.
        c = vec3(0.74, 0.84, 0.90) * (0.88 + 0.16 * fbm(p * 0.5));
        c = mix(c, vec3(0.50, 0.64, 0.78), seam(vnoise(p * 1.3 + 2.0), 0.02));
        c += vec3(0.15) * smoothstep(0.94, 0.99, vnoise(p * 8.0));
    } else if (mat == 12) {
        // A fighting pit's packed sand (a boss chamber's sunken arena): pale earthy tan, paler and
        // yellower than the stone round it, in a few big soft shapes: raked paler here, trodden a
        // little darker and browner there (three close flat tones, no grain).
        c = vec3(0.63, 0.57, 0.46);
        c = mix(c, vec3(0.56, 0.49, 0.39), smoothstep(0.5, 0.66, vnoise(p * 0.32 + 5.0)) * 0.9);
        c = mix(c, vec3(0.69, 0.63, 0.52), smoothstep(0.6, 0.72, vnoise(p * 0.45 + 17.0)) * 0.7);
    } else {
        // Flagstones laid to the squares: one stone a square, or a square split in two along its
        // half line (which way and whether, by square). Boss chambers darker, crypts dusty.
        // (`scale` cancels out, but keep it: without it Adreno's compiler segfaults on this shader,
        // which kills every WebGL context on the page. Simpler ways of tying this to `mat` don't help.)
        float scale = mat == 5 ? 0.7 : mat == 10 ? 0.85 : 1.0;
        vec2 fq = q * scale / scale;
        vec2 sq = floor(fq);
        vec2 g = fract(fq);
        float h = hash21(sq + 3.7);
        vec2 stoneId = sq;
        vec2 e = vec2(0.5);
        if (h > 0.4) {
            // Split across x (two stones side by side) or across y (one above the other).
            bool alongX = h < 0.7;
            float side = step(0.5, alongX ? g.x : g.y);
            stoneId = sq + (alongX ? vec2(side * 0.5 + 0.25, 0.0) : vec2(0.0, side * 0.5 + 0.25));
            e = alongX ? vec2(abs(g.x - 0.5), 1.0) : vec2(1.0, abs(g.y - 0.5));
        }
        float edge = min(min(g.x, 1.0 - g.x), min(g.y, 1.0 - g.y));
        if (h > 0.4) edge = min(edge, min(e.x, e.y));
        edge += 0.025 * (vnoise(p * 6.0) - 0.5);
        float mortar = 1.0 - smoothstep(0.012, 0.04, edge);
        vec3 stone = mat == 5 ? uStone * vec3(0.76, 0.71, 0.74) : mat == 10 ? vec3(0.30, 0.28, 0.30) : uStone;
        c = stone * (0.8 + 0.32 * hash21(stoneId * 1.7 + var)) * (0.9 + 0.15 * vnoise(p * 3.0));
        c = mix(c, INK * 1.6, mortar * 0.35);
    }
    return c;
}

// A cave's biome on its floor: a few big flat patches (moss, drifted sand, frost); the green look's roots
// are `caveRoots`. `d`: squares from the wall, `n`: the wall's normal.
vec3 cave_look(vec3 col, vec2 p, float d, vec2 n) {
    if (uLook < 1.5) {
        col = mix(col, vec3(0.40, 0.50, 0.30), smoothstep(0.58, 0.62, vnoise(p * 0.2 + 31.0)) * 0.7);
    } else if (uLook < 2.5) {
        float drift = vnoise(p * 0.17 + 7.0) + 0.3 * (1.0 - smoothstep(0.0, 1.6, d));
        col = mix(col, vec3(0.82, 0.72, 0.52), smoothstep(0.66, 0.7, drift) * 0.75);
    } else {
        float frost = vnoise(p * 0.19 + 13.0) + 0.3 * (1.0 - smoothstep(0.0, 1.4, d));
        col = mix(col, vec3(0.8, 0.87, 0.92), smoothstep(0.64, 0.68, frost) * 0.7);
    }
    return col;
}

// The green look's roots: a few bold ones a room, reaching out of the wall over the floor's edge, each a
// flat brown shape in the sprites' ink outline, broad at the wall and tapering to a point as it bends,
// most with one side root. One a cell (5 squares) at most, rooted where the wall passes near the cell's
// point (so each is whole or not there at all); the cells round are looked at too, so none is cut off.
vec3 caveRoots(vec3 col, vec2 p, float d, vec2 n, float px) {
    // (Only by a wall: away from one the field is flat, `n` none and `d` no longer a distance.)
    if (d < 0.0 || d > 2.0 || gTrue < 0.01 || dot(n, n) < 0.5) return col;
    vec2 t = vec2(-n.y, n.x);
    float aa = max(px * 0.7, 1e-4);
    // (The sprites' outline weight, `OUT_W` below.)
    float lw = max(0.045, 1.2 * px);
    float dist = 9.0;
    vec2 base = floor(p / 5.0);
    for (int j = -1; j <= 1; j++) {
        for (int i = -1; i <= 1; i++) {
            vec2 cc = base + vec2(float(i), float(j));
            float h = hash21(cc + 61.0);
            if (h < 0.42) continue;
            vec2 c = (cc + 0.2 + 0.6 * vec2(hash21(cc + 7.0), hash21(cc + 13.0))) * 5.0;
            vec2 r = p - c;
            // (Where the wall's foot lies from the cell's point, along the normal: roots only on walls near it.)
            if (abs(dot(r, n) - d) > 1.6) continue;
            float len = 1.15 + 0.6 * hash21(cc + 19.0);
            float bend = (hash21(cc + 23.0) - 0.5) * 0.9;
            float k = d / len;
            float s = dot(r, t) - bend * k * k - 0.08 * sin(d * 2.6 + h * 30.0);
            float hw = mix(0.2, 0.024, clamp(k, 0.0, 1.0));
            float dr = d > len ? length(vec2(s, d - len)) - 0.02 : abs(s) - hw;
            // (A side root forking off halfway, to one side, thinner and shorter.)
            float fork = 0.35 + 0.2 * hash21(cc + 29.0);
            float side = hash21(cc + 31.0) < 0.5 ? -1.0 : 1.0;
            float d1 = d - fork * len;
            if (h > 0.55 && d1 > 0.0) {
                float l1 = 0.5 * len;
                float k1 = d1 / l1;
                float s1 = s - side * 0.7 * d1;
                float w1 = mix(0.1, 0.02, clamp(k1, 0.0, 1.0));
                dr = min(dr, d1 > l1 ? length(vec2(s1, d1 - l1)) - 0.018 : abs(s1) - w1);
            }
            dist = min(dist, dr);
        }
    }
    float cover = (1.0 - smoothstep(-aa, aa, dist)) * gTrue;
    float inner = (1.0 - smoothstep(-lw - aa, -lw + aa, dist)) * gTrue;
    col = mix(col, INK, cover * 0.9);
    return mix(col, vec3(0.47, 0.35, 0.23), inner);
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
    gTerr = max(max(t.r, t.g), max(t.b, t.a));
    if (gTerr < 0.02) return col;
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
        // (Over a ramp, thinner: its slope drawn under shows through.)
        float thin = 1.0 - 0.6 * gRamp;
        col = mix(col, vec3(0.56, 0.53, 0.48) * (0.85 + 0.25 * vnoise(p * 5.0)), cover * 0.3 * (1.0 - 0.75 * gRamp));
        col = screeStone(col, p + 0.37, 2.6, 9.0, smoothstep(-0.45, 0.1, dr) * 0.3 * thin, px);
        col = screeStone(col, p, 1.5, 0.0, cover * 0.4 * thin, px);
    }
    return col;
}

// The sprites' outline weights (squares): a shape's outline, a part's.
const float OUT_W = 0.045;
const float LINE_W = 0.025;

// `c` stepped in value by `dv`, in the square root of its luminance, so that one step reads alike
// on pale flags and dark basalt: ledges take their tops (lighter), faces (mid) and shadows (darker)
// from the theme's own floor tone.
vec3 valueStep(vec3 c, float dv) {
    float y = max(dot(c, vec3(0.3, 0.59, 0.11)), 1e-3);
    float v0 = sqrt(y);
    float v = clamp(v0 + dv, min(v0, 0.03), max(v0, 0.97));
    return min(c * (v * v / y), vec3(1.0));
}

// How dark a floor tone is (1 dark basalt, 0 brown cave rock and paler): on dark floors a ledge's
// top steps up further and its face and shadow further down, so it reads as boldly as in the caves.
float darkness(vec3 c) {
    return 1.0 - smoothstep(0.5, 0.64, sqrt(max(dot(c, vec3(0.3, 0.59, 0.11)), 1e-3)));
}

// A cliff face `down` squares below its lip (`n`: toward the high side), `faceW` wide, stepped down
// from `base` (the floor below): a mid tone by how it meets the light, in a few big slanting facets,
// darker toward its foot; broken rock strokes down the fall line, tapered at both ends, of varied
// length and spacing, many starting below the lip (so never even rungs). Along the edge by the
// foot's place (a built edge's x + y; natural rock's the diagonal running most along it).
vec3 cliffFace(vec3 base, vec2 q, vec2 n, float down, float faceW, float px, float aa) {
    float toward = dot(n, normalize(uLight));
    // (The high side toward the light: the face looks away from it, darkest.)
    // (On a dark floor a step further down, but never so near the ink that its strokes are lost.)
    float lit = (toward > 0.35 ? -0.2 : toward > -0.35 ? -0.15 : -0.1) - 0.06 * darkness(base);
    float floorV = 0.36 - sqrt(max(dot(base, vec3(0.3, 0.59, 0.11)), 1e-3));
    vec2 foot = q + n * down;
    vec2 t = vec2(-n.y, n.x);
    float a1 = abs(t.x + t.y);
    float a2 = abs(t.x - t.y);
    bool first = uSquare > 0.5 || a1 >= a2;
    float along = first ? foot.x + foot.y : foot.x - foot.y;
    float rate = uSquare > 0.5 ? 1.0 : max(max(a1, a2), 0.5);
    float k = clamp(down / max(faceW, 1e-3), 0.0, 1.0);
    // Facets: planes of uneven width, slanting down the face.
    float fu = along / rate / 0.55 + 0.6 * down;
    float fid = floor(fu + 0.45 * hash21(vec2(floor(fu * 0.5), 5.1)));
    vec3 fc = valueStep(base, max(lit + 0.05 * (hash21(vec2(fid, 2.9)) - 0.5) - 0.08 * k * k, min(floorV, -0.1)));
    // Strokes: one in most cells along the edge, at a jittered place, slanting a little.
    const float GAP = 0.16;
    float u = along / GAP;
    float id = floor(u);
    float h1 = hash21(vec2(id, 7.7));
    float h2 = hash21(vec2(id, 1.3));
    float h3 = hash21(vec2(id, 3.1));
    float h4 = hash21(vec2(id, 9.4));
    float start = faceW * (h4 < 0.45 ? 0.0 : 0.45 * (h4 - 0.45));
    float len = faceW * (0.3 + 0.65 * h1);
    float sAt = (down - start) / max(len, 1e-3);
    float off = abs((fract(u) - 0.5 - 0.7 * (h2 - 0.5)) * GAP / rate - (h3 - 0.5) * 0.4 * (down - start));
    float hw = LINE_W * 0.6 * clamp(1.0 - sAt, 0.0, 1.0) * clamp(sAt * 5.0, 0.0, 1.0);
    float stroke = (1.0 - smoothstep(hw - aa * 0.5, hw + aa * 0.5, off)) * smoothstep(0.15 * px, 0.6 * px, hw) * step(0.3, fract(h3 * 7.13));
    // (Gone where strokes would crowd within a few pixels, or the face is only a few pixels wide; and
    // on a step's riser, under 0.15 square: plain, only darker at its foot.)
    stroke *= smoothstep(2.5, 4.0, GAP / max(px, 1e-5)) * smoothstep(5.0, 9.0, faceW / max(px, 1e-5)) * smoothstep(0.13, 0.17, faceW);
    return mix(fc, INK, stroke * 0.7);
}

// A built site's ledge face (a gallery's, a stage's, a sunken arena's wall): a dressed stone wall
// seen from above, in one flat tone (`gFace`) about the floor's value, lighter where it faces the
// light and a little darker where it looks away (never near the ink), so the lighter top, the lip,
// the face and the soft shadow past it read as a walkway's edge dropping to the floor below, not a
// trough. Its blocks in courses (two on a tall face) in running bond, the joints a darker stone
// tone, not ink. (Risers under 0.15 square, a dais's or a step's: plain, the lip their only line.)
vec3 builtFace(vec2 q, vec2 n, float down, float faceW, float px, float aa) {
    float toward = dot(n, normalize(uLight));
    float k = clamp(down / max(faceW, 1e-3), 0.0, 1.0);
    vec3 fc = valueStep(gFace, mix(0.06, -0.03, smoothstep(-0.6, 0.6, toward)) + 0.02 * (1.0 - k));
    float big = smoothstep(0.13, 0.17, faceW) * smoothstep(6.0, 9.0, faceW / max(px, 1e-5));
    float courses = faceW > 0.3 ? 2.0 : 1.0;
    float ck = k * courses;
    vec2 edge = q + n * down;
    float u = (edge.x + edge.y) / 0.5 + 0.5 * floor(ck);
    float hw = max(0.009, 0.5 * px);
    float dj = abs(fract(u + 0.5) - 0.5) * 0.5;
    float joint = (1.0 - smoothstep(hw - aa * 0.5, hw + aa * 0.5, dj)) * smoothstep(5.0, 8.0, 0.5 / max(px, 1e-5));
    float dc = abs(fract(ck + 0.5) - 0.5) * faceW / courses;
    float course = courses > 1.5 ? 1.0 - smoothstep(hw - aa * 0.5, hw + aa * 0.5, dc) : 0.0;
    return mix(fc, valueStep(fc, -0.16), max(joint, course) * 0.8 * big);
}

// How much lighter a raised floor's top is (a value step, `valueStep`), by its height (ft): 2.5 ft
// a touch, 5 ft clearly, 10 ft more, less a foot above that.
float topStep(float ft) {
    return ft > 0.05 ? 0.011 * min(ft, 10.0) + 0.005 * max(ft - 10.0, 0.0) : 0.0;
}

// The face below a lip (squares), for a drop (ft): 2.5 ft 0.1, 5 ft 0.25, 10 ft 0.5, 20 ft 0.8.
float faceWidth(float drop) {
    float w = 0.04 * min(drop, 2.5) + 0.06 * clamp(drop - 2.5, 0.0, 2.5) + 0.05 * clamp(drop - 5.0, 0.0, 5.0) + 0.03 * max(drop - 10.0, 0.0);
    return min(w, 0.8);
}

// Ledges and steps (uLift, `l` its texel here), along every edge between floor heights: the high
// side's lip inked at the sprites' outline weight; the cliff face below it (`cliffFace`), wider the
// higher the drop; the shadow it casts on the floor below, flat with a soft end, longer and darker
// for bigger drops, strongest away from the light. Toward a ramp (`l.b` falling to 0 at it) the
// cliff ends: the lip, at full weight, stops just short of it, the face and shadow narrow to nothing. Natural
// rock's edges wobble as its walls do; built sites' run straight along the squares. First the top
// lightened by its height (`hf`, ft; near a natural edge, where the field blurs it, the height a
// little way off on this pixel's own side), so the faces and shadows step down from the floor's tone.
vec3 ledgeEdge(vec3 col, vec4 l, vec2 q, vec2 p, float px, float hf) {
    float r0 = (l.r * 255.0 - 128.0) / 63.5;
    float dk = darkness(col);
    if (r0 > 0.3 || r0 < -1.9 || l.b < 0.01) return valueStep(col, topStep(hf) * (1.0 + 0.8 * dk) * (uSquare > 0.5 ? 1.35 : 1.0));
    // (Toward the high side, from the field's slope.)
    float e = 0.125;
    vec2 g = vec2(texture(uLift, (q + vec2(e, 0.0)) / uN).r, texture(uLift, (q + vec2(0.0, e)) / uN).r) - l.r;
    g *= 255.0 / 63.5 / e;
    float gl = length(g);
    vec2 n = gl > 1e-4 ? g / gl : vec2(0.0);
    float r = r0;
    if (uSquare < 0.5) {
        r0 /= max(gl, 0.35);
        r = r0 + uRough * ((fbm(p * 1.1 + 21.0) - 0.5) * 0.16 + (vnoise(p * 4.3 + 8.0) - 0.5) * 0.05);
    }
    if (uSquare < 0.5) hf = field(q + n * (r0 > 0.0 ? 0.7 : -0.7)).b * 20.0;
    // (Built sites' tops a step lighter again: their floors are darker and busier than cave rock.)
    float topK = (1.0 + 0.8 * dk) * (uSquare > 0.5 ? 1.35 : 1.0);
    col = valueStep(col, topStep(hf) * topK);
    float drop = l.g * 20.0;
    // (Only where the floor really drops: a crossing with no rise is where two floors of one height meet
    // far from any edge, e.g. inside a terrace between a lower and a higher floor; no lip there.)
    float s = l.b * smoothstep(0.1, 0.45, drop);
    float aa = max(px * 0.7, 1e-4);
    float down = -r;
    float faceW = faceWidth(drop) * s;
    // The shadow on the floor below (fac > 0: the high side lies toward the light).
    float fac = smoothstep(-0.3, 0.7, dot(n, normalize(uLight)));
    float shLen = min(0.95, 0.15 + 0.05 * drop) * mix(0.6, 1.0, fac) * s;
    float shV = min(0.34, 0.12 + 0.015 * drop) * mix(0.55, 1.0, fac) * (1.0 + 0.4 * dk);
    float sh = smoothstep(-aa, aa, down) * (1.0 - smoothstep(faceW + shLen * 0.55, faceW + shLen, down));
    // (Built: a short soft shadow on the floor below only, strongest at the face's foot and fading
    // out, slight where the face is lit; darker than the face but never near the ink.)
    if (uSquare > 0.5) {
        shLen = min(0.42, 0.1 + 0.032 * drop) * mix(0.5, 1.0, fac) * s;
        shV = min(0.13, 0.06 + 0.007 * drop) * mix(0.25, 1.0, fac) * (1.0 + 0.2 * dk);
        float st = clamp((down - faceW) / max(shLen, 1e-3), 0.0, 1.0);
        sh = smoothstep(faceW - aa, faceW + aa, down) * (1.0 - st) * (1.0 - st);
    }
    vec3 floorCol = col;
    col = mix(col, valueStep(col, -shV), sh * min(1.0, s * 2.0));
    float inFace = smoothstep(-aa * 0.5, aa * 0.5, down) * (1.0 - smoothstep(faceW - aa * 0.5, faceW + aa * 0.5, down));
    if (uSquare > 0.5) gLedge = max(gLedge, max(inFace, sh) * smoothstep(0.12, 0.3, s));
    if (inFace > 0.0) col = mix(col, uSquare > 0.5 ? builtFace(q, n, -r0, faceW, px, aa) : cliffFace(floorCol, q, n, -r0, faceW, px, aa), inFace);
    // The lip, at full weight up to just short of a ramp, where it ends (bolder on built sites'
    // straight edges, which have no rough wall line to carry them).
    float lw = max(OUT_W * (uSquare > 0.5 ? 1.3 : 1.0), (uSquare > 0.5 ? 1.5 : 1.2) * px);
    float lip = smoothstep(-aa * 0.5, aa * 0.5, r) * (1.0 - smoothstep(lw - aa * 0.5, lw + aa * 0.5, r));
    return mix(col, INK, lip * 0.95 * smoothstep(0.12, 0.3, s));
}

// A ramp (a rubble slope joining two floors; uLift's alpha `t` how far up it, 0 at the lower floor's
// edge, 1 at the higher's; its own texel's green the rise between them, its low 3 bits the way up):
// its floor graded from the low floor's tone at its foot to the high one's at its top (`hf`, the
// blurred height, half-grades it already) and shaded as it meets the light, with short strokes down
// the fall line from its top edge, broad there and tapering to a point (as the region map's
// hachures), so which way is up reads at a glance. (One fetch more, of its own texel, exact.)
vec3 rampSlope(vec3 col, vec4 l, vec2 q, float px, float hf) {
    float t = clamp((l.a * 255.0 - 1.0) / 254.0, 0.0, 1.0);
    ivec2 sz = textureSize(uLift, 0);
    vec4 e = texelFetch(uLift, clamp(ivec2(floor(q / uN * vec2(sz))), ivec2(0), sz - 1), 0);
    // (A ramp texel's strength is 0; just outside one, only its tone, from the filtered rise.)
    float own = step(e.b, 0.5 / 255.0);
    float code = floor(e.g * 255.0 + 0.5);
    float way = mod(code, 8.0);
    float rise = mix(l.g, (code - way) / 255.0, own) * 20.0;
    vec2 dir = vec2(cos(way * 0.7853982), sin(way * 0.7853982));
    // (Rising toward the light it faces away from it: a little darker.)
    float shade = -0.04 * dot(dir, normalize(uLight)) * own;
    vec3 c = valueStep(col, topStep(hf + (t - 0.5) * 0.5 * rise) * (1.0 + 0.8 * darkness(col)) + shade);
    const float GAP = 0.24;
    float u = dot(q, vec2(-dir.y, dir.x)) / GAP;
    float id = floor(u);
    float h1 = hash21(vec2(id, 4.3));
    float h2 = hash21(vec2(id, 8.1));
    float h3 = hash21(vec2(id, 2.2));
    float len = 0.4 + 0.35 * h1;
    float sAt = (1.0 - t) / len;
    float off = (fract(u) - 0.5 - 0.5 * (h2 - 0.5)) * GAP - (h3 - 0.5) * 0.2 * (1.0 - t);
    float hw = 0.5 * max(0.05, 1.6 * px) * clamp(1.0 - sAt, 0.0, 1.0);
    float aa = max(px * 0.7, 1e-4);
    float stroke = (1.0 - smoothstep(hw - aa * 0.5, hw + aa * 0.5, abs(off))) * step(0.2, fract(h3 * 5.31));
    stroke *= smoothstep(2.0, 0.8, sAt) * smoothstep(3.0, 5.0, GAP / max(px, 1e-5)) * step(0.5, rise) * own;
    return mix(c, INK, stroke * 0.8);
}

// The floor's height drawn (`ledgeEdge`), and over a ramp its slope (`rampSlope`).
vec3 ledges(vec3 col, vec4 l, vec2 q, vec2 p, float px, float hf) {
    vec3 c = ledgeEdge(col, l, q, p, px, hf);
    gRamp = step(0.5 / 255.0, l.a) * (1.0 - smoothstep(0.03, 0.2, l.b));
    if (gRamp > 0.0) c = mix(c, rampSlope(col, l, q, px, hf), gRamp);
    return c;
}

void main() {
    vec2 q = vUV * (uN + 2.0 * uGrow) - uGrow;
    vec2 p = q + uOrigin;
    // (Squares a pixel.)
    float px = length(fwidth(q));
    gPx = px;
    int style = int(uTheme + 0.5);
    // Squares to the nearest floor: past where the rock has faded out, nothing.
    float far = texture(uDist, (q + DIST_PAD) / (uN + 2.0 * DIST_PAD)).r * DIST_MAX;
    if (far > 4.2) {
        finalColor = vec4(0.0);
        return;
    }
    vec4 here = field(q);
    // Ledges and steps.
    vec4 lift = vec4(0.5, 0.0, 0.0, 0.0);
    if (uHasLift > 0.5) lift = texture(uLift, q / uN);
    float d;
    vec2 n;
    if (uSquare > 0.5) {
        boxField(q, d, n);
    } else {
        // Signed distance to the wall (squares, + on the floor) from the blurred floor field.
        float e = 0.25;
        vec2 grad = vec2(field(q + vec2(e, 0.0)).r - field(q - vec2(e, 0.0)).r, field(q + vec2(0.0, e)).r - field(q - vec2(0.0, e)).r) / (2.0 * e);
        d = (here.r - 0.5) / max(length(grad), 0.35);
        gTrue = smoothstep(0.12, 0.3, length(grad));
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
                gTrue = 1.0;
                n = length(gs) > 1e-6 ? normalize(gs) : vec2(0.0);
                won = slot;
                look = lk;
            }
        }
        if (cave && all(greaterThanEqual(q, vec2(0.0))) && all(lessThan(q, uN))) {
            float e = 0.25;
            float c0 = texture(uCave, q / uN).r;
            vec2 gc = vec2(texture(uCave, (q + vec2(e, 0.0)) / uN).r - texture(uCave, (q - vec2(e, 0.0)) / uN).r, texture(uCave, (q + vec2(0.0, e)) / uN).r - texture(uCave, (q - vec2(0.0, e)) / uN).r) / (2.0 * e);
            float dc = (c0 - 0.5) / max(length(gc), 0.35);
            dc += (fbm(p * 0.8 + 3.0) - 0.5) * 0.75 + (vnoise(p * 3.7) - 0.5) * 0.25 + (vnoise(p * 9.0) - 0.5) * 0.08;
            lit = max(lit, smoothstep(0.35, 0.6, texture(uCave, qs / uN).r));
            if (dc > d) {
                d = dc;
                gTrue = smoothstep(0.12, 0.3, length(gc));
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
    // (A dwarven hall's walls are thick: a wider coping, inked on both sides.)
    if (style == 1 && rough < 0.2) rim = 0.3;
    if (d < 0.0) {
        // Rock: the wall's top edge lit from the NW, then the rock hatched beside the walls,
        // fading out beyond (the map above shows round the site). (Near the wall its depth is
        // the wall's own; farther out, from uDist.)
        float depth = mix(-d, far, smoothstep(0.6, 1.4, far));
        float cut;
        float ra;
        vec3 r = hatchedRock(p, depth, px, cut, ra);
        float face = clamp(0.55 + 0.6 * dot(n, uLight), 0.25, 1.15);
        // (Natural rock's lip in the site's own rock: caves as they were.)
        vec3 lip = rockMass(p * 1.3, 0.0) * 1.55 * face * (uRock / vec3(0.29, 0.27, 0.245));
        // (Ice: a flat pale-blue edge, shaded by the light, in place of the dark rock's.)
        if (uLook > 3.5) lip = mix(uRock * 1.2, vec3(0.88, 0.94, 0.98), 0.55) * (0.8 + 0.3 * face);
        float edge = rim;
        if (rough < 0.2) {
            // Dressed stone coping on built walls, by the site's style.
            vec2 b;
            vec2 g;
            float jw = 0.06;
            float jk = 0.5;
            if (style == 2) {
                // Rough ashlar: uneven blocks, each course shifted its own way, wavering joints,
                // a ragged edge toward the rock.
                b = q * vec2(1.3, 1.7);
                b.x += hash21(vec2(floor(b.y), 2.7)) + 0.3 * (vnoise(p * 2.1) - 0.5);
                b.y += 0.15 * (vnoise(p * 2.3 + 7.0) - 0.5);
                g = fract(b);
                jw = 0.09;
                edge = rim + 0.1 * (vnoise(p * 2.7 + 1.0) - 0.5);
            } else if (style == 5) {
                // Brick: stretchers in running bond.
                b = q * vec2(2.2, 4.4);
                b.x += 0.5 * mod(floor(b.y), 2.0);
                g = fract(b);
                jw = 0.1;
                jk = 0.4;
            } else {
                float k = style == 1 ? 0.8 : style == 6 ? 1.0 : 1.6;
                b = q * k;
                g = fract(b + 0.5 * mod(floor(b.yx), 2.0));
                jk = style == 6 ? 0.22 : style == 4 ? 0.3 : style == 7 ? 0.0 : style == 1 ? 0.85 : 0.5;
                jw = style == 1 ? 0.14 : 0.06;
            }
            float joint = 1.0 - smoothstep(0.0, jw, min(min(g.x, 1.0 - g.x), min(g.y, 1.0 - g.y)));
            vec3 cop = uCoping * (0.85 + 0.25 * hash21(floor(b)));
            if (style == 2) {
                // (A few blocks chipped dark.)
                cop *= hash21(floor(b) + 5.5) < 0.2 ? 0.78 : 1.0;
            } else if (style == 4) {
                // (Dust over it.)
                cop = mix(cop, vec3(0.76, 0.73, 0.66), smoothstep(0.45, 0.8, vnoise(p * 1.7)) * 0.4);
            } else if (style == 5) {
                // (Damp, green toward the floor.)
                cop = mix(cop, cop * vec3(0.62, 0.84, 0.5), max(smoothstep(0.4, 0.75, vnoise(p * 0.9)) * 0.6, (1.0 - smoothstep(0.02, 0.2, -d)) * 0.9));
            } else if (style == 7) {
                // (Packed earth: no joints, lumpy.)
                cop = uCoping * (0.82 + 0.3 * vnoise(p * 1.6));
            }
            // (Lit, toned down: a pale coping would be the brightest thing in the room.)
            lip = mix(cop * face * (1.0 - 0.15 * smoothstep(0.7, 1.15, face)), INK, joint * jk);
        }
        col = mix(lip, r, smoothstep(-edge, -edge - 0.12, d));
        if (style == 1 && rough < 0.2) col = mix(col, INK, (1.0 - smoothstep(0.0, aa * 1.6, abs(d + rim))) * 0.75);
        // (Ice rock reaches a little farther before it fades, filling the pockets between rooms.)
        float fade = uLook > 3.5 ? 3.1 : 2.2;
        alpha = ra * (1.0 - smoothstep(fade, fade + 1.0, depth + cut));
        // (Dressed coping is solid stone, not see-through as the hatched rock beyond it is: else the
        // map above showed through a lit wall's top as a pale haze, and round an outer corner as a
        // smeared wedge.)
        if (rough < 0.2) alpha = mix(1.0, alpha, smoothstep(-edge, -edge - 0.12, d));
    } else {
        ivec2 s = clamp(ivec2(floor(q)), ivec2(0), ivec2(uN) - 1);
        vec4 cell = texelFetch(uCells, s, 0) * 255.0;
        if (won > 0 && won < 255) {
            // A shaped room's floor, wherever its shape reaches.
            // (A sunken arena's sand keeps its squares.)
            cell = vec4(cell.r > 11.5 && cell.a > 0.5 ? 12.0 : look.y, 0.0, look.z, 255.0);
        } else if (won == 255) {
            cell = vec4(0.0, 255.0, cell.b, 255.0);
        } else if (cell.a < 0.5) {
            // A smoothed edge reaching into a rock square: the floor next to it.
            cell = texelFetch(uCells, clamp(s + ivec2(round(n)), ivec2(0), ivec2(uN) - 1), 0) * 255.0;
            // (That floor may lie past the section's edge: sewers are all brick.)
            // (The city's stonework is on the rock too.)
            if (cell.a < 0.5 && uPad > 0.5) cell = vec4(3.0, 0.0, cell.b, 255.0);
        }
        int mat = int(cell.r + 0.5);
        col = floorColor(mat, q, p, cell.b);
        if (style == 4) {
            // Dust on a tomb's flags, drifted against the walls.
            float dust = smoothstep(0.5, 0.9, vnoise(p * 0.6 + 40.0) + 0.35 * (1.0 - smoothstep(0.0, 1.2, d)));
            col = mix(col, vec3(0.75, 0.71, 0.63), dust * 0.35 * step(3.5, float(mat)));
        } else if (style == 2) {
            // A fallen fortress's flags, cracked.
            col = mix(col, col * 0.7, seam(vnoise(p * 0.7 + 17.0), 0.012) * 0.8 * step(3.5, float(mat)));
        } else if (style == 3) {
            // The cult's floor, a dark red stain spreading here and there.
            col = mix(col, col * vec3(0.86, 0.7, 0.68), smoothstep(0.55, 0.8, vnoise(p * 0.4 + 23.0)) * 0.5 * step(3.5, float(mat)));
        }
        if (uLook > 0.5 && uLook < 3.5) col = cave_look(col * uTint, p, d, n);
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
        // Raised floors: tops lighter by their height; their edges' lips, faces and shadows.
        float hf = here.b * 20.0;
        if (uSquare > 0.5) {
            vec3 boss = uStone * vec3(0.76, 0.71, 0.74);
            gFace = mat == 7 ? vec3(0.70, 0.68, 0.65) : mat == 10 ? vec3(0.30, 0.28, 0.30) : mat == 5 || mat == 12 ? boss : mat == 9 ? col : uStone * 0.92;
        }
        if (uHasLift > 0.5) col = ledges(col, lift, q, p, px, hf);
        else col = valueStep(col, topStep(hf));
        // Water, chasms, rubble slopes.
        if (uHasTerrain > 0.5) col = terrain(col, q, p, px);
        // Against the walls: ambient occlusion and the shadow cast from the NW.
        // (A built gallery's top, up by the wall's head, keeps only a thin band of it and none of the
        // wall's cast shadow: else a strip a square wide is all shade and its lighter top is lost.)
        float up = uSquare > 0.5 ? max(smoothstep(2.0, 8.0, hf), gLedge) : 0.0;
        col *= mix(0.6 + 0.4 * smoothstep(0.0, 0.75, d), 0.9 + 0.1 * smoothstep(0.0, 0.2, d), up);
        col *= mix(mix(0.66, 1.0, smoothstep(0.35, 0.6, lit)), 1.0, up);
        // (Roots over the wall's shade, so they keep their tone; not over water or a chasm.)
        if (uLook > 0.5 && uLook < 1.5) col = mix(col, caveRoots(col, p, d, n, px), 1.0 - smoothstep(0.02, 0.3, gTerr));
        // Liquid down the channel.
        float l = here.g;
        if (uLiquid > 0.5) {
            float lw = max(fwidth(l), 1e-3);
            float inside = smoothstep(0.5 - lw, 0.5 + lw, l);
            vec3 liq;
            if (uLiquid < 1.5) {
                // Sewage, painted: three flat tones (darker down the middle), a few soft strokes
                // drifting downstream, floating scum and the odd bit of debris, foam where runs
                // meet or end; slime and damp brick along its edge and the wall foot, puddles.
                float wob = vnoise(p * 0.8 + 31.0);
                float lt = l + 0.03 * (wob - 0.5);
                float sw = max(lw, 0.006);
                liq = mix(vec3(0.32, 0.37, 0.20), vec3(0.26, 0.31, 0.16), smoothstep(0.565 - sw, 0.565 + sw, lt));
                liq = mix(liq, vec3(0.21, 0.26, 0.13), smoothstep(0.625 - sw, 0.625 + sw, lt));
                // A thin wet glint just inside each edge (matte, painted).
                liq = mix(liq, vec3(0.47, 0.52, 0.34), (1.0 - smoothstep(0.0, max(lw * 1.2, 0.005), abs(lt - 0.512))) * 0.7);
                // Along and across the run (`flowAngle`; the strokes fade where runs meet).
                float ph = (here.a - 0.5) * 3.14159265;
                vec2 fd = cos(ph) * vec2(0.958, 0.287) + sin(ph) * vec2(-0.287, 0.958);
                vec2 fp = vec2(dot(p, fd), dot(p, vec2(-fd.y, fd.x)));
                float calm = (1.0 - smoothstep(0.3, 0.7, fwidth(here.a) / max(px, 1e-4))) * smoothstep(0.54, 0.6, l);
                vec2 sc = vec2(fp.x * 0.45 - uTime * 0.13, fp.y * 4.0);
                sc.x += 5.0 * hash21(vec2(floor(sc.y), 3.1));
                vec2 sid = floor(sc);
                vec2 sf = fract(sc);
                float stroke = smoothstep(0.0, 0.3, sf.x) * (1.0 - smoothstep(0.5, 0.95, sf.x));
                stroke *= 1.0 - smoothstep(0.1, 0.24, abs(sf.y - 0.5 + 0.12 * sin(sf.x * 6.28 + sid.x)));
                stroke *= step(0.55, hash21(sid + 0.7)) * calm * (1.0 - smoothstep(0.06, 0.12, px));
                // (Foam in steps of 8, the city's stonework in the low bits: `sewerFoam`.)
                float stw = mod(cell.b + 0.5, 8.0) - 0.5;
                // (Blended between squares, so froth has no square edge: four fetches, in the channel only.)
                float fo = 0.0;
                if (uPad > 0.5 && mat == 3 && l > 0.48) {
                    vec2 fq = q - 0.5;
                    ivec2 f0 = ivec2(floor(fq));
                    vec2 fw4 = fract(fq);
                    ivec2 fm = ivec2(uN) - 1;
                    vec4 fs = vec4(texelFetch(uCells, clamp(f0, ivec2(0), fm), 0).b, texelFetch(uCells, clamp(f0 + ivec2(1, 0), ivec2(0), fm), 0).b,
                                   texelFetch(uCells, clamp(f0 + ivec2(0, 1), ivec2(0), fm), 0).b, texelFetch(uCells, clamp(f0 + ivec2(1, 1), ivec2(0), fm), 0).b) * 255.0;
                    fs = max(fs - mod(fs + 0.5, 8.0) + 0.5, 0.0) / 248.0;
                    fo = mix(mix(fs.x, fs.y, fw4.x), mix(fs.z, fs.w, fw4.x), fw4.y);
                }
                // (No strokes in the froth, where the runs' ways meet and they'd break up.)
                stroke *= 1.0 - smoothstep(0.1, 0.4, fo);
                liq = mix(liq, liq * 1.25 + vec3(0.03, 0.04, 0.02), stroke * 0.7);
                // Scum and debris, drifting more slowly (cells 2 x 0.6 squares).
                vec2 bc = vec2(fp.x * 0.5 - uTime * 0.06, fp.y * 1.6);
                vec2 bid = floor(bc);
                float bh = hash21(bid + 2.3);
                vec2 bv = (fract(bc) - vec2(0.3 + 0.4 * hash21(bid + 5.1), 0.5)) * vec2(2.0, 0.625);
                float deb = step(0.94, bh);
                // (Two soft lobes, smooth-joined; debris a single sliver.)
                float ba = 6.2832 * hash21(bid + 3.7);
                vec2 bo = vec2(cos(ba), sin(ba));
                float br = mix(0.12 + 0.04 * hash21(bid + 8.7), 0.09, deb);
                vec2 be = bv * vec2(1.0, 1.0 + deb);
                float bd1 = length(be - bo * 0.05 * (1.0 - deb)) - br;
                float bd2 = length(be + bo * 0.09) - br * 0.68 + deb;
                float bk = clamp(0.5 + 0.5 * (bd2 - bd1) / 0.06, 0.0, 1.0);
                float bd = mix(bd2, bd1, bk) - 0.06 * bk * (1.0 - bk);
                float bew = max(px, 0.015);
                float blob = (1.0 - smoothstep(-bew, bew, bd)) * step(0.72, bh) * calm;
                vec2 bg = bv.x * fd + bv.y * vec2(-fd.y, fd.x);
                vec3 scum = mix(vec3(0.44, 0.45, 0.31), vec3(0.55, 0.55, 0.40), smoothstep(-0.3, 0.6, -dot(bg, uLight) / br));
                scum = mix(scum, vec3(0.30, 0.22, 0.14), deb);
                liq = mix(liq, scum, blob * 0.8);
                // Foam where runs meet, end or fall out (sewer sections' brick only): soft cream
                // patches merging along the falling water, fading out softly.
                // (Where runs meet the way turns within a texel, and `fp`, far from the origin, swings
                // through many noise cells a pixel: there the noise fades to its mean, or it speckles.)
                vec2 fc = vec2(fp.x * 2.2 - uTime * 0.3, fp.y * 2.8);
                vec2 fcw = fwidth(fc);
                float fsharp = 1.0 - smoothstep(0.25, 0.6, max(fcw.x, fcw.y));
                if (fo > 0.0) {
                    // (Value noise from hashes, drifting downstream, roughened by `wob`; no fetches.)
                    vec2 fi = floor(fc);
                    vec2 ff = fract(fc);
                    ff = ff * ff * (3.0 - 2.0 * ff);
                    float fn = mix(mix(hash21(fi), hash21(fi + vec2(1.0, 0.0)), ff.x), mix(hash21(fi + vec2(0.0, 1.0)), hash21(fi + 1.0), ff.x), ff.y);
                    fn = 0.75 * mix(0.5, fn, fsharp) + 0.25 * wob;
                    float fv = 1.2 * (fn - 0.5) + 1.1 * fo - 0.55;
                    float fw = 0.06 + 0.5 * px;
                    float fpat = smoothstep(-fw, fw, fv) * smoothstep(0.52, 0.56, l) * smoothstep(0.0, 0.2, fo);
                    vec3 cream = liq * 1.05 + vec3(0.16, 0.14, 0.10);
                    // (Lit from the upper left: lighter on the side toward the light.)
                    cream += vec3(0.03) * clamp(-dot(fd, uLight) * fv * 4.0, -1.0, 1.0);
                    liq = mix(liq, cream, fpat * 0.85);
                }
                // Kerb stones along the edge (sewer sections): a lighter dressed course in stones
                // a square or so long, the lip toward the water shaded, chipped here and there.
                if (uPad > 0.5 && mat == 3) {
                    float kw = max(lw, 0.004);
                    float kerb = smoothstep(0.26 - kw, 0.26 + kw, l) * (1.0 - inside);
                    float ku = fp.x * 0.85;
                    float kid = floor(ku);
                    float kf = fract(ku);
                    float kh = hash21(vec2(kid, 7.7));
                    float kwarm = step(3.5, stw);
                    vec3 kc = mix(vec3(0.63, 0.61, 0.57), vec3(0.72, 0.65, 0.53), kwarm) * (0.92 + 0.08 * floor(kh * 2.999));
                    // The lip: the stone's face dropping to the water.
                    kc *= mix(1.0, 0.8, smoothstep(0.47, 0.49, l));
                    // Chips: a bite or two out of the lip.
                    float cx = kf - 0.25 - 0.5 * hash21(vec2(kid, 2.9));
                    float chip = step(0.55, kh) * (1.0 - smoothstep(0.0, 0.03, length(vec2(cx * 1.4, (0.5 - l) * 2.5)) - 0.14 - 0.03 * sin(kf * 40.0)));
                    kc = mix(kc, col * 0.78, chip);
                    float kcalm = 1.0 - smoothstep(0.6, 1.2, fwidth(here.a) / max(px, 1e-4));
                    float kj = (1.0 - smoothstep(0.0, 0.04 + 1.2 * fwidth(ku), min(kf, 1.0 - kf))) * kcalm * (1.0 - smoothstep(0.12, 0.25, fwidth(ku)));
                    kc = mix(kc, kc * 0.66, kj * 0.8);
                    // (Its inner edge: a mid-tone joint on the walkway.)
                    kc = mix(kc, col * 0.7, (1.0 - smoothstep(0.0, kw * 2.5, l - 0.26)) * 0.7);
                    col = mix(col, kc, kerb);
                }
                // On the walkway: damp stone toward the water, a soft slime band at its edge, slime
                // at the wall foot, a few puddles.
                col *= mix(1.0, 0.86, smoothstep(0.06, 0.4, l));
                col = mix(col, vec3(0.36, 0.40, 0.22), smoothstep(0.44 + 0.04 * wob, 0.49, l) * 0.4);
                col = mix(col, vec3(0.31, 0.35, 0.21), (1.0 - smoothstep(0.05, 0.16 + 0.14 * wob, d)) * 0.5);
                vec2 pc = p * 0.5;
                vec2 pid = floor(pc);
                vec2 pv = (fract(pc) - 0.3 - 0.4 * vec2(hash21(pid + 7.7), hash21(pid + 2.9))) * 2.0;
                float pr = (0.26 + 0.12 * hash21(pid + 3.3)) * (1.0 + 0.15 * sin(pv.x * 9.0 + pv.y * 7.0));
                float pud = (1.0 - smoothstep(pr - max(px, 0.015), pr + max(px, 0.015), length(pv * vec2(1.0, 1.4)))) * step(0.8, hash21(pid + 0.4));
                pud *= (1.0 - smoothstep(0.12, 0.3, l)) * smoothstep(0.18, 0.35, d);
                vec3 wetCol = col * vec3(0.76, 0.80, 0.80) + vec3(0.03, 0.04, 0.04);
                wetCol = mix(wetCol, vec3(0.72, 0.78, 0.74), (1.0 - smoothstep(0.0, 0.08, length(pv + uLight * pr * 0.55) - pr * 0.3)) * 0.35);
                col = mix(col, wetCol, pud);
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
        if (uGhost < 0.5 && uGrid > 0.0) {
            vec2 gq = min(fract(q), 1.0 - fract(q));
            float gw = length(fwidth(q));
            // (Light over what is nearly black: a chasm, deep water.)
            vec3 gc = mix(vec3(0.45, 0.43, 0.40), INK, smoothstep(0.05, 0.16, dot(col, vec3(0.3, 0.59, 0.11))));
            // Like the surface grid: 0.4-1.2 px soft line.
            float gpx = min(gq.x, gq.y) / max(gw, 1e-5);
            col = mix(col, gc, (1.0 - smoothstep(0.4, 1.2, gpx)) * uGridAlpha);
        }
        alpha = 1.0;
    }
    // The wall's inked outline.
    col = mix(col, INK, (1.0 - smoothstep(0.0, aa * 1.6, abs(d))) * 0.85);
    // A neighbouring section: its floors and their rims only, faint.
    if (uGhost > 0.5) alpha *= d < -rim ? 0.0 : 0.42;
    finalColor = vec4(col * alpha, alpha) * uColor;
}
