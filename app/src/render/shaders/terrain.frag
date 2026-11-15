#version 300 es
// Terrain tile, ink-and-parchment cartographic style:
// biome tint (with ecotone blending) × zoom-adaptive NW hillshade, water with depth and coast
// ripple lines, beaches, map symbols (forest canopy, conifers, marsh, sand, tufts, scree),
// slope hachures, ink contours and coastline, paper grain.
//
// uWorld: the tile's origin (ft) and size (ft), for fields anchored to the world (tundra snow).
// uSeed: the world's seed as two 16-bit halves.
// uTex:  f16 (h - uBase, dh/dx, dh/dy, water - uBase) per sample.
// uBio:  u8 (primary biome, secondary biome, blend, coast distance sqrt-encoded) per sample.
// Symbol patterns live in two world-anchored octaves (uPat0/uPat1: origin mod 256 in cells,
// cells per tile) blended by uPatMix, so they stay put while panning and zooming.

in vec2 vUV;
in vec2 vLocal;

out vec4 finalColor;

uniform highp sampler2D uTex;
uniform highp sampler2D uBio;

// Per tile.
uniform float uBase;
uniform float uAlpha;
uniform vec2 uMapEnd;
uniform vec2 uGrainOrigin;
uniform float uTilePx;
uniform vec4 uPat0;
uniform vec4 uPat1;
uniform vec4 uWorld;

// Per frame.
uniform float uExag;
uniform float uSea;
uniform float uContour;
uniform float uIndexEvery;
uniform float uFtPerPx;
uniform float uPatMix;
uniform float uPatAlpha;
uniform vec2 uSeed;

const vec3 INK = vec3(0.20, 0.18, 0.16);
const vec3 CONTOUR_INK = vec3(0.42, 0.33, 0.24);
const vec3 SAND = vec3(0.90, 0.85, 0.68);
const float SAMPLES = 257.0;
const float COAST_ENCODE_FT = 250000.0;

const vec3 PALETTE[19] = vec3[19](
    vec3(0.54, 0.63, 0.66), // ocean
    vec3(0.62, 0.72, 0.74), // lake
    vec3(0.95, 0.95, 0.93), // ice
    vec3(0.80, 0.80, 0.66), // tundra (grass and snow patches, heather among the grass, see `moorTone`)
    vec3(0.74, 0.71, 0.64), // alpine
    vec3(0.64, 0.69, 0.57), // taiga
    vec3(0.68, 0.74, 0.54), // temperate forest
    vec3(0.58, 0.68, 0.52), // temperate rainforest
    vec3(0.82, 0.82, 0.63), // grassland
    vec3(0.85, 0.81, 0.64), // steppe
    vec3(0.84, 0.79, 0.66), // cold desert
    vec3(0.91, 0.83, 0.63), // hot desert
    vec3(0.86, 0.81, 0.59), // savanna
    vec3(0.52, 0.65, 0.45), // jungle
    vec3(0.66, 0.71, 0.60), // swamp
    vec3(0.53, 0.48, 0.45), // volcanic
    vec3(0.93, 0.91, 0.86), // salt flat
    vec3(0.60, 0.57, 0.58), // blighted woods
    vec3(0.60, 0.56, 0.51)  // ashlands
);

// Symbol per biome: 0 none, 1 canopy, 2 conifer, 3 dense canopy, 4 marsh, 5 sand, 6 sparse dots, 7 tufts, 8 scree,
// 9 heath (heather clumps and grass tufts).
const int SYMBOL[19] = int[19](0, 0, 0, 9, 8, 2, 1, 1, 7, 7, 5, 5, 6, 3, 4, 5, 0, 2, 8);
const vec3 HEATHER = vec3(0.79, 0.74, 0.70);
const vec3 MOOR_GRASS = vec3(0.75, 0.79, 0.60);
const vec3 SNOW = vec3(0.94, 0.95, 0.94);

float hash21(vec2 p) {
    p = fract(p * vec2(123.34, 456.21));
    p += dot(p, p + 45.32);
    return fract(p.x * p.y);
}

vec2 hash22(vec2 p) {
    float a = hash21(p);
    return vec2(a, hash21(p + a * 17.0));
}

float pnoise(vec2 p, float period) {
    vec2 i = floor(p);
    vec2 f = fract(p);
    f = f * f * (3.0 - 2.0 * f);
    float a = hash21(mod(i, period));
    float b = hash21(mod(i + vec2(1.0, 0.0), period));
    float c = hash21(mod(i + vec2(0.0, 1.0), period));
    float d = hash21(mod(i + vec2(1.0, 1.0), period));
    return mix(mix(a, b, f.x), mix(c, d, f.x), f.y);
}

float segment(vec2 p, vec2 a, vec2 b) {
    vec2 pa = p - a, ba = b - a;
    float h = clamp(dot(pa, ba) / dot(ba, ba), 0.0, 1.0);
    return length(pa - ba * h);
}

// Map symbol at pattern coordinate p (cells). Returns (fill, ink, tone, ground shadow):
// fill and ink are coverage; tone (dark > 0, lit < 0) and ground shadow come premultiplied by coverage.
vec4 symbol(vec2 p, int kind, float aa) {
    vec2 c = mod(floor(p), 256.0);
    vec2 f = fract(p);
    vec2 h = hash22(c);
    vec2 o = 0.5 + (h - 0.5) * 0.45;
    vec2 d = f - o;
    if (kind == 1 || kind == 3) {
        // Tree clumps: per cell 1, 2 or (rarely) 3 lobed crowns, each kept inside its own part of the
        // cell (the nearest of m points), so no neighbour search. A crown is 3-5 circles on a slightly
        // squashed ring (each wider than the ring, so the middle is covered): inked outline, a dark
        // crescent on the SE side away from the NW light, a lit spot on the NW lobes, a ground shadow
        // to the SE and, at close zoom, a trunk on some. Crown reach is 2.45 R (dense 2.3 R) from its point.
        bool dense = kind == 3;
        // Groves and clearings: value noise per cell (period 32 = 256 / 8, so it tiles with the cells).
        // (Ordinary forest stays mostly wooded: groves nearly closed, clearings the exception.)
        float grove = smoothstep(0.2, 0.55, pnoise((c + 0.5) * 0.125, 32.0));
        if (h.x > (dense ? 0.84 + 0.14 * grove : 0.3 + 0.65 * grove)) return vec4(0.0);
        float h3 = hash21(c + 31.7);
        vec2 g = hash22(c + 3.7);
        int m = h.y < (dense ? 0.8 : 0.68) ? 1 : h.y < (dense ? 0.97 : 0.93) ? 2 : 3;
        vec2 ctr = vec2(0.5);
        float sub = 0.0;
        if (m > 1) {
            vec2 cs = m == 2 ? vec2(-1.0, 0.0) : vec2(-0.5, 0.8660);
            vec2 v = normalize(g - 0.5 + 1e-4) * (m == 2 ? 0.25 : 0.27);
            float best = 1e3;
            for (int k = 0; k < 3; k++) {
                if (k < m) {
                    vec2 e = f - 0.5 - v;
                    float dd = dot(e, e);
                    if (dd < best) { best = dd; ctr = 0.5 + v; sub = float(k + 1); }
                    v = vec2(v.x * cs.x - v.y * cs.y, v.x * cs.y + v.y * cs.x);
                }
            }
        }
        vec2 k2 = hash22(c + sub * 4.13 + 0.71);
        float reach;
        if (m == 1) {
            reach = dense ? 0.44 + 0.03 * k2.x : 0.34 * (0.7 + 0.6 * k2.x);
            ctr += (g - 0.5) * 2.0 * max(0.0, 0.48 - reach);
        } else {
            reach = (m == 2 ? 0.24 : 0.22) * (0.82 + 0.18 * k2.x);
        }
        // Dense crowns nearly fill the cell (fake overlap); they get no trunk, so need less room.
        float R = reach / (dense ? 2.3 : 2.45);
        vec2 q = f - ctr + vec2(0.0, 0.1 * R);
        int n = dense ? 4 + int(h3 * 1.999) : 3 + int(fract(h3 + k2.y * 3.7) * 2.999);
        vec2 cs = n == 3 ? vec2(-0.5, 0.8660) : n == 4 ? vec2(0.0, 1.0) : vec2(0.3090, 0.9511);
        float a0 = k2.y * 6.2831;
        vec2 v = vec2(cos(a0), sin(a0)) * R;
        float sd = 1e3;
        vec2 dw = vec2(0.0);
        float rw = R;
        for (int k = 0; k < 5; k++) {
            if (k < n) {
                float rk = R * (1.05 + 0.3 * fract(h3 * 7.31 * float(k + 1) + k2.x));
                vec2 dk = q - v * vec2(1.0, 0.85);
                float e = length(dk) - rk;
                if (e < sd) { sd = e; dw = dk; rw = rk; }
                v = vec2(v.x * cs.x - v.y * cs.y, v.x * cs.y + v.y * cs.x);
            }
        }
        float fill = 1.0 - smoothstep(-aa, 0.0, sd);
        // Crescent: inside the lobe but outside a copy of it shifted towards the NW light.
        float shade = smoothstep(rw - aa, rw + aa, length(dw + vec2(rw * 0.6)));
        float lit = (1.0 - smoothstep(rw * 0.45 - aa, rw * 0.45 + aa, length(dw + vec2(rw * 0.4)))) * step(dot(q - dw, vec2(1.0)), 0.0);
        float tone = (dense ? 0.35 : 0.0) + shade - 0.7 * lit * (1.0 - shade);
        float ink = 1.0 - smoothstep(0.0, aa * 1.6, abs(sd));
        float tw = (dense ? 0.0 : 1.0) * step(fract(h3 * 5.1 + k2.y), 0.55) * (1.0 - smoothstep(40.0, 120.0, uFtPerPx));
        float trunk = tw * (1.0 - smoothstep(0.0, aa, segment(q, vec2(0.0, R), vec2(0.0, 2.5 * R)))) * (1.0 - fill);
        float ground = 1.0 - smoothstep(-aa, aa, length((q - vec2(0.28, 0.32) * R) * vec2(1.0, 1.25)) - 1.95 * R);
        return vec4(fill, 1.2 * max(ink, trunk), tone * fill, ground * (1.0 - fill));
    }
    if (kind == 2) {
        if (h.x > 0.8) return vec4(0.0);
        // Isosceles triangle, apex up, like a conifer.
        vec2 q = vec2(abs(d.x), d.y + 0.05);
        float w = 0.22, top = -0.30, bot = 0.22;
        float t = (q.y - top) / (bot - top);
        float edge = q.x - w * t;
        float inside = step(top, q.y) * step(q.y, bot);
        float fill = inside * (1.0 - smoothstep(-aa, 0.0, edge));
        float ink = max(inside * (1.0 - smoothstep(0.0, aa * 1.2, abs(edge))), (1.0 - smoothstep(0.0, aa, abs(q.y - bot))) * step(q.x, w));
        float trunk = 1.0 - smoothstep(0.0, aa, segment(d, vec2(0.0, bot - 0.05), vec2(0.0, bot + 0.1)));
        return vec4(fill, max(ink, trunk), 0.0, 0.0);
    }
    if (kind == 4) {
        if (h.x > 0.6) return vec4(0.0);
        float l = segment(f, vec2(0.25, o.y), vec2(0.75, o.y));
        l = min(l, segment(f, vec2(0.35, o.y + 0.16), vec2(0.65, o.y + 0.16)));
        return vec4(0.0, 1.0 - smoothstep(0.0, aa, l), 0.0, 0.0);
    }
    if (kind == 5 || kind == 6) {
        float p0 = kind == 5 ? 0.35 : 0.12;
        if (h.x > p0) return vec4(0.0);
        float r = kind == 5 ? 0.06 : 0.09;
        return vec4(0.0, 1.0 - smoothstep(r - aa, r, length(d)), 0.0, 0.0);
    }
    if (kind == 7) {
        if (h.x > 0.06) return vec4(0.0);
        float l = min(segment(d, vec2(-0.1, 0.08), vec2(-0.04, -0.08)), segment(d, vec2(0.0, 0.08), vec2(0.0, -0.1)));
        l = min(l, segment(d, vec2(0.1, 0.08), vec2(0.04, -0.08)));
        return vec4(0.0, 1.0 - smoothstep(0.0, aa, l), 0.0, 0.0);
    }
    if (kind == 9) {
        // Heath: a few low heather clumps (three lobes, inked), grass tufts between them (tundra:
        // drawn on its grass only).
        if (h.x < 0.035) {
            float r = 0.14 + 0.04 * h.y;
            float sd = min(min(length(d - vec2(-0.6, 0.25) * r), length(d - vec2(0.6, 0.25) * r)), length(d + vec2(0.0, 0.45) * r)) - r;
            float fill = 1.0 - smoothstep(-aa, 0.0, sd);
            float ink = 1.0 - smoothstep(0.0, aa * 1.4, abs(sd));
            float shade = smoothstep(r - aa, r + aa, length(d + vec2(0.35 * r, 0.6 * r)));
            return vec4(fill, ink, (0.25 + 0.5 * shade) * fill, 0.0);
        }
        if (h.x > 0.13) return vec4(0.0);
        float l = min(segment(d, vec2(-0.07, 0.05), vec2(-0.03, -0.06)), segment(d, vec2(0.0, 0.05), vec2(0.0, -0.07)));
        l = min(l, segment(d, vec2(0.07, 0.05), vec2(0.03, -0.06)));
        return vec4(0.0, 0.8 * (1.0 - smoothstep(0.0, aa, l)), 0.0, 0.0);
    }
    if (kind == 8) {
        if (h.x > 0.45) return vec4(0.0);
        float a = h.y * 6.2831;
        vec2 dir = vec2(cos(a), sin(a)) * 0.12;
        return vec4(0.0, 1.0 - smoothstep(0.0, aa, segment(d, -dir, dir)), 0.0, 0.0);
    }
    return vec4(0.0);
}

// Tundra snow: the battlemap's field (`t0::biome::tundra_snow`, same constants and 32-bit hashing,
// so the map and the battlemaps agree on where snow lies) at world position w (ft). An octave
// fades to its mean (a warp to nothing) once its wavelength is under ~12 px, so it never shimmers.
uint snowHash(uint h) {
    h ^= h >> 16;
    h *= 0x7feb352du;
    h ^= h >> 15;
    h *= 0x846ca68bu;
    return h ^ (h >> 16);
}

float snowNoise(vec2 p, uint s) {
    vec2 i = floor(p);
    vec2 f = p - i;
    vec2 u = f * f * (3.0 - 2.0 * f);
    ivec2 c = ivec2(i);
    uint x0 = uint(c.x) * 0x9e3779b1u, x1 = uint(c.x + 1) * 0x9e3779b1u;
    uint y0 = uint(c.y) * 0x85ebca77u, y1 = uint(c.y + 1) * 0x85ebca77u;
    float a = float(snowHash(x0 ^ y0 ^ s) >> 8u);
    float b = float(snowHash(x1 ^ y0 ^ s) >> 8u);
    float d = float(snowHash(x0 ^ y1 ^ s) >> 8u);
    float e = float(snowHash(x1 ^ y1 ^ s) >> 8u);
    return mix(mix(a, b, u.x), mix(d, e, u.x), u.y) / 16777216.0;
}

float snowFade(float wavelength) {
    return clamp((wavelength / uFtPerPx - 4.0) / 8.0, 0.0, 1.0);
}

float tundraSnow(vec2 w) {
    uint seed = uint(uSeed.x) | (uint(uSeed.y) << 16u);
    uint s[8];
    for (int k = 1; k < 8; k++) s[k] = snowHash(seed + uint(k) * 0x632be5abu);
    vec2 r = vec2(0.8 * w.x - 0.6 * w.y, 0.6 * w.x + 0.8 * w.y);
    r += 700.0 * snowFade(2600.0) * (vec2(snowNoise(r / 2600.0, s[1]), snowNoise(r / 2600.0, s[2])) - 0.5);
    float f = snowFade(520.0);
    if (f > 0.0) r += 160.0 * f * (vec2(snowNoise(r / 520.0, s[3]), snowNoise(r / 520.0, s[4])) - 0.5);
    float n = 0.55 * mix(0.5, snowNoise(r / 1500.0, s[5]), snowFade(1500.0));
    n += 0.3 * mix(0.5, snowNoise(r / 620.0, s[6]), snowFade(620.0));
    f = snowFade(240.0);
    return n + 0.15 * (f > 0.0 ? mix(0.5, snowNoise(r / 240.0, s[7]), f) : 0.5);
}

// Tundra tone at pattern coordinate p (cells): big soft patches of grass and of snow, with a
// little heather among the grass. `world` is `tundraSnow` here and `near` how much of it to
// use: zoomed far out (where its octaves have faded) the patches come from pattern noise
// instead, a few to a screen. The pattern noise is world-anchored and seamless: its lattice is
// turned 45 degrees by a whole-number matrix and repeats every 256 cells like the pattern
// origin, its coordinates warped by a slower noise of the same period so outlines wind.
// `snow` is set to the snow cover (0..1); soft edges, no ink.
vec3 moorTone(vec3 col, vec2 p, float world, float near, out float snow) {
    vec2 r = vec2(p.x + p.y, p.y - p.x);
    r += 20.0 * (vec2(pnoise(r / 64.0 + 13.0, 4.0), pnoise(r / 64.0 + 29.0, 4.0)) - 0.5);
    r += 6.0 * (vec2(pnoise(r / 16.0 + 41.0, 16.0), pnoise(r / 16.0 + 53.0, 16.0)) - 0.5);
    float n = 0.65 * pnoise(r / 32.0, 8.0) + 0.25 * pnoise(r / 16.0 + 3.0, 16.0) + 0.1 * pnoise(r / 8.0 + 9.0, 32.0);
    float m = near < 1.0 ? 0.6 * pnoise(r / 32.0 + 5.0, 8.0) + 0.3 * pnoise(r / 16.0 + 7.0, 16.0) + 0.1 * pnoise(r / 8.0 + 17.0, 32.0) : 0.5;
    // Edges soft, ~40 px wide: the world field's are sharpened as it is zoomed in (it changes
    // about 0.0005 a foot), the pattern field's stay at a few cells.
    float w = mix(0.03, clamp(0.008 * uFtPerPx, 0.005, 0.03), near);
    snow = smoothstep(0.5 - w, 0.5 + w, mix(m, world, near));
    vec3 grass = mix(col, col * (MOOR_GRASS / PALETTE[3]), 0.6);
    grass = mix(grass, grass * (HEATHER / PALETTE[3]), 0.35 * smoothstep(0.58, 0.72, n));
    return mix(grass, SNOW, 0.8 * snow);
}

void main() {
    if (vLocal.x > uMapEnd.x || vLocal.y > uMapEnd.y) discard;
    vec4 t = texture(uTex, vUV);
    float h = uBase + t.r;
    float water = uBase + t.a;
    vec2 g = t.gb;
    float depth = water - h;
    bool wet = depth > 0.0;

    ivec2 texel = ivec2(clamp(floor(vUV * SAMPLES), 0.0, SAMPLES - 1.0));
    vec4 bio = texelFetch(uBio, texel, 0);
    int b1 = int(bio.r * 255.0 + 0.5);
    int b2 = int(bio.g * 255.0 + 0.5);
    // Dry ground whose coarse cell was water (sub-cell coastlines): borrow a land biome.
    if (b1 <= 1) b1 = b2 > 1 ? b2 : 8;
    if (b2 <= 1) b2 = b1;
    float blend = bio.b;
    float coastEnc = texture(uBio, vUV).a;
    float coastFt = coastEnc * coastEnc * COAST_ENCODE_FT;

    float slope = length(g);
    vec3 col;
    float moor = 0.0;
    float snowy = 0.0;
    if (wet) {
        float d = clamp(depth / 6000.0, 0.0, 1.0);
        col = mix(vec3(0.70, 0.78, 0.77), vec3(0.52, 0.62, 0.66), sqrt(d));
    } else {
        col = mix(PALETTE[b1], PALETTE[b2], blend * 0.5);
        // Tundra: grass and snow patches, the two pattern octaves crossfaded.
        moor = (b1 == 3 ? 1.0 - blend * 0.5 : 0.0) + (b2 == 3 ? blend * 0.5 : 0.0);
        if (moor > 0.0) {
            float near = snowFade(1500.0);
            float world = near > 0.0 ? tundraSnow(uWorld.xy + vLocal * uWorld.z) : 0.5;
            float sn0, sn1;
            vec3 m0 = moorTone(col, uPat0.xy + vLocal * uPat0.z, world, near, sn0);
            vec3 m1 = moorTone(col, uPat1.xy + vLocal * uPat1.z, world, near, sn1);
            col = mix(col, mix(m0, m1, uPatMix), moor);
            snowy = mix(sn0, sn1, uPatMix);
        }
        float above = h - water;
        bool beachy = b1 != 2 && b1 != 3 && b1 != 5 && b1 != 14;
        if (beachy && above < 6.0 && slope < 0.08) col = mix(col, SAND, 1.0 - smoothstep(3.0, 6.0, above));
    }

    // Hillshade, normalized so flat ground is 1.0; light from the north-west.
    vec3 n = normalize(vec3(-g * uExag, 1.0));
    vec3 L = normalize(vec3(-1.0, -1.0, 1.3));
    float lambert = dot(n, L) / L.z;
    float shade = wet ? mix(1.0, lambert, 0.1) : lambert;
    col *= clamp(mix(1.0, shade, 0.85), 0.3, 1.3);

    // Map symbols, blended across two octaves; ecotones dither between the two biomes.
    if (!wet && uPatAlpha > 0.0) {
        vec2 p0 = uPat0.xy + vLocal * uPat0.z;
        vec2 p1 = uPat1.xy + vLocal * uPat1.z;
        float aa0 = max(fwidth(p0.x), 1e-4);
        float aa1 = max(fwidth(p1.x), 1e-4);
        // Symbol lattices turned ~20 deg so rows don't line up with the screen. The matrix entries are
        // multiples of 1/256, so a 256-cell shift of the origin stays a whole-cell shift (no tile seams).
        mat2 turn = mat2(0.9375, -0.34375, 0.34375, 0.9375);
        vec2 r0 = turn * p0;
        vec2 r1 = turn * p1;
        vec2 c0 = mod(floor(r0), 256.0);
        vec2 c1 = mod(floor(r1), 256.0);
        // Ecotones: only strong blends scatter the neighbouring biome's symbols.
        float mixin = max(0.0, blend - 0.55) * 0.9;
        // Each octave is only evaluated when the crossfade needs it (uniform branch). The crossfade
        // drops whole symbols cell by cell instead of fading them, so neither octave shows as ghosts;
        // the finer octave is drawn over the coarser one.
        vec4 s0 = vec4(0.0);
        vec4 s1 = vec4(0.0);
        if (uPatMix < 0.98) {
            s0 = symbol(r0, SYMBOL[hash21(c0 + 7.0) < mixin ? b2 : b1], aa0);
            s0 *= clamp(((1.0 - uPatMix) * 1.1 - 0.05 - hash21(c0 + 13.1)) * 20.0, 0.0, 1.0);
        }
        if (uPatMix > 0.02) {
            s1 = symbol(r1, SYMBOL[hash21(c1 + 7.0) < mixin ? b2 : b1], aa1);
            s1 *= clamp((uPatMix * 1.1 - 0.05 - hash21(c1 + 13.1)) * 20.0, 0.0, 1.0);
        }
        vec4 s = (s0 + s1 * (1.0 - max(s0.x, s0.y))) * uPatAlpha;
        // Tundra's heath symbols only on its grass, never on the snow.
        if (moor > 0.0) s *= 1.0 - smoothstep(0.25, 0.5, snowy);
        col *= 1.0 - 0.16 * s.w;
        // Crown fill: a little darker and more saturated than the ground, then shaded.
        vec3 crown = col * 0.78;
        crown = mix(vec3(dot(crown, vec3(0.3, 0.59, 0.11))), crown, 1.3);
        col = mix(col, crown, s.x);
        col *= 1.0 - 0.25 * s.z;
        col = mix(col, INK, min(s.y * 0.75, 0.9));

        // Hachures: fine strokes down steep slopes (strokes follow the fall line).
        float steep = smoothstep(0.35, 1.2, slope * uExag);
        if (steep > 0.0) {
            vec2 dir = normalize(g + 1e-9);
            float across = dot(p0 * 2.2, vec2(-dir.y, dir.x));
            float line = 1.0 - smoothstep(0.0, aa0 * 2.2, abs(fract(across) - 0.5) - 0.34);
            col = mix(col, INK, line * steep * 0.28 * uPatAlpha);
        }
    }

    // Contours on land; fade out where they would crowd closer than a few pixels.
    // Only where the ground slopes in the samples: level ground at a contour height (a plateau's
    // top) has d ~ 0 everywhere, and the height's f16 rounding per tile or a hair of float noise
    // must not ink it across. (fw never 0: smoothstep's edges must not meet.)
    if (!wet && uContour > 0.0) {
        float f = (h - uSea) / uContour;
        float fw = max(fwidth(f), 1e-6);
        float d = abs(fract(f + 0.5) - 0.5);
        float isIndex = mod(floor(f + 0.5), uIndexEvery) < 0.5 ? 1.0 : 0.0;
        float w = mix(0.5, 0.9, isIndex);
        float line = 1.0 - smoothstep(w * fw, (w + 1.0) * fw, d);
        float room = 1.0 - smoothstep(0.12, 0.3, fw);
        float sloped = smoothstep(5e-5, 2e-4, slope);
        col = mix(col, CONTOUR_INK, line * room * sloped * mix(0.22, 0.45, isIndex));
    }

    // Coast ripples: evenly spaced ink lines offshore (sea only).
    if (wet && coastFt > 0.0) {
        float r = coastFt / uFtPerPx / 6.0;
        float fr = max(fwidth(r), 1e-4);
        float line = 1.0 - smoothstep(0.35 * fr, 1.1 * fr, abs(fract(r + 0.5) - 0.5));
        col = mix(col, INK, line * (1.0 - smoothstep(1.5, 2.5, r)) * step(0.5, r) * 0.22);
    }

    // Shoreline ink (sea and lakes). Width comes from the terrain's own slope: the water
    // surface is flat, and its field has jumps where it ends that must not draw lines.
    // A shoal that never reaches the surface has no shore: on the sea side, only near a coast
    // (lakes are land to the coast distance, so their shores stay). Out at sea, an island's
    // line is all on its dry side, twice as wide.
    float offshore = smoothstep(9000.0, 18000.0, coastFt);
    float fwh = max(fwidth(h), 1e-4) * (wet ? 1.0 : 1.0 + offshore);
    float shore = (1.0 - smoothstep(0.6, 1.6, abs(depth) / fwh)) * (1.0 - smoothstep(1500.0, 3000.0, abs(depth)));
    if (wet) shore *= 1.0 - offshore;
    col = mix(col, INK, shore * 0.9);

    // Paper grain (periodic over 512 px).
    vec2 gp = uGrainOrigin + vLocal * uTilePx;
    col *= 0.96 + 0.06 * pnoise(gp * 0.25, 128.0);

    finalColor = vec4(col * uAlpha, uAlpha);
}
