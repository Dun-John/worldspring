#version 300 es
// Overland hex grid: one quad over the view (stage px, `uRect`), each corner's world point
// measured from a hex centre near the camera (so f32 keeps its precision anywhere on the map).

in vec2 aP; // 0..1 across the quad

out vec2 vW; // ft from the reference hex centre

uniform mat3 uProjectionMatrix;
uniform mat3 uWorldTransformMatrix;
uniform mat3 uTransformMatrix;
uniform vec4 uRect;  // the quad (stage px): x0, y0, x1, y1
uniform vec2 uHalf;  // the screen's middle (stage px)
uniform vec2 uCam;   // the camera's centre (ft from the reference hex centre)
uniform float uFpp;  // ft per px

void main() {
    vec2 p = mix(uRect.xy, uRect.zw, aP);
    vW = (p - uHalf) * uFpp + uCam;
    mat3 m = uWorldTransformMatrix * uTransformMatrix;
    gl_Position = vec4((uProjectionMatrix * m * vec3(p, 1.0)).xy, 0.0, 1.0);
}
