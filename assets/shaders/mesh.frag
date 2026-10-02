#ifdef AFFINE_UV
noperspective in vec2 vUV;
#else
in vec2 vUV;
#endif
in vec3 vLight;
in float vFogFactor;

out vec4 FragColor;

uniform sampler2D uTex;
uniform vec3 uFogColor;

void main() {
    vec4 texColor = texture(uTex, vUV);
    vec3 lit = texColor.rgb * vLight;
    vec3 withFog = mix(lit, uFogColor, vFogFactor);
    FragColor = vec4(withFog, texColor.a);
}
