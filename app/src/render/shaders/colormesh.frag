#version 300 es
// Straight-alpha vertex colour, premultiplied, times the container's tint and alpha. Roof faces
// (code >= 0: material + 16 × weathering 0-15, `battlePrep` MeshBuilder) are patterned in roof-local
// feet: u along the courses (stored × d, divided back so a cone's or dome's run along the rays to its
// apex), v up the slope from the eave. Painted cartoon: the vertex colour's matte shade (lit NW slope
// lighter) times two or three flat tones per piece, big pieces drawn with sparse lines in a darker tone
// of the material (not ink), detail fading to the flat colour before it can shimmer.
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

float vnoise(vec2 p) {
    vec2 i = floor(p);
    vec2 f = fract(p);
    f = f * f * (3.0 - 2.0 * f);
    return mix(mix(hash(i), hash(i + vec2(1.0, 0.0)), f.x), mix(hash(i + vec2(0.0, 1.0)), hash(i + 1.0), f.x), f.y);
}

// Coverage of a line w ft wide at distance d ft from its middle, antialiased over a pixel (px ft).
float ln(float d, float w, float px) {
    return 1.0 - smoothstep(0.5 * (w - px), 0.5 * (w + px), d);
}

// A step from 0 to 1 at e, antialiased over w (a pixel in x's units).
float stp(float e, float x, float w) {
    return smoothstep(e - 0.5 * w, e + 0.5 * w, x);
}

// Three close flat tones from a piece's hash.
float tone3(float h) {
    return h < 0.34 ? 0.94 : h < 0.67 ? 1.0 : 1.055;
}

// Rows of rounded scallops w ft wide and h ft tall, each row offset half a piece: row n's lower edge
// hangs drop ft at a piece's middle. Returns (row, x in pieces, ft up from its own edge, ft down to the
// next row's edge).
vec4 scallops(float u, float v, float w, float h, float drop) {
    float n = floor(v / h);
    float e1 = (n + 1.0) * h - drop * sin(3.14159 * fract(u / w + 0.5 * mod(n + 1.0, 2.0)));
    if (v >= e1) n += 1.0;
    float x = u / w + 0.5 * mod(n, 2.0);
    float e0 = n * h - drop * sin(3.14159 * fract(x));
    float eu = (n + 1.0) * h - drop * sin(3.14159 * fract(u / w + 0.5 * mod(n + 1.0, 2.0)));
    return vec4(n, x, v - e0, eu - v);
}

// A short painted crack on plaster: a zigzag stroke in about a quarter of 12-ft cells.
float crack(vec2 p, float lw, float px) {
    vec2 cell = floor(p / 12.0);
    float h = hash(cell + 23.0);
    if (h > 0.25) return 0.0;
    vec2 o = p - (cell + 0.3 + 0.4 * vec2(hash(cell + 7.0), hash(cell + 9.0))) * 12.0;
    float an = h * 25.0;
    vec2 dir = vec2(cos(an), sin(an));
    float along = dot(o, dir);
    float across = dot(o, vec2(-dir.y, dir.x)) - 0.35 * (abs(fract(along * 0.55) - 0.5) * 2.0 - 0.5);
    float len = 1.4 + 1.2 * fract(h * 37.0);
    return ln(abs(across), lw * (1.0 - 0.6 * abs(along) / len), px) * step(abs(along), len);
}

void main() {
    vec3 col = vColor.rgb;
    float a = vColor.a;
    if (vCode >= 0.0) {
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
            // Matte: saturation at most 0.55 at the same value (copper and glazed tile are the accent).
            float mx = max(col.r, max(col.g, col.b));
            float sat = (mx - min(col.r, min(col.g, col.b))) / max(mx, 1e-3);
            if (sat > 0.55 && mat != 6 && mat != 7) col = mx - (mx - col) * (0.55 / sat);
            // The pieces' size (ft; shrinking toward a cone's or dome's apex): under ~4 px the drawing fades
            // to the flat colour.
            float size = mat <= 1 ? 3.4 : mat <= 3 ? 3.0 : mat == 4 ? 4.4 : mat <= 6 ? 3.4 : mat == 7 ? 2.8 : mat == 9 ? 5.5 : mat == 10 ? 2.2 : 8.0;
            float k = smoothstep(3.0, 6.0, size * pxFt * min(vRoof.z, 1.0));
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
                    // Thatch: courses of soft bundled tufts hanging over the one below, two long straw
                    // strokes in each, the next course's shadow on its top.
                    vec4 q = scallops(u, v + wob, 3.0, 3.4, 0.8);
                    float fx = fract(q.y);
                    float th = hash(vec2(floor(q.y), q.x));
                    s = (q.z < 0.7 ? 1.05 : 1.0) * mix(0.84, 1.0, stp(0.6, q.w, px)) * (0.97 + 0.06 * th);
                    s *= 1.0 - 0.2 * ln(q.z, lw, px);
                    float st = min(abs(fx - 0.3 - 0.1 * th), abs(fx - 0.68 + 0.08 * th)) * 2.0;
                    s *= 1.0 - 0.15 * ln(st, lw * 0.8, px) * step(0.3, q.z) * step(q.z, 2.1);
                } else if (mat == 1) {
                    // Slate: big offset rectangles in three soft tones, a course line under each row and a joint
                    // between slates, the next course's shadow on its top.
                    float vv = v + wob;
                    float row = floor(vv / 3.2);
                    float x = u / 4.4 + 0.5 * mod(row, 2.0);
                    tile = hash(vec2(floor(x), row));
                    float fx = fract(x);
                    float fv = fract(vv / 3.2) * 3.2;
                    s = tone3(tile) * mix(1.0, 0.9, stp(2.65, fv, px));
                    s *= 1.0 - 0.28 * ln(fv, lw, px);
                    s *= 1.0 - 0.22 * ln(min(fx, 1.0 - fx) * 4.4, lw, px);
                    c *= vec3(1.0 - 0.05 * (tile - 0.5), 1.0, 1.0 + 0.06 * (tile - 0.5));
                } else if (mat == 2) {
                    // Clay tile: a few chunky rows of rounded scallops, each lit on one flank and shaded on the
                    // other in flat tones, its lower rim drawn.
                    vec4 q = scallops(u, v + wob, 3.2, 2.8, 1.0);
                    float fx = fract(q.y);
                    tile = hash(vec2(floor(q.y), q.x));
                    s = (fx < 0.32 ? 1.08 : fx < 0.7 ? 1.0 : 0.89) * mix(0.82, 1.0, stp(0.55, q.w, px)) * (0.97 + 0.06 * tile);
                    s *= 1.0 - 0.3 * ln(q.z, lw * 1.2, px);
                } else if (mat == 3) {
                    // Wood shingle: uneven courses of odd-width shingles, close tones, a grain stroke in each.
                    float vv = v + wob;
                    float row = floor(vv / 3.2);
                    float x = u / 2.8 + 0.37 * row + 0.3 * hash(vec2(row, 5.0));
                    x += 0.25 * sin(x * 1.9 + row * 1.7);
                    tile = hash(vec2(floor(x), row));
                    float fx = fract(x);
                    float fv = fract(vv / 3.2 + 0.08 * (tile - 0.5)) * 3.2;
                    s = (0.97 + 0.06 * tile) * mix(1.0, 0.88, stp(2.6, fv, px));
                    s *= 1.0 - 0.28 * ln(fv, lw, px);
                    s *= 1.0 - 0.26 * ln(min(fx, 1.0 - fx) * 2.8, lw, px);
                    float gx = abs(fx - 0.3 - 0.4 * fract(tile * 7.3)) * 2.8;
                    s *= 1.0 - 0.18 * ln(gx, lw * 0.8, px) * step(0.4, fv) * step(fv, 1.2 + 0.9 * tile);
                } else if (mat == 4) {
                    // Turf: a few painted grassy lumps (lighter upslope, a soft rim below), now and then a flower.
                    vec2 cell = floor(p / 4.4);
                    float fh = hash(cell + 17.0);
                    vec2 o = p - (cell + 0.5 + 0.18 * (vec2(fh, hash(cell + 5.0)) - 0.5)) * 4.4;
                    float r = 1.3 + 0.4 * hash(cell + 3.0);
                    float d = length(o * vec2(0.85, 1.0));
                    float in1 = 1.0 - stp(r, d, px);
                    s = mix(0.95, o.y > 0.15 * r ? 1.07 : 1.01, in1);
                    s *= 1.0 - 0.16 * ln(abs(d - r), lw, px) * step(o.y, 0.1);
                    if (fh > 0.92) c = mix(c, (fh > 0.96 ? vec3(0.84, 0.81, 0.68) : vec3(0.78, 0.66, 0.34)) / max(s, 0.5), ln(length(o - vec2(0.0, 0.3 * r)), 0.32, px));
                } else if (mat <= 6) {
                    // Lead and copper: a few standing seams down the slope (lit, a shadow beside); copper in flat
                    // patches of verdigris.
                    float x = u / 3.4;
                    float d = fract(x) * 3.4;
                    tile = hash(vec2(floor(x), floor(v / 7.0)));
                    s = (tile < 0.5 ? 0.98 : 1.02) * (1.0 + 0.12 * ln(d, lw, px) - 0.18 * ln(abs(d - 0.24), lw, px));
                    if (mat == 6) {
                        float st = vnoise(vec2(u * 0.4, v * 0.14));
                        c = mix(c, c * vec3(1.14, 1.12, 1.05), stp(0.58, st, fwidth(st)));
                    }
                } else if (mat == 7) {
                    // Glazed tile: neat offset rows of big tiles in three tones, a flat sheen on each.
                    vec2 gq = vec2(u / 2.8 + 0.5 * mod(floor(v / 2.8), 2.0), v / 2.8);
                    vec2 f = fract(gq);
                    tile = hash(floor(gq));
                    s = tone3(tile) * ((1.0 - f.x) * f.y > 0.55 ? 1.08 : 1.0) * (f.y > 0.82 ? 0.9 : 1.0);
                    s *= 1.0 - 0.28 * ln(min(min(f.x, 1.0 - f.x), min(f.y, 1.0 - f.y)) * 2.8, lw, px);
                } else if (mat == 9) {
                    // Hide / felt: broad panels in three tones, stitched seams, rope bands across.
                    float x = u / 5.5;
                    float fx = fract(x);
                    tile = hash(vec2(floor(x), floor(v / 9.0)));
                    s = tone3(tile);
                    s *= 1.0 - 0.24 * ln(min(fx, 1.0 - fx) * 5.5, lw, px) * step(0.45, fract(v * 1.2));
                    float rope = ln(abs(fract(v / 5.0) - 0.5) * 5.0, 0.36, px);
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
                    s = tone3(tile);
                    s *= 1.0 - 0.28 * ln(min(fx, 1.0 - fx) * 2.2, lw, px);
                    s *= 1.0 - 0.24 * ln(min(e, 1.0 - e) * 9.0, lw, px);
                    float gx = abs(fx - 0.3 - 0.4 * fract(tile * 5.1)) * 2.2;
                    s *= 1.0 - 0.13 * ln(gx, lw * 0.8, px) * step(0.15, e) * step(e, 0.7);
                } else {
                    // Flat clay and whitewash: smooth plaster, a painted crack or two.
                    if (mat == 8) c *= vec3(1.03, 1.0, 0.96);
                    else s = 1.04;
                    s *= 1.0 - 0.2 * crack(p, lw, px);
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
                    if (hash(mc + 41.0) < max(0.0, wear - 0.55) * 0.5) {
                        vec2 o = p - (mc + 0.5) * 9.0 - 2.0 * (vec2(hash(mc + 43.0), hash(mc + 47.0)) - 0.5);
                        float r = 1.2 + 0.8 * hash(mc + 53.0);
                        float d = min(length(o), length(o - vec2(0.55, 0.3) * r) + 0.2);
                        float mm = 1.0 - smoothstep(r - 0.35, r + 0.05, d);
                        if (mossy) c = mix(c, vec3(0.4, 0.43, 0.27) / max(s, 0.5), mm * 0.45);
                        else s *= 1.0 - 0.06 * mm;
                    }
                }
                col = mix(col, c * s, k);
            }
        }
    }
    finalColor = vec4(col * a, a) * uColor;
}
