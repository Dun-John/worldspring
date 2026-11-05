#version 300 es
// Straight-alpha vertex colour, premultiplied, times the container's tint and alpha. Roof faces
// (code >= 0: material + 16 × weathering 0-15, `battlePrep` MeshBuilder) are patterned in roof-local
// feet: u along the courses (stored × d, divided back so a cone's or dome's run along the rays to its
// apex), v up the slope from the eave. Cartoon tones: the vertex colour's matte shade times a few close
// steps, detail fading to the flat colour where its rhythm is under ~3 px.
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

// A smooth 1-D noise (two hashes).
float vnoise1(float x, float k) {
    float i = floor(x);
    float f = fract(x);
    return mix(hash(vec2(i, k)), hash(vec2(i + 1.0, k)), f * f * (3.0 - 2.0 * f));
}

// Coverage of a line w ft wide at distance d ft from its middle, antialiased over a pixel (px ft).
float ln(float d, float w, float px) {
    return 1.0 - smoothstep(0.5 * (w - px), 0.5 * (w + px), d);
}

// A course of overlapping pieces, f its fraction up from its lower edge: the lower edge catches the
// light, the shadow of the course above falls on its top.
float course(float f) {
    return mix(1.06, 1.0, smoothstep(0.0, 0.14, f)) * mix(1.0, 0.76, smoothstep(0.74, 1.0, f));
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
            // Soot round a chimney (u, v: from its middle, 1 at the rim).
            float r = length(p) + 0.35 * (vnoise(p * 2.6 + 5.0) - 0.5);
            a *= 0.7 * (1.0 - smoothstep(0.2, 1.0, r));
        } else {
            // The pattern's rhythm (ft): under ~3 px it fades out.
            float size = mat == 0 ? 1.4 : mat == 1 ? 1.0 : mat == 2 ? 1.0 : mat == 3 ? 0.9 : mat == 4 ? 1.2 : mat <= 6 ? 1.6 : mat == 7 ? 0.8 : mat == 8 ? 2.0 : mat == 9 ? 2.0 : mat == 10 ? 0.8 : 2.5;
            float k = smoothstep(2.0, 4.0, size * pxFt);
            if (k > 0.0) {
                vec3 c = col;
                float s = 1.0;
                // The piece's hash (missing and patched ones), -1 where there are no pieces.
                float tile = -1.0;
                // Moss takes to it (not metal, glaze or plaster).
                bool mossy = mat <= 4 || mat == 9 || mat == 10;
                if (mat == 0) {
                    // Thatch: long straw streaks down the slope in layered courses, a ragged fringe at the eave.
                    float layer = v / 1.4 + 0.3 * vnoise1(u * 0.6, 3.0);
                    float f = fract(layer);
                    float streak = vnoise(vec2(u * 2.8, v * 0.3 + floor(layer) * 7.0));
                    s = (0.86 + 0.2 * streak) * mix(1.0, 0.8, smoothstep(0.7, 1.0, f));
                    float h = hash(vec2(floor(u * 5.0), 1.0));
                    if (v < 0.35 + 0.2 * h) s *= 0.8 + 0.12 * h;
                } else if (mat == 1) {
                    // Slate: small slates in courses, each offset half a slate, blue-grey from slate to slate.
                    float row = floor(v);
                    float x = u / 1.2 + 0.5 * mod(row, 2.0);
                    tile = hash(vec2(floor(x), row));
                    float fx = fract(x);
                    s = (0.94 + 0.1 * tile) * course(fract(v));
                    s *= 1.0 - 0.26 * ln(min(fx, 1.0 - fx) * 1.2, 0.11, px);
                    c *= vec3(1.0 - 0.06 * (tile - 0.5), 1.0, 1.0 + 0.08 * (tile - 0.5));
                } else if (mat == 2) {
                    // Clay barrel tile: half-round tiles in rows down the slope, scalloped ends, dark channels.
                    float x = u / 0.95;
                    float fx = fract(x);
                    float barrel = sin(3.14159 * fx);
                    float layer = (v + 0.22 * barrel) / 1.05;
                    tile = hash(vec2(floor(x), floor(layer)));
                    s = (0.76 + 0.34 * barrel - 0.1 * fx) * course(fract(layer)) * (0.93 + 0.14 * tile);
                    s *= 1.0 - 0.3 * ln(min(fx, 1.0 - fx) * 0.95, 0.13, px);
                } else if (mat == 3) {
                    // Wood shingle / shake: staggered courses of shingles of odd widths, grain down them.
                    float row = floor(v / 0.9);
                    float x = u / 0.7 + 0.37 * row + 0.3 * hash(vec2(row, 5.0));
                    tile = hash(vec2(floor(x), row));
                    float fx = fract(x);
                    s = (0.88 + 0.18 * tile) * course(fract(v / 0.9 + 0.1 * (tile - 0.5)));
                    s *= 0.95 + 0.1 * vnoise(vec2(u * 6.0, v * 0.6));
                    s *= 1.0 - 0.28 * ln(min(fx, 1.0 - fx) * 0.7, 0.1, px);
                } else if (mat == 4) {
                    // Turf: lumpy grass, a few flowers.
                    s = 0.84 + 0.26 * vnoise(p * 0.8) + 0.07 * (hash(floor(p * 4.0)) - 0.5);
                    vec2 cell = floor(p / 1.6);
                    float fh = hash(cell + 17.0);
                    if (fh > 0.9) {
                        vec2 o = fract(p / 1.6) - 0.25 - 0.5 * vec2(hash(cell + 3.0), hash(cell + 5.0));
                        float dot1 = ln(length(o) * 1.6, 0.26, px);
                        c = mix(c, fh > 0.95 ? vec3(0.84, 0.81, 0.68) / max(s, 0.5) : vec3(0.78, 0.66, 0.34) / max(s, 0.5), dot1);
                    }
                } else if (mat <= 6) {
                    // Lead and copper: standing seams down the slope (lit, a shadow beside), cross welts; copper
                    // streaked with verdigris.
                    float x = u / 1.6;
                    float d = fract(x) * 1.6;
                    s = 0.96 + 0.06 * vnoise(vec2(u * 0.8, v * 0.25));
                    s *= 1.0 + 0.16 * ln(d, 0.13, px) - 0.14 * ln(abs(d - 0.17), 0.12, px);
                    s *= 1.0 - 0.12 * ln(fract(v / 5.0 + 0.5 * mod(floor(x), 2.0)) * 5.0, 0.09, px);
                    if (mat == 6) {
                        float st = vnoise(vec2(u * 1.7, v * 0.18));
                        c = mix(c, c * vec3(1.16, 1.14, 1.06), smoothstep(0.45, 0.8, st));
                        c = mix(c, c * vec3(0.92, 0.86, 0.78), smoothstep(0.75, 0.95, 1.0 - st) * 0.6);
                    }
                } else if (mat == 7) {
                    // Glazed tile: neat rows, crisp joints, a slight sheen on each (no glow).
                    vec2 gq = p / 0.8;
                    vec2 f = fract(gq);
                    tile = hash(floor(gq));
                    s = (0.95 + 0.08 * tile) * (0.95 + 0.1 * (1.0 - f.x) * f.y);
                    s *= 1.0 - 0.3 * ln(min(min(f.x, 1.0 - f.x), min(f.y, 1.0 - f.y)) * 0.8, 0.1, px);
                } else if (mat == 8) {
                    // Flat clay: smooth mud plaster, trowel marks, a few cracks.
                    float n = vnoise(p * 0.45);
                    s = 0.95 + 0.08 * n;
                    float cr = abs(vnoise(p * 0.7 + 13.0) - 0.5) * 2.2;
                    s *= 1.0 - 0.3 * ln(cr, 0.09, px) * step(0.55, n) * smoothstep(1.5, 3.0, 0.5 * pxFt);
                } else if (mat == 9) {
                    // Hide / felt: panels stitched together, rope bands across them.
                    float x = u / 3.2;
                    float fx = fract(x);
                    tile = hash(vec2(floor(x), floor(v / 6.0)));
                    s = (0.9 + 0.14 * tile) * (0.95 + 0.07 * vnoise(p * 1.7));
                    s *= 1.0 - 0.28 * ln(min(fx, 1.0 - fx) * 3.2, 0.12, px) * step(0.45, fract(v * 2.0));
                    float rope = ln(abs(fract(v / 2.6) - 0.5) * 2.6, 0.3, px);
                    float twist = step(0.5, fract((u + v) * 2.2));
                    c = mix(c, vec3(0.42, 0.35, 0.26) * (0.85 + 0.25 * twist) / max(s, 0.5), rope);
                    tile = -1.0;
                } else if (mat == 10) {
                    // Plank: boards down the slope, seams between, butt joints here and there, grain.
                    float x = u / 0.8;
                    float fx = fract(x);
                    float bh = hash(vec2(floor(x), 9.0));
                    float e = fract(v / 7.0 + bh);
                    tile = hash(vec2(floor(x), floor(v / 7.0 + bh)));
                    s = (0.86 + 0.22 * tile) * (0.94 + 0.1 * vnoise(vec2(u * 6.0, v * 0.5)));
                    s *= 1.0 - 0.32 * ln(min(fx, 1.0 - fx) * 0.8, 0.1, px);
                    s *= 1.0 - 0.3 * ln(min(e, 1.0 - e) * 7.0, 0.1, px);
                } else {
                    // Whitewash: bright plaster, soft brushed shading.
                    s = (0.97 + 0.05 * vnoise(p * 0.35)) * (0.985 + 0.03 * hash(vec2(floor(u * 2.0), floor(v * 0.5))));
                }
                if (wear > 0.05) {
                    // Missing pieces (dark battens), newer patches; rain streaks down the slope; moss.
                    if (tile >= 0.0) {
                        float t2 = fract(tile * 13.7);
                        if (t2 < wear * wear * 0.025) {
                            s *= 0.55;
                            c = mix(c, vec3(0.24, 0.2, 0.16), 0.5);
                        } else if (t2 < wear * wear * 0.07) {
                            s *= 1.1;
                            c *= vec3(1.04, 1.0, 0.94);
                        }
                    }
                    s *= 1.0 - 0.22 * wear * smoothstep(0.55, 0.9, vnoise1(u * 1.1, floor(v / 9.0)));
                    float mo = vnoise(p * 0.33 + 41.0) + 0.22 * hash(floor(p * 3.0));
                    float mm = smoothstep(1.22 - 0.5 * wear, 1.32 - 0.5 * wear, mo);
                    if (mossy) c = mix(c, vec3(0.38, 0.41, 0.25) / max(s, 0.5), mm * 0.55);
                    else s *= 1.0 - 0.12 * mm;
                }
                col = mix(col, c * s, k);
            }
        }
    }
    finalColor = vec4(col * a, a) * uColor;
}
