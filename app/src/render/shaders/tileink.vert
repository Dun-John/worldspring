#version 300 es
// Ink lines owned by a tile (building outlines, town walls, towers) as screen-space capsules.
// aQ.x = style: 0 outline, 1 wall, 2 tower (a zero-length capsule = a round tower), 3 a flight of
// steps (hatched across its line, square ends; aW its full width).

in vec2 aA;
in vec2 aB;
in vec2 aCorner;
in vec2 aW; // width (ft) at A and B
in vec2 aQ; // style at A and B

out vec2 vP;
flat out vec2 vSA;
flat out vec2 vSB;
flat out float vHalf;
flat out float vFade;
flat out float vStyle;
flat out float vPpf;

uniform mat3 uProjectionMatrix;
uniform mat3 uWorldTransformMatrix;
uniform mat3 uTransformMatrix;

uniform float uPpf;

void main() {
    mat3 m = uWorldTransformMatrix * uTransformMatrix;
    vec2 sa = (m * vec3(aA, 1.0)).xy;
    vec2 sb = (m * vec3(aB, 1.0)).xy;
    float style = aQ.x;
    float minPx = style < 0.5 ? 0.45 : (style < 1.5 ? 1.2 : (style < 2.5 ? 2.2 : 0.0));
    float hw = max(minPx, 0.5 * aW.x * uPpf);
    vec2 dir = sb - sa;
    float len = length(dir);
    dir = len > 1e-4 ? dir / len : vec2(1.0, 0.0);
    vec2 nrm = vec2(-dir.y, dir.x);
    float pad = hw + 1.0;
    vec2 p = mix(sa, sb, aCorner.x) + dir * (aCorner.x * 2.0 - 1.0) * pad + nrm * aCorner.y * pad;
    vP = p;
    vSA = sa;
    vSB = sb;
    vHalf = hw;
    // Building outlines only once buildings are a few pixels across.
    // Steps as paved streets do (class 3 on roads).
    vFade = style < 0.5 ? smoothstep(0.04, 0.12, uPpf) : (style > 2.5 ? 1.0 - smoothstep(6.0, 14.0, 1.0 / uPpf) : 1.0);
    vStyle = style;
    vPpf = uPpf;
    gl_Position = vec4((uProjectionMatrix * vec3(p, 1.0)).xy, 0.0, 1.0);
    if (vFade <= 0.0) gl_Position = vec4(2.0, 2.0, 2.0, 1.0);
}
