#version 300 es
// Terrain tile, ink-and-parchment cartographic style:
// biome tint (with ecotone blending) × zoom-adaptive NW hillshade, water with depth and coast
// ripple lines, beaches, map symbols (forest canopy, conifers, marsh, sand, tufts, scree),
// slope hachures, ink contours and coastline, paper grain.
//
// uTex:  f16 (h - uBase, dh/dx, dh/dy, water - uBase) per sample.
// uBio:  u8 (primary biome, secondary biome, blend, coast distance sqrt-encoded) per sample.
// uTundra: tundra's ground texture (`tundraTexture.ts`: heather, grass shade, -, grass light),
//        repeating every TUNDRA_CELLS pattern cells. (Its snow is drawn on battlemaps only.)
// Symbol patterns live in two world-anchored octaves (uPat0/uPat1: origin mod 256 in cells,
// cells per tile) blended by uPatMix, so they stay put while panning and zooming.

in vec2 vUV;
in vec2 vLocal;

out vec4 finalColor;

uniform highp sampler2D uTex;
uniform highp sampler2D uBio;
uniform highp sampler2D uTundra;

// Per tile.
uniform float uBase;
uniform float uAlpha;
uniform vec2 uMapEnd;
uniform vec2 uGrainOrigin;
uniform float uTilePx;
uniform vec4 uPat0;
uniform vec4 uPat1;

// Per frame.
uniform float uExag;
uniform float uSea;
uniform float uContour;
uniform float uIndexEvery;
uniform float uFtPerPx;
uniform float uPatMix;
uniform float uPatAlpha;
uniform float uSymAlpha;
// Forest tufts' strength by zoom (gone as the battlemap's trees come in).
uniform float uForestAlpha;

const vec3 INK = vec3(0.20, 0.18, 0.16);
const vec3 CONTOUR_INK = vec3(0.42, 0.33, 0.24);
const vec3 SAND = vec3(0.90, 0.85, 0.68);
const float SAMPLES = 257.0;
// Cliff faces, as the battlemap's (`ground.frag`): the grade they start at and three matte tones
// by the way the face looks (lit from the upper left).
const float CLIFF_T = 1.2;
const vec3 ROCK_LIT = vec3(0.58, 0.53, 0.45);
const vec3 ROCK_MID = vec3(0.45, 0.41, 0.36);
const vec3 ROCK_SHADE = vec3(0.33, 0.30, 0.27);
const float COAST_ENCODE_FT = 250000.0;

const vec3 PALETTE[19] = vec3[19](
    vec3(0.54, 0.63, 0.66), // ocean
    vec3(0.62, 0.72, 0.74), // lake
    vec3(0.95, 0.95, 0.93), // ice
    vec3(0.80, 0.80, 0.66), // tundra (painted grass with heather: see `tundraTone`)
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

// Symbol per biome: 0 none, 4 marsh, 5 sand, 7 tufts, 8 scree. No tree symbols: trees are the
// battlemap's (its sprites); on the map the woods are their area tint alone.
const int SYMBOL[19] = int[19](0, 0, 0, 7, 8, 0, 0, 0, 7, 7, 5, 5, 0, 0, 4, 5, 0, 0, 8);
// Woods per biome: 0 none, 1 conifer (taiga), 2 leafy (temperate, rainforest, blighted), 3 jungle.
const int WOOD[19] = int[19](0, 0, 0, 0, 0, 1, 2, 2, 0, 0, 0, 0, 0, 3, 0, 0, 0, 2, 0);
// Per wood kind (none, conifer, leafy, jungle): tuft density.
const float TUFTS[4] = float[4](0.0, 0.3, 0.26, 0.4);
const vec3 TUNDRA_PALE = vec3(0.88, 0.89, 0.80);
const vec3 HEATHER_DAB = vec3(0.78, 0.72, 0.74);
// Tundra: `uTundra` repeats every this many pattern cells (a divisor of 256: seamless where the
// pattern origin wraps).
const float TUNDRA_CELLS = 16.0;

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

// Map symbol at pattern coordinate p (cells): its ink coverage.
float symbol(vec2 p, int kind, float aa) {
    vec2 c = mod(floor(p), 256.0);
    vec2 f = fract(p);
    vec2 h = hash22(c);
    vec2 o = 0.5 + (h - 0.5) * 0.45;
    vec2 d = f - o;
    if (kind == 4) {
        if (h.x > 0.6) return 0.0;
        float l = segment(f, vec2(0.25, o.y), vec2(0.75, o.y));
        l = min(l, segment(f, vec2(0.35, o.y + 0.16), vec2(0.65, o.y + 0.16)));
        return 1.0 - smoothstep(0.0, aa, l);
    }
    if (kind == 5) {
        if (h.x > 0.35) return 0.0;
        return 1.0 - smoothstep(0.06 - aa, 0.06, length(d));
    }
    if (kind == 7) {
        if (h.x > 0.06) return 0.0;
        float l = min(segment(d, vec2(-0.1, 0.08), vec2(-0.04, -0.08)), segment(d, vec2(0.0, 0.08), vec2(0.0, -0.1)));
        l = min(l, segment(d, vec2(0.1, 0.08), vec2(0.04, -0.08)));
        return 1.0 - smoothstep(0.0, aa, l);
    }
    if (kind == 8) {
        if (h.x > 0.45) return 0.0;
        float a = h.y * 6.2831;
        vec2 dir = vec2(cos(a), sin(a)) * 0.12;
        return 1.0 - smoothstep(0.0, aa, segment(d, -dir, dir));
    }
    return 0.0;
}

// Tundra: pale frosted grass, faintly painted by the ground texture `d` (`uTundra`, both pattern
// octaves crossfaded like the symbols): lit humps, shaded tussocks, the odd heather dab.
vec3 tundraTone(vec3 col, vec4 d) {
    vec3 grass = mix(col, TUNDRA_PALE, 0.45);
    grass *= 1.0 + 0.06 * d.a - 0.10 * d.g;
    return mix(grass, grass * (HEATHER_DAB / TUNDRA_PALE), 0.3 * d.r);
}
// Forest tufts (Roadmap 7 item 197), in a world-anchored pattern octave `q` (cells, the
// symbol octave scaled by a power of two; hashes mod `per`, its 256-cell wrap, so tiles agree);
// `aa` is cells per pixel. Returns a tint factor around 1.

// Tufts: a short drawn mark in some cells: a scallop (leafy), a caret (conifer), a double scallop
// (jungle). Line about 0.9 px wide.
float tuft(vec2 q, float per, float aa, int kind, float dens) {
    vec2 c = mod(floor(q), per);
    vec2 h = hash22(c);
    if (h.x > dens) return 1.0;
    // (Back from the 45-deg lattice to the screen's axes: the marks stand upright.)
    vec2 d = mat2(0.5, 0.5, -0.5, 0.5) * (fract(q) - (0.5 + (h - 0.5) * 0.5)) * 1.4142;
    float l;
    if (kind == 1) {
        l = min(segment(d, vec2(-0.13, 0.07), vec2(0.0, -0.1)), segment(d, vec2(0.13, 0.07), vec2(0.0, -0.1)));
    } else if (kind == 3) {
        vec2 a = d - vec2(-0.1, 0.03), b = d - vec2(0.1, 0.03);
        l = min(a.y < 0.0 ? abs(length(a) - 0.1) : length(vec2(abs(a.x) - 0.1, a.y)), b.y < 0.0 ? abs(length(b) - 0.1) : length(vec2(abs(b.x) - 0.1, b.y)));
    } else {
        vec2 a = d - vec2(0.0, 0.05);
        l = a.y < 0.0 ? abs(length(a) - 0.15) : length(vec2(abs(a.x) - 0.15, a.y));
    }
    float w = aa * 0.45;
    return 1.0 - 0.16 * (1.0 - smoothstep(w, w + aa, l));
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
    // How the grade changes per pixel (taken here, in uniform flow): the cliffs' top and foot.
    vec2 dslope = vec2(dFdx(slope), dFdy(slope));
    // Pattern coordinates (cells) of both octaves, and their screen gradients (taken here, outside
    // any branch, for the tundra texture's reads under one).
    vec2 p0 = uPat0.xy + vLocal * uPat0.z;
    vec2 p1 = uPat1.xy + vLocal * uPat1.z;
    vec2 lx = dFdx(vLocal);
    vec2 ly = dFdy(vLocal);
    vec3 col;
    if (wet) {
        float d = clamp(depth / 6000.0, 0.0, 1.0);
        col = mix(vec3(0.70, 0.78, 0.77), vec3(0.52, 0.62, 0.66), sqrt(d));
    } else {
        col = mix(PALETTE[b1], PALETTE[b2], blend * 0.5);
        // Tundra: its painted grass (`tundraTone`).
        float moor = (b1 == 3 ? 1.0 - blend * 0.5 : 0.0) + (b2 == 3 ? blend * 0.5 : 0.0);
        if (moor > 0.0) {
            float k0 = uPat0.z / TUNDRA_CELLS, k1 = uPat1.z / TUNDRA_CELLS;
            vec4 d = mix(textureGrad(uTundra, p0 / TUNDRA_CELLS, lx * k0, ly * k0), textureGrad(uTundra, p1 / TUNDRA_CELLS, lx * k1, ly * k1), uPatMix);
            col = mix(col, tundraTone(col, d), moor);
        }
        // Woods: hatched tufts, feathered by the biome blend.
        int w1 = WOOD[b1], w2 = WOOD[b2];
        float wood = (w1 != 0 ? 1.0 - blend * 0.5 : 0.0) + (w2 != 0 ? blend * 0.5 : 0.0);
        if (uForestAlpha > 0.0 && wood > 0.0) {
            float fa = uForestAlpha * smoothstep(0.15, 0.6, wood);
            int wk = w1 != 0 ? w1 : w2;
            int wk2 = w2 != 0 ? w2 : w1;
            float m = uPatMix;
            float tex0 = 1.0, tex1 = 1.0;
            // Marks (cells 1.4 of a symbol cell) dropped cell by cell across the octave crossfade, as the
            // symbols are. The lattice is turned 45 deg (so rows don't line up with the screen) by an
            // integer matrix: a 256-cell wrap of the origin is a whole number of periods `per`, so
            // tiles agree.
            const float S = 0.5;
            const float per = 256.0 * S;
            mat2 diag = mat2(1.0, -1.0, 1.0, 1.0);
            vec2 q0 = diag * p0 * S, q1 = diag * p1 * S;
            float a0 = (abs(lx.x) + abs(ly.x)) * uPat0.z * S * 1.4142, a1 = (abs(lx.x) + abs(ly.x)) * uPat1.z * S * 1.4142;
            float dn = mix(TUFTS[wk], TUFTS[wk2], blend * 0.5);
            if (m < 0.98) {
                vec2 c0 = mod(floor(q0), per);
                float keep = clamp(((1.0 - m) * 1.1 - 0.05 - hash21(c0 + 13.1)) * 20.0, 0.0, 1.0);
                if (keep > 0.0) tex0 = mix(1.0, tuft(q0, per, a0, hash21(c0 + 7.0) < blend * 0.5 ? wk2 : wk, dn), keep);
            }
            if (m > 0.02) {
                vec2 c1 = mod(floor(q1), per);
                float keep = clamp((m * 1.1 - 0.05 - hash21(c1 + 13.1)) * 20.0, 0.0, 1.0);
                if (keep > 0.0) tex1 = mix(1.0, tuft(q1, per, a1, hash21(c1 + 7.0) < blend * 0.5 ? wk2 : wk, dn), keep);
            }
            // Each octave's marks where its own are (darker of the two wins).
            float t = abs(tex0 - 1.0) > abs(tex1 - 1.0) ? tex0 : tex1;
            col *= mix(1.0, t, fa * smoothstep(0.3, 0.7, wood));
        }
        float above = h - water;
        bool beachy = b1 != 2 && b1 != 3 && b1 != 5 && b1 != 14;
        if (beachy && above < 6.0 && slope < 0.08) col = mix(col, SAND, 1.0 - smoothstep(3.0, 6.0, above));
    }

    // Cliffs and escarpments (Roadmap 7 items 169-170). Close in, where a riser is many pixels wide,
    // its face is drawn as the battlemap's: matte rock by the way it looks, an inked lip where the
    // grade crosses CLIFF_T going over the rim (convex) with a lit rim under it, and a long shadow
    // past its foot (concave), all from the grade's change per pixel (no extra height taps).
    // Farther out a rock band a pixel or two wide floats as a dark smudge: there the scarp is
    // hatched, short ink strokes down the fall line, bolder where it is steeper, and its hillshade
    // is tempered, so a run tapers off into the slope's shading. Close in the strokes hatch the
    // slopes beside a face.
    float fill = 1.0 - smoothstep(5.0, 14.0, uFtPerPx);
    float cliff = wet ? 0.0 : smoothstep(0.9, 1.3, slope) * fill;
    float dsl = length(dslope);
    vec2 up = g / max(slope, 1e-6);
    // Far out, risers are the only natural ground steeper than ~0.3 (mountainsides stay under it at
    // those levels), and a riser a pixel or two wide changes grade sharply from pixel to pixel; so
    // at every zoom the strokes keep to sharp grade changes (a face's sides), not to an even steep
    // slope (a hill raised by hand, which keeps the plain hillshade).
    float scarp = wet ? 0.0 : smoothstep(mix(0.25, 0.4, fill), mix(0.45, 1.0, fill), slope) * mix(smoothstep(0.08, 0.25, dsl / max(slope, 0.05)), smoothstep(0.02, 0.08, dsl), fill) * (1.0 - cliff);

    // Hillshade, normalized so flat ground is 1.0; light from the north-west. (On a hatched scarp
    // the exaggerated grade is held to a moderate one: the strokes carry it.)
    vec2 ge = g * uExag;
    float gl = length(ge);
    ge *= mix(1.0, min(1.0, 0.35 / max(gl, 1e-6)), smoothstep(0.24, 0.4, slope) * (1.0 - fill));
    vec3 n = normalize(vec3(-ge, 1.0));
    vec3 L = normalize(vec3(-1.0, -1.0, 1.3));
    float lambert = dot(n, L) / L.z;
    float shade = wet ? mix(1.0, lambert, 0.1) : lambert;
    col *= clamp(mix(1.0, shade, 0.85), 0.3, 1.3);

    if (!wet) {
        float facing = dot(-up, vec2(-0.7071));
        float away = smoothstep(0.14, 0.26, -facing);
        vec3 rock = mix(mix(ROCK_MID, ROCK_LIT, smoothstep(0.14, 0.26, facing)), ROCK_SHADE, away);
        // (Screen y runs down the map as the samples' y does.)
        float ds = dot(dslope, up) / max(dsl, 1e-6);
        col = mix(col, rock, cliff * 0.95);
        if (scarp > 0.0) {
            col *= 1.0 - 0.14 * scarp * (0.4 + 0.6 * away);
            // One stroke per 6.4-px cell (a whole number of cells per 512-px grain period, so tiles
            // agree), jittered, along the fall line.
            vec2 gc = (uGrainOrigin + vLocal * uTilePx) / 6.4;
            vec2 cc = mod(floor(gc), 80.0);
            vec2 v = fract(gc) - 0.5 - (vec2(hash21(cc), hash21(cc + 17.0)) - 0.5) * 0.3;
            float along = abs(dot(v, up)), across = abs(dot(v, vec2(-up.y, up.x)));
            float wid = mix(0.05, 0.13, scarp), len = mix(0.2, 0.42, scarp);
            float tick = (1.0 - smoothstep(wid, wid + 0.16, across)) * (1.0 - smoothstep(len, len + 0.16, along));
            col = mix(col, INK, tick * scarp * 0.66);
        }
        if (dsl > 1e-3 && fill > 0.0) {
            float gap = abs(slope - CLIFF_T) / dsl;
            float sharp = smoothstep(0.04, 0.2, dsl) * fill;
            if (ds < 0.0) {
                float over = smoothstep(0.3, 0.7, -ds) * sharp;
                col = mix(col, INK, (1.0 - smoothstep(1.2, 2.2, gap)) * 0.9 * over);
                // The face's lit rim just under the lip.
                if (slope > CLIFF_T) col = mix(col, ROCK_LIT * 1.1, (smoothstep(1.8, 2.6, gap) - smoothstep(4.0, 6.0, gap)) * 0.7 * over);
            } else if (slope < CLIFF_T) {
                // The foot's shadow, deeper and longer below a face turned from the light.
                float reach = mix(6.0, 12.0, away);
                col *= 1.0 - mix(0.3, 0.42, away) * (1.0 - smoothstep(0.0, reach, gap)) * smoothstep(0.3, 0.7, ds) * sharp;
            }
        }
    }

    // Map symbols, blended across two octaves; ecotones dither between the two biomes.
    if (!wet && uPatAlpha > 0.0) {
        float aa0 = max(fwidth(p0.x), 1e-4);
        float aa1 = max(fwidth(p1.x), 1e-4);
        // Far zoom: the symbols are gone (uSymAlpha 0), so none of their work is done (uniform branch);
        // nor where neither biome has a symbol (the woods: no tree symbols on the map).
        float mixin = max(0.0, blend - 0.55) * 0.9;
        if (uSymAlpha > 0.0 && (SYMBOL[b1] != 0 || (mixin > 0.0 && SYMBOL[b2] != 0))) {
            // Symbol lattices turned ~20 deg so rows don't line up with the screen. The matrix entries are
            // multiples of 1/256, so a 256-cell shift of the origin stays a whole-cell shift (no tile seams).
            mat2 turn = mat2(0.9375, -0.34375, 0.34375, 0.9375);
            vec2 r0 = turn * p0;
            vec2 r1 = turn * p1;
            vec2 c0 = mod(floor(r0), 256.0);
            vec2 c1 = mod(floor(r1), 256.0);
            // Ecotones: only strong blends (`mixin`) scatter the neighbouring biome's symbols.
            // Each octave is only evaluated when the crossfade needs it (uniform branch). The crossfade
            // drops whole symbols cell by cell instead of fading them, so neither octave shows as ghosts;
            // the finer octave is drawn over the coarser one.
            float s0 = 0.0;
            float s1 = 0.0;
            if (uPatMix < 0.98) {
                s0 = symbol(r0, SYMBOL[hash21(c0 + 7.0) < mixin ? b2 : b1], aa0);
                s0 *= clamp(((1.0 - uPatMix) * 1.1 - 0.05 - hash21(c0 + 13.1)) * 20.0, 0.0, 1.0);
            }
            if (uPatMix > 0.02) {
                s1 = symbol(r1, SYMBOL[hash21(c1 + 7.0) < mixin ? b2 : b1], aa1);
                s1 *= clamp((uPatMix * 1.1 - 0.05 - hash21(c1 + 13.1)) * 20.0, 0.0, 1.0);
            }
            float ink = max(s0, s1) * uSymAlpha;
            col = mix(col, INK, min(ink * 0.75, 0.9));
        }

        // Hachures: fine strokes down steep slopes (strokes follow the fall line). Where the fall line
        // turns fast (round a small steep hill) the strokes, measured across it from the pattern's
        // origin, crowd into rows of dots: there they fade out (`fa`: their rate per pixel, about
        // 1 to 1.4 times `aa0 * 2.2` on an even slope).
        float steep = smoothstep(0.35, 1.2, slope * uExag);
        vec2 dir = normalize(g + 1e-9);
        float across = dot(p0 * 2.2, vec2(-dir.y, dir.x));
        float fa = fwidth(across);
        if (steep > 0.0) {
            float line = 1.0 - smoothstep(0.0, aa0 * 2.2, abs(fract(across) - 0.5) - 0.34);
            col = mix(col, INK, line * steep * 0.28 * uPatAlpha * (1.0 - cliff) * (1.0 - scarp) * (1.0 - smoothstep(2.5, 4.0, fa / (aa0 * 2.2))));
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
        col = mix(col, CONTOUR_INK, line * room * sloped * mix(0.22, 0.45, isIndex) * (1.0 - 0.8 * cliff));
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
