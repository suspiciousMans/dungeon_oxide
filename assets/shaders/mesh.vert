layout (location = 0) in vec3 aPos;
layout (location = 1) in vec3 aNormal;
layout (location = 2) in vec2 aUV;
layout (location = 3) in vec3 aColor;

uniform mat4 uModel;
uniform mat4 uView;
uniform mat4 uProj;
uniform vec3 uLightDir;
uniform vec3 uAmbientColor;
uniform int uLightingMode;
uniform float uVertexSnapAmount;
uniform float uFogStart;
uniform float uFogEnd;

// Fixed-size point light arrays (simple uniform arrays, not a UBO/SSBO —
// plenty for a handful of level lights and keeps the shader trivial).
uniform vec3 uPointLightPos[4];
uniform vec3 uPointLightColor[4];
uniform float uPointLightIntensity[4];
uniform float uPointLightRange[4];
uniform int uPointLightCount;

#ifdef AFFINE_UV
noperspective out vec2 vUV;
#else
out vec2 vUV;
#endif
out vec3 vLight;
out float vFogFactor;

void main() {
    vec4 worldPos = uModel * vec4(aPos, 1.0);
    vec4 viewPos = uView * worldPos;
    vec4 clipPos = uProj * viewPos;

    if (uVertexSnapAmount > 0.0) {
        float w = clipPos.w;
        vec2 ndc = clipPos.xy / w;
        ndc = floor(ndc / uVertexSnapAmount + 0.5) * uVertexSnapAmount;
        clipPos.xy = ndc * w;
    }

    gl_Position = clipPos;
    vUV = aUV;

    if (uLightingMode == 1) {
        vec3 worldNormal = normalize(mat3(uModel) * aNormal);
        float ndotl = max(dot(worldNormal, normalize(uLightDir)), 0.0);
        vLight = uAmbientColor + vec3(ndotl);

        for (int i = 0; i < uPointLightCount; i++) {
            vec3 toLight = uPointLightPos[i] - worldPos.xyz;
            float dist = length(toLight);
            float atten = clamp(1.0 - dist / max(uPointLightRange[i], 0.001), 0.0, 1.0);
            float pointNdotl = max(dot(worldNormal, normalize(toLight)), 0.0);
            vLight += uPointLightColor[i] * uPointLightIntensity[i] * pointNdotl * atten;
        }

        // Baked static-light contribution — precomputed once per vertex by
        // `Sandbox::bake_static_lighting` (Rust-side, same N-dot-L +
        // attenuation formula as the dynamic loop above, just summed over
        // every `is_static` light with no 4-light array limit). A never-baked
        // mesh has `aColor == 0`, a pure no-op.
        vLight += aColor;
    } else {
        vLight = vec3(1.0);
    }

    float viewDist = length(viewPos.xyz);
    vFogFactor = clamp((viewDist - uFogStart) / max(uFogEnd - uFogStart, 0.001), 0.0, 1.0);
}
