#version 300 es
// River segment owned by a tile, expanded into a screen-space capsule. Positions are tile
// units (0..1) through the same transform as the terrain tile, so they are exact at any zoom.

in vec2 aA;
in vec2 aB;
in vec2 aCorner; // (along 0/1, side -1/+1)
in vec2 aW;      // channel width (ft) at A and B
in vec2 aQ;      // discharge at A and B

out vec2 vP;
flat out vec2 vSA;
flat out vec2 vSB;
out float vHalf;
out float vFade;

uniform mat3 uProjectionMatrix;
uniform mat3 uWorldTransformMatrix;
uniform mat3 uTransformMatrix;

uniform float uPpf;
uniform float uMinQ;

float halfWidth(float q, float w) {
    float symbolic = clamp(0.55 + 0.8 * (log(q / uMinQ) / log(10.0)), 0.5, 2.6);
    return 0.5 * max(symbolic, w * uPpf);
}

// Small rivers appear as you zoom in; once the carved channel is wide enough to show as
// terrain water (with its own ink banks), the vector line hands over completely.
float fade(float q, float w) {
    return smoothstep(uMinQ * 0.6, uMinQ, q) * (1.0 - smoothstep(3.0, 6.0, w * uPpf));
}

void main() {
    mat3 m = uWorldTransformMatrix * uTransformMatrix;
    vec2 sa = (m * vec3(aA, 1.0)).xy;
    vec2 sb = (m * vec3(aB, 1.0)).xy;
    float q = mix(aQ.x, aQ.y, aCorner.x);
    float w = mix(aW.x, aW.y, aCorner.x);
    float hw = halfWidth(max(aQ.x, aQ.y), max(aW.x, aW.y)) + 1.0;
    vec2 dir = sb - sa;
    float len = length(dir);
    dir = len > 1e-4 ? dir / len : vec2(1.0, 0.0);
    vec2 nrm = vec2(-dir.y, dir.x);
    vec2 p = mix(sa, sb, aCorner.x) + dir * (aCorner.x * 2.0 - 1.0) * hw + nrm * aCorner.y * hw;

    vP = p;
    vSA = sa;
    vSB = sb;
    vHalf = halfWidth(q, w);
    vFade = fade(q, w);
    gl_Position = vec4((uProjectionMatrix * vec3(p, 1.0)).xy, 0.0, 1.0);
    // Rivers too small to show at this zoom: off screen, so they cost no fragments at all. The
    // whole segment or none of it (one corner moved alone stretches the quad into a streak).
    if (max(fade(aQ.x, aW.x), fade(aQ.y, aW.y)) <= 0.0) gl_Position = vec4(2.0, 2.0, 2.0, 1.0);
}
