#version 300 es
// Sea lane: a dotted line of round pale dots with a thin soft sea-blue rim. The dots are spaced
// in feet along the line, the spacing a power of two in feet blended with the next one up, so
// they keep their place while panning and part or fill in smoothly while zooming (a screen-pixel
// spacing would slide along the line).

in float vLat;
in float vS;
flat in float vHalf;

out vec4 finalColor;

uniform float uPpf;
uniform float uAlpha;
uniform float uPeriodPx;
uniform vec3 uInk;
uniform vec3 uRim;

const float RIM_W = 0.8;     // rim width (px)
const float RIM_A = 0.65;    // rim strength

// Coverage of the disc (outer radius vHalf) of the dot nearest along the line.
float dotAt(float s, float period) {
    float pPx = period * uPpf;
    float x = (fract(s / period + 0.5) - 0.5) * pPx;
    float d = length(vec2(x, vLat));
    float outer = clamp(vHalf + 0.5 - d, 0.0, 1.0);
    float fill = clamp(vHalf - RIM_W + 0.5 - d, 0.0, 1.0);
    return outer * mix(RIM_A, 1.0, fill);
}

void main() {
    float u = log2(uPeriodPx / uPpf);
    float k = floor(u);
    float p0 = exp2(k);
    float t = smoothstep(0.0, 1.0, u - k);
    // The fill share (for the colour) comes from the same blend.
    float x0 = (fract(vS / p0 + 0.5) - 0.5) * p0 * uPpf;
    float x1 = (fract(vS / (2.0 * p0) + 0.5) - 0.5) * 2.0 * p0 * uPpf;
    float x = abs(mix(x0, x1, t));
    float d = length(vec2(x, vLat));
    float fill = clamp(vHalf - RIM_W + 0.5 - d, 0.0, 1.0);
    float a = mix(dotAt(vS, p0), dotAt(vS, 2.0 * p0), t) * uAlpha;
    if (a <= 0.002) discard;
    vec3 col = mix(uRim, uInk, fill);
    finalColor = vec4(col * a, a);
}
