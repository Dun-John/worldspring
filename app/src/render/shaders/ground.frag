#version 300 es
// Battlemap ground, painterly: mottled surfaces with organic square boundaries, hillshade,
// elevation drawn from the smooth height field (5-ft tier contours; cliff-steep ground painted
// as rock faces with an inked top edge and a cast shadow below), water with depth/ripples/inked
// shore, lava glow, flights of steps (treads square across their streets), and the 5-ft grid.
// Gameplay tiers stay per-square in the data.
//
// uData:    u8 (surface, edge bits, tier & 255, building; on a field its furrows) per square (texelFetch); edge bit 32
//           marks a park's pond or stream and the ground by it.
// uHeights: f16 (h - uBase, water - uBase) per square center, with a one-square halo ring
//           (130 × 130), so gradients at chunk edges use the neighbours' real heights.

in vec2 vUV;
in vec2 vLocal;

out vec4 finalColor;

uniform highp sampler2D uNoise;
uniform highp sampler2D uData;
uniform highp sampler2D uHeights;

uniform float uBase;
uniform float uAlpha;
uniform float uTime;
uniform float uGrid;
uniform float uPxPerSq;
uniform vec2 uNoiseOff;
uniform float uSea;
// Whether the chunk has a park's gravel (else its smoothing is skipped).
uniform float uGravel;
// The cobbles' tint: the settlement's stone (`stoneTone`).
uniform vec3 uStone;
// Whether the chunk has paving (else its soft edges are skipped).
uniform float uPaved;

const float N = 128.0;
const vec3 INK = vec3(0.12, 0.10, 0.08);

const vec3 SURF[21] = vec3[21](
    vec3(0.40, 0.54, 0.25), // grass
    vec3(0.31, 0.35, 0.19), // forest floor
    vec3(0.64, 0.60, 0.36), // dry grass
    vec3(0.85, 0.76, 0.55), // sand
    vec3(0.91, 0.93, 0.96), // snow
    vec3(0.50, 0.49, 0.46), // rock
    vec3(0.33, 0.29, 0.21), // mud
    vec3(0.27, 0.25, 0.24), // ash
    vec3(0.89, 0.87, 0.83), // salt
    vec3(0.52, 0.42, 0.29), // dirt
    vec3(0.30, 0.50, 0.52), // shallow
    vec3(0.15, 0.29, 0.38), // deep
    vec3(1.00, 0.45, 0.10), // lava
    vec3(0.77, 0.87, 0.93), // ice
    vec3(0.66, 0.62, 0.55), // cobble
    vec3(0.60, 0.48, 0.33), // road (packed earth)
    vec3(0.55, 0.40, 0.25), // planks
    vec3(0.58, 0.50, 0.38), // under a building (roofs are drawn as vectors)
    vec3(0.66, 0.62, 0.36), // field
    vec3(0.62, 0.59, 0.53), // stairs (dressed stone)
    vec3(0.76, 0.71, 0.60)  // gravel (a park's paths)
);

// v63 countryside: the field square's furrows under the pixel (`town::fields::square_byte`: angle in
// 64ths of a half turn, mode << 6: 0 arable, 1 open-field strips, 2 fallow), set in main.
float gField = 0.0;


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

float fbm3(vec2 p) {
    return 0.55 * vnoise(p) + 0.3 * vnoise(p * 2.13 + 7.1) + 0.15 * vnoise(p * 4.37 + 3.3);
}

vec4 dataAt(ivec2 s) {
    return texelFetch(uData, clamp(s, ivec2(0), ivec2(int(N) - 1)), 0) * 255.0;
}

float tierDiff(float nb, float own) {
    return mod(nb - own + 128.0, 256.0) - 128.0;
}

vec2 heightsAt(vec2 q) {
    return texture(uHeights, (q + 1.0) / (N + 2.0)).rg + uBase;
}

// Road coverage (0..1) and mean kind (0 earth, 0.5 cobbles, 1 deck), bilinear.
vec2 roadAt(vec2 q) {
    vec2 r = texture(uHeights, (q + 1.0) / (N + 2.0)).ba;
    return vec2(r.x, r.y / max(r.x, 1e-3));
}

// One cobblestone, painted: v its surface's tilt (the way out from its middle, length 0 there to
// about 1 at its rim), hs its hash, joint the joint round it, aa a pixel in v. Three close flat
// tones (lit crescent on the NW, mid, shade crescent on the SE), no grain, each stone one of three
// value steps; the joint a mid-tone a step below the stone, never ink. About a third of the stones
// get a hand-drawn ink stroke on their SE rim, the rest none. Below about 25 px a square the
// crescents are a quarter and the joints a fifth softer, so the 5-ft grid wins over them.
vec3 stoneLook(vec3 col, vec2 v, float hs, float joint, float aa) {
    vec3 st = col * (0.955 + 0.045 * floor(hs * 2.999));
    float side = dot(v, vec2(-0.7071));
    float lit = smoothstep(0.42 - aa, 0.42 + aa, side);
    float shade = smoothstep(0.38 - aa, 0.38 + aa, -side);
    st *= 1.0 + (0.06 * lit - 0.12 * shade) * mix(0.75, 1.0, smoothstep(20.0, 25.0, uPxPerSq));
    joint *= mix(0.8, 1.0, smoothstep(20.0, 28.0, uPxPerSq));
    vec3 c = mix(st, col * 0.86, joint);
    float stroke = step(0.667, fract(hs * 13.7)) * smoothstep(0.2, 0.6, -side);
    return mix(c, INK, joint * stroke * 0.55);
}

// Cobbles (surface 14). Streets: round and oval stones of even size (0.85 to 1.15 times about half a
// square) in loose courses: rows offset by half a stone, gently wobbling, each stone an ellipse of its
// own radius, stretch and turn (near the course's line) round its jittered point (2 x 2 taps); the
// joints are the gaps between the ellipses (wider where three meet). Plazas and market squares (fan):
// segmental arcs ("Bogenpflaster"), fans 3.2 squares across laid in a scale pattern, six rows of
// rounded setts to a fan (their rows a little wider or narrower fan row by fan row, and hand-wobbled),
// short radial joints between the setts of a row; the rows soft tone steps, only every third arc a
// thin mid-dark line. Both lattices' periods divide the noise offset's
// 512 squares, so chunks meet seamlessly. Below a few px a stone the detail fades to the stones'
// mean, as it does by the paving's edge (fade 0 there).
// How much the stones show (0 flat paving), for the grid's weight.
float gStones = 0.0;

vec3 cobbles(vec3 col, vec2 w, bool fan, float fade) {
    vec3 flat_ = col * 0.95;
    float px = (fan ? 0.5333 : 0.5714) * uPxPerSq;
    // (Stones gone by ~5 px, so at town zoom the 5-ft grid reads over the paving.)
    float show = smoothstep(5.0, 10.0, px) * fade;
    gStones = show;
    if (show <= 0.0) return flat_;
    float jw = max(0.5, 0.035 * px);
    vec3 stone;
    if (fan) {
        // Fans' middles in rows half a fan apart, every other row offset by half; the point is in
        // the nearest fan of the row below it if that reaches it, else of its own row.
        vec2 p = w * 0.3125;
        float row = floor(p.y * 2.0) + 1.0;
        float off = 0.5 * mod(row, 2.0);
        vec2 c = vec2(floor(p.x - off + 0.5) + off, row * 0.5);
        // (The fans' round edges wobble by hand, a fiftieth of a fan.)
        float wob = vnoise(w * 1.3 + 7.0) - 0.5;
        float rim = 0.5 + 0.04 * wob;
        if (dot(p - c, p - c) > rim * rim) {
            row -= 1.0;
            off = 0.5 - off;
            c = vec2(floor(p.x - off + 0.5) + off, row * 0.5);
        }
        // The arcs are rings round the tip of the fan's visible part (half a fan below its middle,
        // where the two fans in front of it meet), so its round edge is an arc and its rings meet the
        // fans in front at their edges.
        vec2 o = p - c - vec2(0.0, 0.5);
        float len = length(o) + 1e-5;
        vec2 n = o / len;
        float rr = len * 6.0 * (0.9 + 0.2 * hash21(vec2(mod(row, 320.0), 5.0))) + wob * 0.5;
        float k = floor(rr);
        float fr = rr - k;
        float hf = hash21(mod(c * 2.0, 320.0));
        // Along the arc, in setts half again as long as a row is wide, offset row by row.
        float t = atan(o.x, -o.y) * (k + 0.5) / 1.5 + fract(hf * 7.0 + k * 0.37) * 2.0;
        float ft = fract(t);
        // A sett: joints only across the row (radial, softer under 25 px a square, so no fine
        // crosshatch with the grid); the rows part by a soft tone step, every third arc a thin
        // mid-dark line.
        float joint = (1.0 - smoothstep(jw - 0.5, jw + 0.5, min(ft, 1.0 - ft) * 1.5 * px)) * mix(0.6, 1.0, smoothstep(20.0, 28.0, uPxPerSq));
        vec2 v = vec2(-n.y, n.x) * (ft - 0.5) * 2.0 + n * (fr - 0.5) * 0.9;
        vec3 rowCol = col * (1.0 + 0.045 * (mod(k, 2.0) - 0.5));
        stone = stoneLook(rowCol, v, hash21(vec2(floor(t) + 31.0 * k, hf * 97.0)), joint, 2.0 / px);
        float ka = floor(rr + 0.5);
        float arc = step(2.5, mod(ka + 300.0, 3.0)) * (1.0 - smoothstep(0.3, 0.9, abs(rr - ka) * px));
        // The fan's round edge one of the lines, a little bolder.
        float edge = (rim - distance(p, c)) * 6.0 * px;
        arc = max(arc, 1.0 - smoothstep(jw, jw + 1.0, edge));
        stone = mix(stone, col * 0.72, arc * 0.85);
    } else {
        vec2 p = w * 1.75;
        float rb = floor(p.y - 0.5);
        float s1 = -9.0;
        float s2 = -9.0;
        vec2 v1 = vec2(0.0);
        float h1 = 0.0;
        for (int k = 0; k < 4; k++) {
            // Course r (every other one offset by half a stone), its two stones nearest the point.
            float r = rb + float(k >> 1);
            float off = 0.5 * mod(r, 2.0);
            vec2 c = vec2(floor(p.x - off - 0.5) + float(k & 1), r);
            float h = hash21(mod(c, 896.0));
            // (The course's wobble: a wave of 512 / 170 squares, so chunks still meet.)
            float cx = c.x + 0.5 + off;
            float wy = 0.12 * sin(cx * (2.0862 / 1.75) + 6.2832 * hash21(vec2(mod(r, 896.0), 3.0)));
            vec2 o = p - vec2(cx, r + 0.5 + wy) - (vec2(h, fract(h * 37.1)) - 0.5) * vec2(0.24, 0.1);
            // Its own turn (within 20 degrees of the course), stretch (oval up to 1.15 : 1) and
            // radius (0.43 to 0.58 a cell).
            float ang = (fract(h * 41.0) - 0.5) * 0.7;
            vec2 ax = vec2(cos(ang), sin(ang));
            float isx = 1.0 / (1.0 + 0.15 * fract(h * 7.3));
            vec2 m = vec2(dot(o, ax) * isx, dot(o, vec2(-ax.y, ax.x)));
            float rad = 0.5 * (0.85 + 0.3 * fract(h * 3.1));
            // How far inside its rim (negative outside it).
            float s = rad - length(m);
            if (s > s1) {
                s2 = s1;
                s1 = s;
                // The dome's tilt: the ellipse's own gradient, its length the way to the rim.
                v1 = (ax * (m.x * isx) + vec2(-ax.y, ax.x) * m.y) / rad;
                h1 = h;
            } else {
                s2 = max(s2, s);
            }
        }
        // Inside its own ellipse and off the joint with an overlapping neighbour, the corner between
        // the two rounded off.
        float a2 = 0.5 * (s1 - s2);
        float hk = max(0.12 - abs(s1 - a2), 0.0) / 0.12;
        float gap = min(s1, a2) - hk * hk * 0.03;
        float joint = 1.0 - smoothstep(jw - 0.5, jw + 0.5, gap * px);
        stone = stoneLook(col, v1, fract(h1 * 19.7), joint, 2.5 / px);
    }
    return mix(flat_, stone, show);
}

// Every surface but the cobbles.
vec3 groundColor(int s, vec2 w) {
    vec3 base = SURF[s];
    float m = fbm3(w * 0.45);
    float fine = vnoise(w * 3.1);
    vec3 col = base * (0.82 + 0.3 * m) * (0.94 + 0.12 * fine);
    if (s == 0 || s == 2) {
        // Grass: tufts of lighter and darker blades.
        float blades = vnoise(w * 7.0 + vec2(0.0, w.x * 0.3));
        col = mix(col, col * vec3(1.15, 1.18, 1.05), smoothstep(0.65, 0.9, blades) * 0.6);
        col = mix(col, col * 0.8, smoothstep(0.7, 0.95, vnoise(w * 5.3 + 11.0)) * 0.5);
    } else if (s == 1) {
        // Leaf litter.
        float litter = vnoise(w * 6.0);
        col = mix(col, vec3(0.46, 0.36, 0.20), smoothstep(0.7, 0.9, litter) * 0.6);
    } else if (s == 3) {
        // Sand ripples.
        col *= 0.95 + 0.07 * sin(w.x * 2.6 + w.y * 0.9 + 4.0 * vnoise(w * 0.4));
    } else if (s == 5) {
        // Rock: cracks.
        float c = abs(vnoise(w * 1.7) - 0.5);
        col = mix(col, col * 0.6, 1.0 - smoothstep(0.0, 0.035, c));
    } else if (s == 15) {
        // Packed earth with two worn cart ruts along the dominant direction of the grain.
        float rut = vnoise(w * vec2(0.8, 4.0)) * vnoise(w * vec2(4.0, 0.8));
        col = mix(col, col * 0.78, smoothstep(0.35, 0.6, rut) * 0.6);
        col = mix(col, vec3(0.46, 0.44, 0.40), smoothstep(0.82, 0.95, vnoise(w * 6.0)) * 0.5);
    } else if (s == 16) {
        // Deck planks with dark seams and nail heads.
        float p = w.y * 2.5;
        float seam = 1.0 - smoothstep(0.0, 0.08, min(fract(p), 1.0 - fract(p)));
        col *= 0.85 + 0.25 * hash21(vec2(floor(p), floor(w.x * 0.4 + hash21(vec2(floor(p), 3.0)) * 7.0)));
        col = mix(col, INK, seam * 0.7);
    } else if (s == 19) {
        // Dressed stone: coarse slabs with faint joints.
        vec2 c = w * vec2(1.1, 1.6);
        c.x += 0.5 * mod(floor(c.y), 2.0);
        vec2 g = fract(c) - 0.5;
        float slab = smoothstep(0.5, 0.44, max(abs(g.x), abs(g.y)));
        col = mix(col * 0.8, col * (0.94 + 0.12 * hash21(floor(c))), slab);
    } else if (s == 20) {
        // Gravel: fine pebbles, light and dark, a little raked.
        float peb = hash21(floor(w * 9.0));
        col = mix(col, col * (0.86 + 0.28 * peb), 0.55);
        col = mix(col, col * 0.84, smoothstep(0.6, 0.85, vnoise(w * 1.6)) * 0.35);
    } else if (s == 18) {
        // Crop rows along the field's furrows (v63 countryside); an open field's strips each a shade
        // of their own with a grass balk between; fallow rows faint, the ground browner.
        int fb = int(gField + 0.5);
        int mode = fb >> 6;
        float ang = float(fb & 63) * (3.14159265 / 64.0);
        float across = dot(w, vec2(-sin(ang), cos(ang)));
        // (Two flat tones, ridge and furrow, a narrow soft step between: painted, not noise.)
        float row = abs(fract(across * 0.9 + 0.2 * vnoise(w * 0.3)) - 0.5);
        col = mix(col * (mode == 2 ? 0.9 : 0.8), col * 1.1, smoothstep(0.17, 0.27, row));
        if (mode == 1) {
            float st = across / 7.6;
            col *= 0.93 + 0.14 * hash21(vec2(floor(st), 5.1));
            float balk = 1.0 - smoothstep(0.03, 0.07, min(fract(st), 1.0 - fract(st)));
            col = mix(col, SURF[0] * 1.05, balk * 0.75);
        } else if (mode == 2) {
            col = mix(col, vec3(0.56, 0.46, 0.33), 0.38);
        }
    } else if (s == 8) {
        // Salt crust polygons.
        float c = abs(vnoise(w * 2.2) - vnoise(w * 2.2 + 5.0));
        col = mix(col, col * 0.82, 1.0 - smoothstep(0.0, 0.05, c));
    }
    return col;
}

vec3 surfaceColor(int s, vec2 w, bool fan, float fade) {
    // Painted, not photo: only a faint wash of the ground's mottling, the city's tone.
    if (s == 14) return cobbles(SURF[14] * (0.94 + 0.1 * fbm3(w * 0.45)) * uStone, w, fan, fade);
    return groundColor(s, w);
}

// Flights of steps crossing the chunk: per axis (bottom, top) and (half width, start up the
// flight, tread depth, 0), in squares.
uniform vec4 uStairs[128];
uniform float uStairN;

// A flight of steps: treads square across the nearest flight's axis (a bold nosing line on each,
// lit at its front edge and shaded towards the riser above), each tread laid in a row of stone
// blocks. Fades out where the treads crowd on screen and where the ground is a cliff.
vec3 treads(vec3 col, vec2 q, float steep) {
    float best = 1e9;
    vec4 axis = vec4(0.0);
    vec4 more = vec4(0.0);
    for (int i = 0; i < 64; i++) {
        if (float(i) >= uStairN) break;
        vec4 a = uStairs[2 * i];
        vec4 m = uStairs[2 * i + 1];
        vec2 ba = a.zw - a.xy;
        float len = max(length(ba), 1e-4);
        vec2 u = ba / len;
        vec2 pa = q - a.xy;
        float t = dot(pa, u);
        // Outside the axis's ends or its width (a little slack for the warped squares).
        float out_ = max(max(-t, t - len), abs(pa.x * u.y - pa.y * u.x) - m.x);
        if (out_ < best) {
            best = out_;
            axis = a;
            more = m;
        }
    }
    if (best > 0.6) return col;
    vec2 u = normalize(axis.zw - axis.xy);
    vec2 pa = q - axis.xy;
    float x = (more.y + dot(pa, u)) / more.z;
    float fr = fract(x);
    // Squares per tread on screen: fade the treads out below a few pixels.
    float px = more.z * uPxPerSq;
    float show = smoothstep(2.5, 5.0, px) * (1.0 - steep);
    // Blocks across the tread, offset tread by tread.
    float across = (pa.x * u.y - pa.y * u.x) / 0.55 + hash21(vec2(floor(x), 7.0)) * 3.0;
    float joint = min(fract(across), 1.0 - fract(across)) * 0.55 * uPxPerSq;
    col *= 0.92 + 0.14 * hash21(vec2(floor(x), floor(across)));
    col = mix(col, col * 0.78, (1.0 - smoothstep(0.3, 1.0, joint)) * 0.5 * show);
    // Lit at the front (downhill) edge, in the shadow of the riser above at the back.
    col = mix(col, col * 1.15, (1.0 - smoothstep(0.0, 0.25, fr)) * 0.7 * show);
    col = mix(col, col * 0.6, smoothstep(0.55, 1.0, fr) * 0.8 * show);
    // A bold nosing: about a twentieth of the tread each side, 1-3 px.
    float d = min(fr, 1.0 - fr) * px;
    float hw = clamp(0.05 * px, 0.9, 3.0);
    float nosing = 1.0 - smoothstep(hw - 0.5, hw + 0.5, d);
    return mix(col, INK, nosing * 0.85 * show);
}

void main() {
    vec2 q = vLocal * N;
    vec2 w = q + uNoiseOff;
    float aa = 1.0 / max(uPxPerSq, 1.0);

    // Organic boundaries between surface types: warp the square lookup.
    vec2 warp = vec2(vnoise(w * 0.9), vnoise(w * 0.9 + 17.0)) - 0.5;
    ivec2 sw = ivec2(floor(q + warp * 0.7));
    vec4 dw = dataAt(sw);
    int surf = int(dw.r + 0.5);
    // A plaza or market square (bit 64): its cobbles laid in fans.
    bool plaza = (int(dw.g + 0.5) & 64) != 0;

    ivec2 sq = ivec2(floor(q));
    // Roads: smooth edges from filtered coverage instead of square steps.
    vec2 road = roadAt(q);
    float cov = road.x + (vnoise(w * 2.3) - 0.5) * 0.12;
    bool onRoad = cov > 0.5;
    // A cobbled road across a plaza is laid in its fans, no verge: no seams across the square.
    bool roadInPlaza = plaza && road.y >= 0.25 && road.y < 0.75;
    if (onRoad) {
        surf = road.y < 0.25 ? 15 : (road.y < 0.75 ? 14 : 16);
    } else if (((surf >= 14 && surf <= 16 || surf == 19) && road.x > 0.01) || (surf == 17 && dataAt(sq).a < 0.5)) {
        // Beside a road or building: the ground of a neighbouring square that is neither
        // (town streets and plazas away from roads keep their paving).
        int natural = 9;
        bool paved = false;
        for (int dy = -1; dy <= 1; dy++) {
            for (int dx = -1; dx <= 1; dx++) {
                int s = int(dataAt(sq + ivec2(dx, dy)).r + 0.5);
                if ((s < 14 || s == 18) && natural == 9) natural = s;
                paved = paved || s == 14 || s == 19;
            }
        }
        // In town the ground beside a road is the paved street, not dirt.
        surf = natural == 9 && paved ? 14 : natural;
    }
    // A park's gravel paths: smooth edges from the squares' bilinear coverage (as a road's), not
    // their steps; beside one, the ground of a neighbouring square that isn't gravel.
    float gravel = -1.0;
    if (!onRoad && uGravel > 0.5) {
        vec2 g = q - 0.5 + warp * 0.3;
        ivec2 gi = ivec2(floor(g));
        int s00 = int(dataAt(gi).r + 0.5);
        int s10 = int(dataAt(gi + ivec2(1, 0)).r + 0.5);
        int s01 = int(dataAt(gi + ivec2(0, 1)).r + 0.5);
        int s11 = int(dataAt(gi + ivec2(1, 1)).r + 0.5);
        vec4 on = vec4(s00 == 20, s10 == 20, s01 == 20, s11 == 20);
        if (dot(on, vec4(1.0)) > 0.0) {
            // (A tent 1.5 squares round over the 4 x 4 squares about the point: a slanting path's
            // steps smooth out.)
            float sum = 0.0;
            float wsum = 0.0;
            for (int j = -1; j <= 2; j++) {
                for (int i = -1; i <= 2; i++) {
                    vec2 dd = g - (vec2(gi + ivec2(i, j)) + 0.5) + 0.5;
                    float wt = max(0.0, 1.5 - abs(dd.x)) * max(0.0, 1.5 - abs(dd.y));
                    sum += wt * float(int(dataAt(gi + ivec2(i, j)).r + 0.5) == 20);
                    wsum += wt;
                }
            }
            gravel = sum / max(wsum, 1e-3) + (vnoise(w * 2.7) - 0.5) * 0.14;
            if (gravel > 0.5) {
                surf = 20;
            } else if (surf == 20) {
                surf = s00 != 20 ? s00 : (s10 != 20 ? s10 : (s01 != 20 ? s01 : s11));
            }
        }
    }
    // Paving's edges with the ground: soft and hand-wobbled from the 2 x 2 squares' bilinear
    // paving (as a gravel path's), not the warped lookup's square steps; between the paving and
    // a natural ground only, every other boundary as it was.
    // A field's edge with the ground round it (v63 countryside) the same way: crisp, a slight wobble
    // (the warp's), a rim of plain darker earth inside it (`frim`) for a headland.
    // (Only on squares by the paving's or a field's edge, bit 128: everywhere else this changes
    // nothing.)
    float pave = -1.0;
    float frim = 0.0;
    float fieldA = -1.0;
    int nat = 0;
    vec4 d = dataAt(sq);
    if (!onRoad && uPaved > 0.5 && (int(d.g + 0.5) & 128) != 0 && gravel < 0.0 && (surf <= 9 || surf == 13 || surf == 14 || surf == 18)) {
        vec2 g = q - 0.5 + warp * 0.3;
        ivec2 gi = ivec2(floor(g));
        vec2 fg = g - floor(g);
        float sp = 0.0;
        float sn = 0.0;
        float best = 0.0;
        float sf = 0.0;
        float so = 0.0;
        float fbest = 0.0;
        float fa = 0.0;
        int oth = -1;
        nat = -1;
        bool fanHere = plaza;
        for (int k = 0; k < 4; k++) {
            vec4 dk = dataAt(gi + ivec2(k & 1, k >> 1));
            int s = int(dk.r + 0.5);
            float wt = ((k & 1) == 1 ? fg.x : 1.0 - fg.x) * ((k >> 1) == 1 ? fg.y : 1.0 - fg.y) + 1e-4;
            if (s == 14) {
                sp += wt;
                if (wt > best) {
                    best = wt;
                    fanHere = (int(dk.g + 0.5) & 64) != 0;
                }
            } else if (s <= 9 || s == 13 || s == 18) {
                sn += wt;
                if (nat < 0) nat = s;
                if (s == 18) {
                    sf += wt;
                    if (wt > fbest) {
                        fbest = wt;
                        fa = dk.a;
                    }
                } else {
                    so += wt;
                    if (oth < 0) oth = s;
                }
            }
        }
        if (sp > 0.0 && sn > 0.0) {
            pave = sp / (sp + sn) + (vnoise(w * 2.6 + 41.0) - 0.5) * 0.3;
            surf = pave > 0.5 ? 14 : nat;
            plaza = fanHere;
        } else if (sp > 0.0 && surf != 14) {
            // (Where the warped lookup reaches past the squares round the point, the same rule.)
            surf = 14;
            plaza = fanHere;
        } else if (sn > 0.0 && surf == 14) {
            surf = nat;
        } else if (sp == 0.0 && sf > 0.0 && so > 0.0) {
            float fc = sf / (sf + so) + warp.y * 0.14;
            surf = fc > 0.5 ? 18 : oth;
            fieldA = fa;
            frim = fc > 0.5 ? 1.0 - smoothstep(0.66, 0.8, fc) : 0.0;
        }
    }
    vec2 f = fract(q);
    gField = fieldA >= 0.0 ? fieldA : (int(dw.r + 0.5) == 18 ? dw.a : d.a);
    float own = d.b;
    vec2 hw = heightsAt(q);
    float h = hw.x;
    float depth = hw.y - h;
    // By a park's pond or stream (edge bit 32) the ground follows the water's edge, not the squares:
    // dry ground the warped lookup gave the water's bed takes the land's of its own square or one
    // round it (not a path's gravel, drawn smooth above); water it gave land takes the bed.
    if ((int(d.g + 0.5) & 32) != 0) {
        bool bed = surf == 10 || surf == 11;
        if (depth <= 0.0 && bed) {
            int land = int(d.r + 0.5);
            for (int k = 0; k < 9 && (land == 10 || land == 11 || land == 20); k++) {
                land = int(dataAt(sq + ivec2(k % 3 - 1, k / 3 - 1)).r + 0.5);
            }
            surf = land == 10 || land == 11 || land == 20 ? 0 : land;
        } else if (depth > 0.0 && !bed) {
            surf = 10;
        }
    }

    // (By the paving's edge its outer stones fade to their mean.)
    float band = pave >= 0.0 && surf == 14 ? 1.0 - smoothstep(0.5, 0.8, pave) : 0.0;
    bool fanned = surf == 14 && plaza && (!onRoad || roadInPlaza);
    vec3 col = surfaceColor(surf, w, fanned, 1.0 - band);
    if (frim > 0.0) col = mix(col, SURF[18] * vec3(0.74, 0.68, 0.62), frim * 0.85);
    if (band > 0.0) {
        // The ground laps over the edge: soft (a pixel or so, never a stair), a few tufts of it
        // over the outer stones, a faint painted shade along the line.
        // (Crisp blobs, fewer further in, not a half-mixed fringe.)
        float tuft = smoothstep(0.0, 0.06, vnoise(w * 4.3 + 3.0) - 0.88 + 0.4 * band);
        float soft = 1.0 - smoothstep(0.5, 0.5 + max(1.5 * aa, 0.02), pave);
        col = mix(col, col * 0.88, band * band * (1.0 - tuft));
        col = mix(col, groundColor(nat, w), max(soft, tuft));
    } else if (pave >= 0.0 && surf == nat) {
        // The ground a shade darker along the paving's edge, a rim (as a sprite's) so the street's
        // outline reads from afar.
        col *= 1.0 - 0.09 * smoothstep(0.2, 0.5, pave);
    }
    // Worn verge: the road edge darkens slightly into the surrounding ground (a gravel path's
    // edge, a little).
    float verge = roadInPlaza ? 0.0 : smoothstep(0.3, 0.5, cov) * (1.0 - smoothstep(0.5, 0.62, cov));
    col = mix(col, col * 0.72, verge * 0.6);
    if (gravel > 0.0) col = mix(col, col * 0.8, smoothstep(0.3, 0.5, gravel) * (1.0 - smoothstep(0.5, 0.64, gravel)) * 0.7);

    // Hillshade from square heights (NW light).
    float hx = heightsAt(q + vec2(0.5, 0.0)).x - heightsAt(q - vec2(0.5, 0.0)).x;
    float hy = heightsAt(q + vec2(0.0, 0.5)).x - heightsAt(q - vec2(0.0, 0.5)).x;
    // (Steps are level treads: their slope shades less.)
    float relief = surf == 19 ? 0.5 : 1.6;
    vec3 n = normalize(vec3(-hx / 5.0 * relief, -hy / 5.0 * relief, 1.0));
    vec3 L = normalize(vec3(-1.0, -1.0, 1.4));
    // v63 underground: small closed relief (data bit 8, the edges' 128: `battlemap::soft_relief`,
    // `battlePrep`) is shaded soft: a narrower range, which gentle ground stays inside.
    bool softRelief = (int(d.g + 0.5) & 8) != 0;
    col *= clamp(0.35 + 0.65 * dot(n, L) / L.z, softRelief ? 0.8 : 0.45, softRelief ? 1.12 : 1.25);

    // Water.
    if (depth > 0.0) {
        float t = clamp(depth / 8.0, 0.0, 1.0);
        vec3 water = mix(vec3(0.33, 0.56, 0.58), vec3(0.13, 0.27, 0.36), sqrt(t));
        float rip = vnoise(w * 1.3 + vec2(uTime * 0.25, uTime * 0.15)) * vnoise(w * 2.1 - vec2(uTime * 0.2, 0.0));
        water += vec3(0.10, 0.12, 0.12) * smoothstep(0.35, 0.55, rip);
        col = mix(col, water, smoothstep(0.0, 0.6, depth) * 0.92 + 0.08);
    }
    float fwh = max(fwidth(h), 1e-3);
    float shore = 1.0 - smoothstep(0.0, 1.5, abs(depth) / fwh);
    col = mix(col, INK, shore * 0.8 * step(-3.0, depth));

    if (surf == 12) {
        float glow = fbm3(w * 0.8 + vec2(0.0, uTime * 0.3));
        col = mix(vec3(0.95, 0.30, 0.05), vec3(1.0, 0.85, 0.3), glow);
    }

    // Elevation from the smooth height field. Slope in ft per ft; gradient points uphill.
    vec2 grad = vec2(hx, hy) / 5.0;
    float slope = length(grad);
    // Above a grade of 1.5 the 5-ft lines would crowd into a smear of ink (a 40-ft step in one
    // square is 8 of them): only every n-th is drawn, n doubling with the grade, so a cliff reads
    // as rock with a bold line or two.
    float stride = exp2(max(0.0, ceil(log2(slope / 1.5))));
    float t = (h - uSea) / (5.0 * stride);
    float fr = fract(t);
    // Horizontal distance (squares) to the nearest line drawn.
    float distSq = min(fr, 1.0 - fr) * 5.0 * stride / max(slope, 0.02) / 5.0;
    // Steps climb steeper ground before it is a cliff.
    float steep = surf == 19 ? smoothstep(1.0, 1.4, slope) : smoothstep(0.45, 0.8, slope);
    // (Soft relief: a faint face, and the gentle slopes' dashed contour.)
    float face = softRelief ? 0.25 * steep : steep;
    if (softRelief) steep = 0.0;
    if (depth <= 0.0) {
        if (face > 0.0) {
            // Cliff face: rock with striations running down the fall line.
            vec2 dir = normalize(grad + 1e-6);
            float stri = vnoise(vec2(dot(w, vec2(-dir.y, dir.x)) * 3.0, 0.5));
            vec3 rock = vec3(0.44, 0.41, 0.37) * (0.7 + 0.5 * stri);
            // Faces turned away from the NW light are in shadow (dark grey rock, not black: a
            // sheer step then reads as a rock face all round).
            float lit = clamp(0.55 + 0.6 * dot(-dir, normalize(vec2(-1.0, -1.0))), 0.6, 1.15);
            col = mix(col, rock * lit, face * 0.9);
        }
        // Tier contour: bold ink on cliffs, thin dashed line on gentle slopes (on steps only the
        // cliff's: a contour would cut across the treads; on a plaza too, where a graded square's
        // straight contours read as seams across its fans).
        float lineW = mix(0.035, 0.07, steep);
        float line = 1.0 - smoothstep(lineW, lineW + aa * 1.3, distSq);
        float dash = mix(step(0.35, fract((w.x + w.y) * 1.5)), 1.0, steep);
        col = mix(col, INK, line * dash * mix(0.4, 0.85, steep) * (surf == 19 || fanned ? steep : 1.0));
        if (surf == 19) col = treads(col, q, steep);
    }

    // 5-ft grid.
    if (uGrid > 0.0) {
        vec2 g = min(f, 1.0 - f) * uPxPerSq;
        float line = 1.0 - smoothstep(0.4, 1.2, min(g.x, g.y));
        // Bolder over paving, so it reads over the stones' joints (cobbles: as far as their stones
        // show, flat paving as grass, so faded streets don't look tiled).
        bool hard = surf == 16 || surf == 19 || surf == 20;
        float wt = hard ? 0.45 : (surf == 14 ? mix(0.28, 0.45, gStones) : 0.28);
        col = mix(col, INK, line * wt * uGrid);
    }

    finalColor = vec4(col * uAlpha, uAlpha);
}
