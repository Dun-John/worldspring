#version 300 es

in vec2 vP;
flat in vec2 vSA;
flat in vec2 vSB;
flat in float vHalf;
flat in float vFade;
flat in float vStyle;
flat in float vPpf;
flat in float vJoin;

out vec4 finalColor;

uniform float uAlpha;

const vec3 INK = vec3(0.17, 0.14, 0.11);

void main() {
    vec2 pa = vP - vSA;
    vec2 ba = vSB - vSA;
    float t = dot(pa, ba) / max(dot(ba, ba), 1e-6);
    float h = clamp(t, 0.0, 1.0);
    float d = length(pa - ba * h);
    float cover = 1.0 - smoothstep(vHalf - 0.5, vHalf + 0.5, d);
    if (vStyle > 3.5 && vStyle < 4.5) {
        // A square tower: square ends.
        if (t < 0.0 || t > 1.0) discard;
    } else if (vStyle > 2.5 && vStyle < 3.5) {
        // Steps: square ends, a hatch across the line every 2.5 ft (or 3 px), fading in as
        // the treads open up.
        if (t < 0.0 || t > 1.0) discard;
        float along = dot(pa, normalize(ba + vec2(1e-6, 0.0)));
        cover *= step(0.7, fract(along / max(2.5 * vPpf, 3.0))) * 0.5 * smoothstep(0.4, 1.0, vPpf * 2.5);
    }
    // A wall piece's joined end (vJoin: end A + 4 * end B, 1 a sharp left turn, 2 right, 3 a shallow bend): its round cap carries no rim
    // except on the outside of the bend, so a run reads as one band (`joinWallInk`).
    float jn = floor(vJoin + 0.5);
    float jb = floor(jn / 4.0);
    float jt = t < 0.0 ? jn - jb * 4.0 : (t > 1.0 ? jb : 0.0);
    float side = ba.x * pa.y - ba.y * pa.x;
    float bare = jt > 2.5 || (jt > 0.5 && side * (jt < 1.5 ? 1.0 : -1.0) >= 0.0) ? 1.0 : 0.0;
    vec3 col = INK;
    // A stone wall's or tower's stone + 1, in sixteenths after its style (a town wall's group, from the plan's
    // looks): the ink shifted toward granite's cool grey, sandstone's honey, rose brick's rose, mud brick's ochre.
    float stone = floor(fract(vStyle) * 16.0 + 0.5);
    if (stone > 0.5 && vStyle < 4.9) {
        col = stone < 1.5 ? vec3(0.15, 0.17, 0.22) : (stone < 3.5 ? vec3(0.25, 0.17, 0.08) : (stone < 4.5 ? vec3(0.27, 0.15, 0.13) : vec3(0.26, 0.16, 0.11)));
    }
    if (vStyle < 4.9 && (vStyle < 2.5 || vStyle > 3.5)) {
        // A wall's or tower's stone (limestone buff when none): its light tone inside a thin ink edge, once wide
        // enough (granite blue-grey, sandstone honey, rose brick rose, mud brick ochre-pink).
        vec3 tone = stone < 0.5 ? vec3(0.86, 0.80, 0.65) : (stone < 1.5 ? vec3(0.66, 0.71, 0.78) : (stone < 3.5 ? vec3(0.90, 0.72, 0.42) : (stone < 4.5 ? vec3(0.82, 0.52, 0.47) : vec3(0.86, 0.64, 0.52))));
        float rim = clamp(vHalf * 0.3, 0.7, 1.3);
        col = mix(INK, tone, max(1.0 - smoothstep(vHalf - rim - 0.5, vHalf - rim + 0.5, d), bare) * step(2.0, vHalf));
        if (bare > 0.5 && (t < 0.0 || t > 1.0)) cover *= 1.0 - smoothstep(vHalf - rim - 0.5, vHalf - rim + 0.5, d);
    }
    if (vStyle > 4.5) {
        // A palisade (timber brown) or rammed earth (tan): its colour inside an ink edge, once wide enough.
        vec3 tone = vStyle > 5.5 ? vec3(0.80, 0.72, 0.56) : vec3(0.55, 0.39, 0.24);
        float rim = clamp(vHalf * 0.35, 0.6, 1.2);
        col = mix(INK, tone, max(1.0 - smoothstep(vHalf - rim - 0.5, vHalf - rim + 0.5, d), bare) * step(1.6, vHalf));
        if (bare > 0.5 && (t < 0.0 || t > 1.0)) cover *= 1.0 - smoothstep(vHalf - rim - 0.5, vHalf - rim + 0.5, d);
    }
    float a = cover * vFade * uAlpha;
    if (a <= 0.001) discard;
    finalColor = vec4(col * a, a);
}
