#version 300 es
// Sea lane: a ribbon along the lane's line, its corners mitred on the CPU (world-space unit
// normals scaled by the mitre), widened here to a screen-pixel width by class.

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
    // Half widths (px): busy 1.0, normal 0.7, quiet 0.5, ferry (3) 0.7.
    float hw = (cls < 0.5 ? 1.0 : (cls < 1.5 ? 0.7 : (cls < 2.5 ? 0.5 : 0.7))) * uWidth;
    float pad = hw + 1.0;
    vLat = aInfo.x * pad;
    vS = aInfo.y;
    vHalf = hw;
    gl_Position = vec4((uProjectionMatrix * vec3(p + n * pad, 1.0)).xy, 0.0, 1.0);
}
