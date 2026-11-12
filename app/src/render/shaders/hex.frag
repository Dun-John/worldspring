#version 300 es
// Overland hex grid: each pixel's distance to the nearest hex edge (the lattice is the same
// round every hex centre, so only the point's place in its own hex matters), drawn as a line of
// `uWidth` px, clipped to the map.

in vec2 vW;

out vec4 finalColor;

uniform float uFlat;  // 1 flat-topped, 0 pointy
uniform float uS;     // ft across the flats
uniform float uFpp;   // ft per px
uniform float uAa;    // a device pixel (px)
uniform float uWidth; // line width (px)
uniform vec3 uColor;
uniform float uAlpha;
uniform vec4 uMap;    // the map (ft from the reference hex centre): x0, y0, x1, y1

const float SQRT3 = 1.7320508;

void main() {
    if (vW.x < uMap.x || vW.y < uMap.y || vW.x > uMap.z || vW.y > uMap.w) discard;
    // Flat-topped: the point turned a quarter into the pointy frame (the same lattice turned).
    vec2 p = uFlat > 0.5 ? vec2(vW.y, vW.x) : vW;
    float R = uS / SQRT3;
    // Axial coordinates (pointy), rounded in cube coordinates to the nearest centre.
    float q = (SQRT3 / 3.0 * p.x - p.y / 3.0) / R;
    float r = (2.0 / 3.0 * p.y) / R;
    vec3 c = vec3(q, r, -q - r);
    vec3 rc = floor(c + 0.5);
    vec3 dc = abs(rc - c);
    if (dc.x > dc.y && dc.x > dc.z) rc.x = -rc.y - rc.z;
    else if (dc.y > dc.z) rc.y = -rc.x - rc.z;
    vec2 centre = vec2(uS * (rc.x + rc.y * 0.5), 1.5 * R * rc.y);
    vec2 d = abs(p - centre);
    // Distance to the hex's edge (its apothem s/2 less the largest of the three edge normals').
    float e = max(d.x, dot(d, vec2(0.5, SQRT3 / 2.0)));
    float px = (uS * 0.5 - e) / uFpp;
    float a = (1.0 - smoothstep(uWidth * 0.5 - uAa * 0.5, uWidth * 0.5 + uAa * 0.5, px)) * uAlpha;
    if (a <= 0.002) discard;
    finalColor = vec4(uColor * a, a);
}
