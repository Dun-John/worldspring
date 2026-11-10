#version 300 es
// Sea lane: a ribbon along the lane's line, its corners mitred on the CPU (world-space unit
// normals scaled by the mitre), widened here to a screen-pixel size by class (dots are drawn in the fragment shader).

in vec2 aP;    // point (ft from the layer's origin)
in vec2 aN;    // mitred normal (ft-free: unit length times the mitre factor, side included)
in vec3 aInfo; // (side -1/+1, distance along the line ft, class)

out float vLat;
out float vS;
flat out float vHalf;

uniform mat3 uProjectionMatrix;
uniform mat3 uWorldTransformMatrix;
uniform mat3 uTransformMatrix;
uniform float uWidth; // widening when zoomed out

void main() {
    mat3 m = uWorldTransformMatrix * uTransformMatrix;
    vec2 p = (m * vec3(aP, 1.0)).xy;
    // The normal turned with the view (a similarity: its scale divided out).
    vec2 nt = (m * vec3(aN, 0.0)).xy;
    float nl = length(nt);
    float k = length(aN);
    vec2 n = nl > 1e-12 ? nt / nl * k : vec2(0.0);
    float cls = aInfo.z;
    // Dot radii (px): busy 2.85, normal 2.25, quiet 1.9, ferry (3) 2.25; the pad holds the disc and its rim.
    float hw = (cls < 0.5 ? 2.85 : (cls < 1.5 ? 2.25 : (cls < 2.5 ? 1.9 : 2.25))) * uWidth;
    float pad = hw + 1.5;
    vLat = aInfo.x * pad;
    vS = aInfo.y;
    vHalf = hw;
    gl_Position = vec4((uProjectionMatrix * vec3(p + n * pad, 1.0)).xy, 0.0, 1.0);
}
