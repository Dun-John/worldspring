#version 300 es
// Straight-alpha vertex colour, premultiplied, times the container's tint and alpha. Roof faces
// (code >= 0: material + 16 × weathering 0-15, `battlePrep` MeshBuilder) are patterned in roof-local
// feet: u along the courses (stored × d, divided back so a cone's or dome's run along the rays to its
// apex), v up the slope from the eave. Painted cartoon: the vertex colour's matte shade (lit NW slope
// lighter) times three close flat tones stepped by course or patch (no tone per piece), big pieces drawn
// with sparse lines in a darker tone of the material (not ink), detail fading to flat courses, then the
// flat colour, before it can shimmer. Lines (code -2 - half width, `colormesh.vert`) are antialiased
// across: vRoof.x the distance from the middle, y the half width (screen px).
precision highp float;

in vec4 vColor;
in vec3 vRoof;
flat in float vCode;

out vec4 finalColor;

uniform vec4 uColor;
uniform float uPxPerSq;

float hash(vec2 p) {
    vec3 q = fract(vec3(p.xyx) * 0.1031);
    q += dot(q, q.yzx + 33.33);
    return fract((q.x + q.y) * q.z);
}

// Coverage of a line w ft wide at distance d ft from its middle, antialiased over a pixel (px ft).
float ln(float d, float w, float px) {
    return 1.0 - smoothstep(0.5 * (w - px), 0.5 * (w + px), d);
}

// A step from 0 to 1 at e, antialiased over w (a pixel in x's units).
float stp(float e, float x, float w) {
    return smoothstep(e - 0.5 * w, e + 0.5 * w, x);
}

// Three close flat tones from a course's or patch's hash.
float tone3(float h) {
    return h < 0.34 ? 0.94 : h < 0.67 ? 1.0 : 1.055;
}

// Row n's pieces across: about w ft wide (jit: up to that fraction wider or narrower), offset half a piece
// from the row below, plus up to jit of a piece.
float rowX(float u, float n, float w, float jit) {
    return u / (w * (1.0 + jit * (hash(vec2(n, 3.7)) - 0.5))) + 0.5 * mod(n, 2.0) + jit * hash(vec2(n, 8.1));
}

// Rows of rounded scallops (`rowX`) h ft tall: row n's lower edge hangs drop ft at a piece's middle.
// Returns (row, x in pieces, ft up from its own edge, ft down to the next row's edge).
vec4 scallops(float u, float v, float w, float h, float drop, float jit) {
    float n = floor(v / h);
    float eu = (n + 1.0) * h - drop * sin(3.14159 * fract(rowX(u, n + 1.0, w, jit)));
    float x;
    if (v >= eu) {
        n += 1.0;
        x = rowX(u, n, w, jit);
        eu = (n + 1.0) * h - drop * sin(3.14159 * fract(rowX(u, n + 1.0, w, jit)));
    } else x = rowX(u, n, w, jit);
    float e0 = n * h - drop * sin(3.14159 * fract(x));
    return vec4(n, x, v - e0, eu - v);
}

void main() {
    vec3 col = vColor.rgb;
    float a = vColor.a;
    if (vCode < -1.5) {
        // A line at least a pixel wide, thinner ones fainter.
        float hw = vRoof.y;
        a *= clamp(max(hw, 0.5) + 0.5 - abs(vRoof.x), 0.0, 1.0) * min(1.0, 2.0 * hw);
    } else if (vCode >= 0.0) {
        int ci = int(vCode + 0.5);
        int mat = ci & 15;
        float wear = float(ci >> 4) / 15.0;
        float pxFt = uPxPerSq / 5.0;
        float px = 1.0 / pxFt;
        float u = vRoof.x / max(vRoof.z, 1e-3);
        float v = vRoof.y;
        vec2 p = vec2(u, v);
        if (mat == 12) {
            // Soot round a chimney (u, v: from its middle, 1 at the rim): one soft smudge.
            a *= 0.6 * (1.0 - smoothstep(0.15, 1.0, length(p)));
        } else {
            // Matte: saturation at most 0.55 at the same value (copper and glazed tile, dustier: 0.5).
            float mx = max(col.r, max(col.g, col.b));
            float sat = (mx - min(col.r, min(col.g, col.b))) / max(mx, 1e-3);
            float cap = mat == 0 ? 0.45 : mat == 6 || mat == 7 ? 0.42 : 0.55;
            if (sat > cap) col = mx - (mx - col) * (cap / sat);
            // The pieces' size (ft; shrinking toward a cone's or dome's apex): under ~4 px the drawing fades
            // to the flat colour.
            float size = mat == 0 ? 4.6 : mat == 1 ? 3.4 : mat == 2 ? 4.8 : mat == 3 ? 3.0 : mat == 4 ? 4.4 : mat <= 6 ? 4.4 : mat == 7 ? 2.8 : mat == 9 ? 5.5 : mat == 10 ? 2.2 : 8.0;
            float sp = size * pxFt * min(vRoof.z, 1.0);
            // (Clay's scale rows go sooner: at a wide zoom they alias into hatching.)
            float k = mat == 2 ? smoothstep(8.0, 13.0, sp) : smoothstep(4.0, 7.0, sp);
            // (Scallops and piece joints go first, to flat courses.)
            float kp = mat == 2 ? smoothstep(13.0, 20.0, sp) : smoothstep(8.0, 14.0, sp);
            if (k > 0.0) {
                vec3 c = col;
                float s = 1.0;
                // Lines: a darker tone of the material (not ink), never under most of a pixel wide.
                float lw = max(0.13, 0.8 * px);
                // A slight hand wobble along the courses.
                float wob = 0.09 * sin(u * 0.83) + 0.05 * sin(u * 2.1 + 1.3);
                // The piece's hash (patched ones), -1 where there are no pieces.
                float tile = -1.0;
                // Moss takes to it (not metal, glaze or plaster).
                bool mossy = mat <= 4 || mat == 9 || mat == 10;
                if (mat == 0) {
                    // Thatch: a few wide wavy courses of straw hanging over the one below, each lit along
                    // its lower lip, the next course's shadow on its top; no strokes inside.
                    vec4 q = scallops(u, v + wob, 4.6, 4.4, 0.7 * kp, 0.0);
                    s = (q.z < 0.9 ? 1.05 : 1.0) * mix(0.88, 1.0, stp(0.8, q.w, px));
                    s *= 1.0 - 0.16 * ln(q.z, lw, px);
                } else if (mat == 1) {
                    // Slate: big offset rectangles in two soft tones, a course line under each row and a joint
                    // between slates, the next course's shadow on its top.
                    float vv = v + wob;
                    float row = floor(vv / 3.2);
                    float x = u / 4.4 + 0.5 * mod(row, 2.0);
                    float fx = fract(x);
                    float fv = fract(vv / 3.2) * 3.2;
                    // (A tone per patch of 3 slates by 3 courses, two tones; no patched slates.)
                    s = (hash(vec2(floor(floor(x) / 3.0), floor(row / 3.0))) < 0.5 ? 0.975 : 1.025) * mix(1.0, 0.92, stp(2.65, fv, px));
                    s *= 1.0 - 0.26 * ln(fv, lw, px);
                    s *= 1.0 - 0.2 * kp * ln(min(fx, 1.0 - fx) * 4.4, lw, px);
                } else if (mat == 2) {
                    // Clay tile: a few chunky rows of rounded scallops, each lit on one flank and shaded on the
                    // other in flat tones, its lower rim drawn.
                    // (Scales a little uneven in size and offset; the rim a mid brown, not ink.)
                    vec4 q = scallops(u, v + wob, 4.8, 4.2, kp, 0.25);
                    float fx = fract(q.y);
                    tile = hash(vec2(floor(q.y), q.x));
                    s = mix(1.0, fx < 0.32 ? 1.08 : fx < 0.7 ? 1.0 : 0.9, kp) * mix(0.86, 1.0, stp(0.8, q.w, px));
                    c = mix(c, vec3(0.45, 0.31, 0.23) / max(s, 0.5), 0.55 * ln(q.z, lw * 1.2, px));
                } else if (mat == 3) {
                    // Wood shingle: uneven courses of odd-width shingles, close tones, a grain stroke in each.
                    float vv = v + wob;
                    float row = floor(vv / 3.2);
                    float x = u / 2.8 + 0.37 * row + 0.3 * hash(vec2(row, 5.0));
                    x += 0.25 * sin(x * 1.9 + row * 1.7);
                    tile = hash(vec2(floor(x), row));
                    float fx = fract(x);
                    float fv = fract(vv / 3.2 + 0.08 * (tile - 0.5)) * 3.2;
                    // (A tone per course.)
                    s = mix(1.0, tone3(hash(vec2(row, 2.0))), 0.7) * mix(1.0, 0.9, stp(2.6, fv, px));
                    s *= 1.0 - 0.26 * ln(fv, lw, px);
                    s *= 1.0 - 0.24 * kp * ln(min(fx, 1.0 - fx) * 2.8, lw, px);
                    float gx = abs(fx - 0.3 - 0.4 * fract(tile * 7.3)) * 2.8;
                    s *= 1.0 - 0.14 * kp * ln(gx, lw, px) * step(0.4, fv) * step(fv, 1.2 + 0.9 * tile);
                } else if (mat == 4) {
                    // Turf: a few painted grassy lumps (lighter upslope, a soft rim below), now and then a flower.
                    vec2 cell = floor(p / 4.4);
                    float fh = hash(cell + 17.0);
                    vec2 o = p - (cell + 0.5 + 0.18 * (vec2(fh, hash(cell + 5.0)) - 0.5)) * 4.4;
                    float r = 1.3 + 0.4 * hash(cell + 3.0);
                    float d = length(o * vec2(0.85, 1.0));
                    float in1 = 1.0 - stp(r, d, px);
                    s = mix(0.95, o.y > 0.15 * r ? 1.07 : 1.01, in1);
                    s *= 1.0 - 0.14 * ln(abs(d - r), lw, px) * step(o.y, 0.1);
                    // (Soft green tufts: the lumps' lit tops greener.)
                    c *= mix(vec3(1.0), vec3(0.94, 1.07, 0.88), in1 * step(0.15 * r, o.y));
                    if (fh > 0.92) c = mix(c, (fh > 0.96 ? vec3(0.84, 0.81, 0.68) : vec3(0.78, 0.66, 0.34)) / max(s, 0.5), ln(length(o - vec2(0.0, 0.3 * r)), 0.32, px));
                } else if (mat <= 6) {
                    // Lead and copper: a few standing seams down the slope (lit, a shadow beside); copper in flat
                    // patches of verdigris.
                    // (A slight hand wobble down the seams.)
                    float x = (u + 0.12 * sin(v * 0.7 + u * 0.3)) / 4.4;
                    float d = fract(x) * 4.4;
                    tile = hash(vec2(floor(x), floor(v / 7.0)));
                    s = (tile < 0.5 ? 0.98 : 1.02) * (1.0 + 0.12 * ln(d, lw * 1.6, px) - 0.18 * ln(abs(d - 0.3), lw * 1.3, px));
                    // (Copper: flat patches of verdigris, two bays by a course.)
                    if (mat == 6 && hash(vec2(floor(floor(x) / 2.0), floor(v / 9.0)) + 31.0) > 0.55) c *= vec3(1.12, 1.1, 1.04);
                } else if (mat == 7) {
                    // Glazed tile: neat offset rows of big tiles in three tones, a flat sheen on each.
                    float vv = v + wob;
                    vec2 gq = vec2((u + 0.1 * sin(v * 0.9)) / 2.8 + 0.5 * mod(floor(vv / 2.8), 2.0), vv / 2.8);
                    vec2 f = fract(gq);
                    tile = hash(floor(gq));
                    // (A tone per patch of 3 tiles by 2 rows.)
                    s = tone3(hash(floor(floor(gq) / vec2(3.0, 2.0)))) * mix(1.0, ((1.0 - f.x) * f.y > 0.55 ? 1.06 : 1.0) * (f.y > 0.82 ? 0.92 : 1.0), kp);
                    s *= 1.0 - 0.26 * ln(min(min(f.x, 1.0 - f.x) * kp + (1.0 - kp), min(f.y, 1.0 - f.y)) * 2.8, lw, px);
                } else if (mat == 9) {
                    // Hide / felt: broad panels in three tones, stitched seams, rope bands across.
                    float x = u / 5.5;
                    float fx = fract(x);
                    tile = hash(vec2(floor(x), floor(v / 9.0)));
                    s = tone3(tile);
                    s *= 1.0 - 0.22 * kp * ln(min(fx, 1.0 - fx) * 5.5, lw, px) * step(0.4, fract(v * 0.8));
                    float rope = ln(abs(fract(v / 8.0) - 0.5) * 8.0, 0.5, px);
                    float twist = step(0.5, fract((u + v) * 1.2));
                    c = mix(c, vec3(0.42, 0.35, 0.26) * (0.88 + 0.18 * twist) / max(s, 0.5), rope);
                } else if (mat == 10) {
                    // Plank: broad boards down the slope in three tones, seams, butt joints here and there, a grain
                    // stroke on each board.
                    float x = u / 2.2;
                    float fx = fract(x);
                    float bh = hash(vec2(floor(x), 9.0));
                    float e = fract(v / 9.0 + bh);
                    tile = hash(vec2(floor(x), floor(v / 9.0 + bh)));
                    // (A tone per 3 boards.)
                    s = tone3(hash(vec2(floor(floor(x) / 3.0), 4.0)));
                    s *= 1.0 - 0.28 * ln(min(fx, 1.0 - fx) * 2.2, lw, px);
                    s *= 1.0 - 0.24 * ln(min(e, 1.0 - e) * 9.0, lw, px);
                    float gx = abs(fx - 0.3 - 0.4 * fract(tile * 5.1)) * 2.2;
                    s *= 1.0 - 0.12 * kp * ln(gx, lw, px) * step(0.15, e) * step(e, 0.7);
                } else {
                    // Flat clay and whitewash: smooth plaster, a few soft painted patches.
                    if (mat == 8) c *= vec3(1.03, 1.0, 0.96);
                    vec2 cell = floor(p / 11.0);
                    float ph = hash(cell + 61.0);
                    if (ph < 0.4) {
                        vec2 o = p - (cell + 0.3 + 0.4 * vec2(hash(cell + 63.0), hash(cell + 67.0))) * 11.0;
                        float r = 2.2 + 1.6 * fract(ph * 17.0);
                        float d = length(o * vec2(1.0, 1.3)) + 0.35 * sin(o.x * 0.9 + o.y * 0.7 + ph * 20.0);
                        s = mix(1.0, ph < 0.2 ? 0.95 : 1.04, 1.0 - smoothstep(r - 0.8, r + 0.4, d));
                    }
                }
                if (wear > 0.05) {
                    // A patched piece of a slightly different flat tone here and there; a few soft painted moss
                    // blobs (a darker smudge where moss won't take).
                    if (tile >= 0.0) {
                        float t2 = fract(tile * 13.7);
                        if (t2 < wear * wear * 0.03) s *= 0.9;
                        else if (t2 < wear * wear * 0.06) {
                            s *= 1.07;
                            c *= vec3(1.03, 1.0, 0.96);
                        }
                    }
                    vec2 mc = floor(p / 9.0);
                    // (Only old roofs, and few blobs on them: spread evenly they read as polka dots.)
                    if (hash(mc + 41.0) < max(0.0, wear - 0.6) * 0.3) {
                        vec2 o = p - (mc + 0.5) * 9.0 - 2.0 * (vec2(hash(mc + 43.0), hash(mc + 47.0)) - 0.5);
                        float r = 1.2 + 0.8 * hash(mc + 53.0);
                        float d = min(length(o), length(o - vec2(0.55, 0.3) * r) + 0.2);
                        // (Soft, and gone before a wide zoom smudges them.)
                        float mm = (1.0 - smoothstep(r - 0.8, r + 0.2, d)) * kp;
                        if (mossy) c = mix(c, vec3(0.42, 0.45, 0.3) / max(s, 0.5), mm * 0.32);
                        else s *= 1.0 - 0.05 * mm;
                    }
                }
                col = mix(col, c * s, k);
            }
        }
    }
    finalColor = vec4(col * a, a) * uColor;
}
