#version 330 core
in vec2 vUV;
out vec4 FragColor;

uniform sampler2D uSourceTex;
uniform float uColorLevels;
uniform float uDitherStrength;
uniform vec3 uTintColor;
uniform float uTintStrength;

// Values 0..15 arranged for a reasonably well-spread ordered-dither pattern.
const mat4 kBayer = mat4(
     0.0,  8.0,  2.0, 10.0,
    12.0,  4.0, 14.0,  6.0,
     3.0, 11.0,  1.0,  9.0,
    15.0,  7.0, 13.0,  5.0
) / 16.0;

void main() {
    vec4 color = texture(uSourceTex, vUV);

    ivec2 cell = ivec2(mod(gl_FragCoord.xy, 4.0));
    float threshold = kBayer[cell.x][cell.y] - 0.5;

    vec3 tinted = mix(color.rgb, uTintColor, uTintStrength);

    float levels = max(uColorLevels, 1.0);
    vec3 dithered = tinted + threshold * uDitherStrength / levels;
    vec3 quantized = floor(dithered * levels + 0.5) / levels;

    FragColor = vec4(clamp(quantized, 0.0, 1.0), color.a);
}
