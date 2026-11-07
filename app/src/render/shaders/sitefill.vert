#version 300 es
// Settlement polygons (buildings, blocks, plazas, fields), pre-triangulated, in tile units.

in vec2 aPos;
in vec4 aColor;
in vec3 aEdge;

out vec4 vColor;
out vec2 vTile;
out vec3 vEdge;

uniform mat3 uProjectionMatrix;
uniform mat3 uWorldTransformMatrix;
uniform mat3 uTransformMatrix;

void main() {
    mat3 m = uWorldTransformMatrix * uTransformMatrix;
    vec2 p = (m * vec3(aPos, 1.0)).xy;
    vColor = aColor;
    vTile = aPos;
    vEdge = aEdge;
    gl_Position = vec4((uProjectionMatrix * vec3(p, 1.0)).xy, 0.0, 1.0);
}
