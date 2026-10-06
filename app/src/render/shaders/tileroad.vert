#version 300 es
// Road segment owned by a tile, expanded into a screen-space capsule (same transform as the
// terrain tile). Classes: 0 king's road, 1 road, 2 track, 3 paved street (the strip beside a
// block; no symbol line), 4 paved main street, 5 a ferry's line across a river (dashed).

in vec2 aA;
in vec2 aB;
in vec2 aCorner; // (along 0/1, side -1/+1)
in vec2 aW;      // bed width (ft) at A and B
in vec2 aQ;      // class at A and B

out vec2 vP;
flat out vec2 vSA;
flat out vec2 vSB;
flat out float vHalf;
flat out float vClass;
flat out float vFade;
flat out float vBed;
flat out float vPpf;

uniform mat3 uProjectionMatrix;
uniform mat3 uWorldTransformMatrix;
uniform mat3 uTransformMatrix;

uniform float uPpf;

void main() {
    mat3 m = uWorldTransformMatrix * uTransformMatrix;
    vec2 sa = (m * vec3(aA, 1.0)).xy;
    vec2 sb = (m * vec3(aB, 1.0)).xy;
    float cls = aQ.x;
    float w = aW.x;
    // Symbolic half width (px) by class, or the real bed once it is wider on screen.
    float symbolic = cls < 0.5 ? 1.25 : (cls < 1.5 ? 0.9 : (cls < 2.5 ? 0.6 : (cls < 3.5 ? 0.0 : (cls < 4.5 ? 0.9 : 0.7))));
    float bedPx = 0.5 * w * uPpf;
    // A ferry's line stays a line (its rope), however close.
    float hw = cls > 4.5 ? max(symbolic, min(bedPx * 0.15, 2.0)) : max(symbolic, bedPx);
    vec2 dir = sb - sa;
    float len = length(dir);
    dir = len > 1e-4 ? dir / len : vec2(1.0, 0.0);
    vec2 nrm = vec2(-dir.y, dir.x);
    float pad = hw + 1.5;
    vec2 p = mix(sa, sb, aCorner.x) + dir * (aCorner.x * 2.0 - 1.0) * pad + nrm * aCorner.y * pad;

    vP = p;
    vSA = sa;
    vSB = sb;
    vHalf = hw;
    vClass = cls;
    vBed = smoothstep(1.5, 3.0, bedPx);
    vPpf = uPpf;
    // Lesser roads appear as you zoom in (ft per px thresholds).
    float ftpx = 1.0 / uPpf;
    vFade = cls < 0.5 ? 1.0 : (cls < 1.5 || cls > 3.5 ? 1.0 - smoothstep(2500.0, 4500.0, ftpx) : (cls < 2.5 ? 1.0 - smoothstep(700.0, 1400.0, ftpx) : 1.0 - smoothstep(6.0, 14.0, ftpx)));
    gl_Position = vec4((uProjectionMatrix * vec3(p, 1.0)).xy, 0.0, 1.0);
    // Roads not shown at this zoom: off screen, so they cost no fragments at all.
    if (vFade <= 0.0) gl_Position = vec4(2.0, 2.0, 2.0, 1.0);
}
