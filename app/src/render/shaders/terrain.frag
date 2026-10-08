#version 300 es
// Terrain tile, ink-and-parchment cartographic style:
// biome tint (with ecotone blending) × zoom-adaptive NW hillshade, water with depth and coast
// ripple lines, beaches, map symbols (forest canopy, conifers, marsh, sand, tufts, scree),
// slope hachures, ink contours and coastline, paper grain.
//
// uTex:  f16 (h - uBase, dh/dx, dh/dy, water - uBase) per sample.
// uBio:  u8 (primary biome, secondary biome, blend, coast distance sqrt-encoded) per sample.
// Symbol patterns live in two world-anchored octaves (uPat0/uPat1: origin mod 256 in cells,
// cells per tile) blended by uPatMix, so they stay put while panning and zooming.

in vec2 vUV;
in vec2 vLocal;

out vec4 finalColor;

uniform sampler2D uTex;
uniform sampler2D uBio;

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

const vec3 INK = vec3(0.20, 0.18, 0.16);
const vec3 CONTOUR_INK = vec3(0.42, 0.33, 0.24);
const vec3 SAND = vec3(0.90, 0.85, 0.68);
const float SAMPLES = 257.0;
const float COAST_ENCODE_FT = 250000.0;

const vec3 PALETTE[19] = vec3[19](
    vec3(0.54, 0.63, 0.66), // ocean
    vec3(0.62, 0.72, 0.74), // lake
    vec3(0.95, 0.95, 0.93), // ice
    vec3(0.80, 0.79, 0.70), // tundra
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

// Symbol per biome: 0 none, 1 canopy, 2 conifer, 3 dense canopy, 4 marsh, 5 sand, 6 sparse dots, 7 tufts, 8 scree.
const int SYMBOL[19] = int[19](0, 0, 0, 6, 8, 2, 1, 1, 7, 7, 5, 5, 6, 3, 4, 5, 0, 2, 8);

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

// Map symbol at pattern coordinate p (cells). Returns (fill darkening, ink coverage).
vec2 symbol(vec2 p, int kind, float aa) {
    vec2 c = mod(floor(p), 256.0);
    vec2 f = fract(p);
    vec2 h = hash22(c);
    vec2 o = 0.5 + (h - 0.5) * 0.45;
    vec2 d = f - o;
    if (kind == 1 || kind == 3) {
        if (kind == 1 && h.x > 0.75) return vec2(0.0);
        float r = kind == 3 ? 0.40 + 0.06 * h.y : 0.30 + 0.08 * h.y;
        float dist = length(d);
        float fill = 1.0 - smoothstep(r - aa, r, dist);
        float ink = 1.0 - smoothstep(0.0, aa * 1.2, abs(dist - r));
        return vec2(fill, ink);
    }
    if (kind == 2) {
        if (h.x > 0.8) return vec2(0.0);
        // Isosceles triangle, apex up, like a conifer.
        vec2 q = vec2(abs(d.x), d.y + 0.05);
        float w = 0.22, top = -0.30, bot = 0.22;
        float t = (q.y - top) / (bot - top);
        float edge = q.x - w * t;
        float inside = step(top, q.y) * step(q.y, bot);
        float fill = inside * (1.0 - smoothstep(-aa, 0.0, edge));
        float ink = max(inside * (1.0 - smoothstep(0.0, aa * 1.2, abs(edge))), (1.0 - smoothstep(0.0, aa, abs(q.y - bot))) * step(q.x, w));
        float trunk = 1.0 - smoothstep(0.0, aa, segment(d, vec2(0.0, bot - 0.05), vec2(0.0, bot + 0.1)));
        return vec2(fill, max(ink, trunk));
    }
    if (kind == 4) {
        if (h.x > 0.6) return vec2(0.0);
        float l = segment(f, vec2(0.25, o.y), vec2(0.75, o.y));
        l = min(l, segment(f, vec2(0.35, o.y + 0.16), vec2(0.65, o.y + 0.16)));
        return vec2(0.0, 1.0 - smoothstep(0.0, aa, l));
    }
    if (kind == 5 || kind == 6) {
        float p0 = kind == 5 ? 0.35 : 0.12;
        if (h.x > p0) return vec2(0.0);
        float r = kind == 5 ? 0.06 : 0.09;
        return vec2(0.0, 1.0 - smoothstep(r - aa, r, length(d)));
    }
    if (kind == 7) {
        if (h.x > 0.06) return vec2(0.0);
        float l = min(segment(d, vec2(-0.1, 0.08), vec2(-0.04, -0.08)), segment(d, vec2(0.0, 0.08), vec2(0.0, -0.1)));
        l = min(l, segment(d, vec2(0.1, 0.08), vec2(0.04, -0.08)));
        return vec2(0.0, 1.0 - smoothstep(0.0, aa, l));
    }
    if (kind == 8) {
        if (h.x > 0.45) return vec2(0.0);
        float a = h.y * 6.2831;
        vec2 dir = vec2(cos(a), sin(a)) * 0.12;
        return vec2(0.0, 1.0 - smoothstep(0.0, aa, segment(d, -dir, dir)));
    }
    return vec2(0.0);
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
    if (wet) {
        float d = clamp(depth / 6000.0, 0.0, 1.0);
        col = mix(vec3(0.70, 0.78, 0.77), vec3(0.52, 0.62, 0.66), sqrt(d));
    } else {
        col = mix(PALETTE[b1], PALETTE[b2], blend * 0.5);
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
        // Ecotones: only strong blends scatter the neighbouring biome's symbols.
        float mixin = max(0.0, blend - 0.55) * 0.9;
        // Each octave is only evaluated when the crossfade needs it (uniform branch).
        vec2 s0 = vec2(0.0);
        vec2 s1 = vec2(0.0);
        if (uPatMix < 0.98) s0 = symbol(p0, SYMBOL[hash21(mod(floor(p0), 256.0) + 7.0) < mixin ? b2 : b1], aa0);
        if (uPatMix > 0.02) s1 = symbol(p1, SYMBOL[hash21(mod(floor(p1), 256.0) + 7.0) < mixin ? b2 : b1], aa1);
        vec2 s = mix(s0, s1, uPatMix) * uPatAlpha;
        col = mix(col, col * 0.80, s.x);
        col = mix(col, INK, s.y * 0.75);

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
    if (!wet && uContour > 0.0) {
        float f = (h - uSea) / uContour;
        float fw = fwidth(f);
        float d = abs(fract(f + 0.5) - 0.5);
        float isIndex = mod(floor(f + 0.5), uIndexEvery) < 0.5 ? 1.0 : 0.0;
        float w = mix(0.5, 0.9, isIndex);
        float line = 1.0 - smoothstep(w * fw, (w + 1.0) * fw, d);
        float room = 1.0 - smoothstep(0.12, 0.3, fw);
        col = mix(col, CONTOUR_INK, line * room * mix(0.22, 0.45, isIndex));
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
