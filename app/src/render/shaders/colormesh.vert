#version 300 es
// Pre-tessellated, vertex-coloured triangles (battlemap roofs), in chunk pixels; roof faces carry
// their pattern's coordinates and code (`battlePrep` MeshBuilder.vert). Lines (code -2 - half width) have
// their vertices on the middle, the unit normal in xy and the side in w: widened here to at least a
// screen pixel and a pixel more to antialias (`colormesh.frag`).

in vec2 aPos;
in vec4 aColor;
in vec4 aRoof;

out vec4 vColor;
out vec3 vRoof;
flat out float vCode;

uniform mat3 uProjectionMatrix;
uniform mat3 uWorldTransformMatrix;
uniform mat3 uTransformMatrix;
uniform float uPxPerSq;

void main() {
    mat3 m = uWorldTransformMatrix * uTransformMatrix;
    vColor = aColor;
    vRoof = aRoof.xyw;
    vCode = aRoof.z;
    vec2 pos = aPos;
    if (aRoof.z < -1.5) {
        // (Screen px per chunk px: the battlemap's 64 a square.)
        float sc = uPxPerSq / 64.0;
        float hw = -2.0 - aRoof.z;
        float e = max(hw, 0.5 / sc) + 1.0 / sc;
        pos += aRoof.xy * (aRoof.w * e);
        vRoof = vec3(aRoof.w * e * sc, hw * sc, 0.0);
    }
    gl_Position = vec4((uProjectionMatrix * m * vec3(pos, 1.0)).xy, 0.0, 1.0);
}
