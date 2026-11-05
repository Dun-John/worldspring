#version 300 es
// Pre-tessellated, vertex-coloured triangles (battlemap roofs), in chunk pixels; roof faces carry
// their pattern's coordinates and code (`battlePrep` MeshBuilder.vert).

in vec2 aPos;
in vec4 aColor;
in vec4 aRoof;

out vec4 vColor;
out vec3 vRoof;
flat out float vCode;

uniform mat3 uProjectionMatrix;
uniform mat3 uWorldTransformMatrix;
uniform mat3 uTransformMatrix;

void main() {
    mat3 m = uWorldTransformMatrix * uTransformMatrix;
    vColor = aColor;
    vRoof = aRoof.xyw;
    vCode = aRoof.z;
    gl_Position = vec4((uProjectionMatrix * m * vec3(aPos, 1.0)).xy, 0.0, 1.0);
}
