#version 300 es
// Sea lane: thin dashed ink. The dashes are measured in feet along the line, their length a
// power of two in feet blended with the next one up, so they keep their place while panning and
// grow or part smoothly while zooming (a screen-pixel dash would slide along the line).

in float vLat;
in float vS;
flat in float vHalf;

out vec4 finalColor;

uniform float uPpf;
uniform float uAlpha;
uniform float uPeriodPx;
uniform vec3 uInk;

const float ON = 0.7;

float dash(float s, float period) {
    float pPx = period * uPpf;
    float x = fract(s / period) * pPx;
    return clamp(min(x, ON * pPx - x) + 0.5, 0.0, 1.0);
}

void main() {
    float cover = clamp(vHalf + 0.5 - abs(vLat), 0.0, 1.0);
    float u = log2(uPeriodPx / uPpf);
    float k = floor(u);
    float p0 = exp2(k);
    float on = mix(dash(vS, p0), dash(vS, 2.0 * p0), smoothstep(0.0, 1.0, u - k));
    float a = cover * on * uAlpha;
    if (a <= 0.002) discard;
    finalColor = vec4(uInk * a, a);
}
