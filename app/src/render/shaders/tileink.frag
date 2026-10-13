#version 300 es

in vec2 vP;
flat in vec2 vSA;
flat in vec2 vSB;
flat in float vHalf;
flat in float vFade;
flat in float vStyle;
flat in float vPpf;

out vec4 finalColor;

uniform float uAlpha;

const vec3 INK = vec3(0.17, 0.14, 0.11);

void main() {
    vec2 pa = vP - vSA;
    vec2 ba = vSB - vSA;
    float t = dot(pa, ba) / max(dot(ba, ba), 1e-6);
    float h = clamp(t, 0.0, 1.0);
    float d = length(pa - ba * h);
    float cover = 1.0 - smoothstep(vHalf - 0.5, vHalf + 0.5, d);
    if (vStyle > 2.5) {
        // Steps: square ends, a hatch across the line every 2.5 ft (or 3 px), fading in as
        // the treads open up.
        if (t < 0.0 || t > 1.0) discard;
        float along = dot(pa, normalize(ba + vec2(1e-6, 0.0)));
        cover *= step(0.7, fract(along / max(2.5 * vPpf, 3.0))) * 0.5 * smoothstep(0.4, 1.0, vPpf * 2.5);
    }
    float a = cover * vFade * uAlpha;
    if (a <= 0.001) discard;
    finalColor = vec4(INK * a, a);
}
