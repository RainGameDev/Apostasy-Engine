#version 450

layout(location = 0) in vec3 fragNormal;
layout(location = 1) in vec2 fragTexCoord;
layout(location = 2) in vec3 fragWorldPos;
layout(location = 3) in vec4 fragWeights0;
layout(location = 4) in vec4 fragWeights1;
layout(location = 5) in vec4 fragWeights2;
layout(location = 6) in vec4 fragWeights3;
layout(location = 7) in vec4 fragWeights4;
layout(location = 8) in vec4 fragWeights5;
layout(location = 9) in vec4 fragWeights6;
layout(location = 10) in vec4 fragWeights7;
layout(location = 11) in vec3 fragColor;

layout(set = 1, binding = 0) uniform sampler2DArray terrainTex;

layout(push_constant) uniform PushConstants {
    mat4 mvp;
    mat4 model;
    vec3 pos;
    vec3 scale;
    vec4 rotation;
    vec4 colorModifier;
    // 32 layer IDs packed as 4 u8s per uint (little-endian).
    // Layer i: (activeLayerIdsPacked[i/4] >> ((i%4)*8)) & 0xFF
    uint activeLayerIdsPacked[8];
    uint layerCount;
} pc;

#define LIGHT_DIRECTIONAL 0u
#define LIGHT_POINT       1u
#define LIGHT_SPOT        2u

struct GpuLight {
    vec4  position;
    vec4  direction;
    vec3  color;
    float intensity;
    uint  light_type;
    float radius;
    float angle_cos;
    float length;
};

layout(set = 0, binding = 0, std430) readonly buffer LightBuffer {
    uint     count;
    uint     shadow_enabled;
    uint     cascade_count;
    float    shadow_distance;
    vec4     camera_world_pos;
    vec4     camera_world_dir;
    mat4     light_space[4];
    float    cascade_splits[4];
    uint     shadow_light_index;
    uint     point_shadow_enabled;
    uint     point_shadow_light_index;
    float    point_shadow_far;
    GpuLight lights[];
} light_buf;

layout(set = 0, binding = 1) uniform sampler2DArrayShadow shadowMap;
layout(set = 0, binding = 2) uniform samplerCubeShadow pointShadowMap;

layout(location = 0) out vec4 outColor;

float hash21(vec2 p) {
    p = fract(p * vec2(234.34, 435.345));
    p += dot(p, p + 34.23);
    return fract(p.x * p.y);
}

float value_noise(vec2 p) {
    vec2 i = floor(p);
    vec2 f = fract(p);
    f = f * f * (3.0 - 2.0 * f);
    return mix(
        mix(hash21(i), hash21(i + vec2(1.0, 0.0)), f.x),
        mix(hash21(i + vec2(0.0, 1.0)), hash21(i + vec2(1.0, 1.0)), f.x),
        f.y
    );
}

float attenuate(float dist, float radius) {
    float r = dist / radius;
    return clamp(1.0 - r * r, 0.0, 1.0);
}

float compute_shadow(vec3 worldPos) {
    if (light_buf.shadow_enabled == 0u) return 0.0;

    int cascade = 0;
    if (light_buf.shadow_enabled == 2u) {
        float depth = dot(worldPos - light_buf.camera_world_pos.xyz, light_buf.camera_world_dir.xyz);
        if (depth <= 0.0) return 0.0;
        cascade = int(light_buf.cascade_count) - 1;
        for (int i = 0; i < int(light_buf.cascade_count); i++) {
            if (depth < light_buf.cascade_splits[i]) { cascade = i; break; }
        }
    }

    vec4 sc = light_buf.light_space[cascade] * vec4(worldPos, 1.0);
    sc.xyz /= sc.w;
    sc.xy = sc.xy * 0.5 + 0.5;
    if (sc.z > 1.0 || sc.z < 0.0 || sc.x < 0.0 || sc.x > 1.0 || sc.y < 0.0 || sc.y > 1.0)
        return 0.0;
    return 1.0 - texture(shadowMap, vec4(sc.xy, float(cascade), sc.z));
}

float compute_point_shadow(vec3 worldPos, vec3 N) {
    if (light_buf.point_shadow_enabled == 0u) return 0.0;
    GpuLight light = light_buf.lights[light_buf.point_shadow_light_index];
    vec3 d = worldPos - light.position.xyz;
    float dist = length(d);
    float currentDepth = dist / light_buf.point_shadow_far;
    if (currentDepth > 1.0) return 0.0;
    vec3 L = -d / dist;
    float cosTheta = clamp(dot(N, L), 0.0, 1.0);
    float bias = mix(0.08, 0.005, cosTheta) / light_buf.point_shadow_far;
    return 1.0 - texture(pointShadowMap, vec4(d, currentDepth - bias));
}

vec3 compute_lighting(vec3 N) {
    vec3 result = vec3(0.0);
    for (uint i = 0u; i < light_buf.count; i++) {
        GpuLight light = light_buf.lights[i];
        vec3  L;
        float atten = 1.0;

        if (light.light_type == LIGHT_DIRECTIONAL) {
            L = normalize(-light.direction.xyz);
        } else if (light.light_type == LIGHT_POINT) {
            vec3  to_light = light.position.xyz - fragWorldPos;
            float dist     = length(to_light);
            if (dist >= light.radius) continue;
            L     = to_light / dist;
            atten = attenuate(dist, light.radius);
        } else {
            vec3  to_light  = light.position.xyz - fragWorldPos;
            float dist      = length(to_light);
            if (dist >= light.radius) continue;
            L               = to_light / dist;
            float spot_cos  = dot(-L, normalize(light.direction.xyz));
            if (spot_cos < light.angle_cos) continue;
            atten = attenuate(dist, light.radius)
                  * smoothstep(light.angle_cos, light.angle_cos + 0.05, spot_cos);
        }

        float diff   = max(dot(N, L), 0.0);
        float shadow = 0.0;
        if (i == light_buf.shadow_light_index && light_buf.shadow_enabled != 0u) {
            shadow = compute_shadow(fragWorldPos);
        } else if (i == light_buf.point_shadow_light_index && light_buf.point_shadow_enabled != 0u) {
            shadow = compute_point_shadow(fragWorldPos, N);
        }
        result += light.color * light.intensity * diff * atten * (1.0 - shadow);
    }
    return result;
}

void main() {
    vec3 N = normalize(fragNormal);
    vec3 lighting = compute_lighting(N);

    // World-space UVs: tile 16 times per cell (128 world units).
    float tileScale = 16.0 / 128.0;
    vec2 uv = fragWorldPos.xz * tileScale;

    // Unpack 32 blend weights from the 8 vec4 inputs.
    float weights[32] = float[](
        fragWeights0.x, fragWeights0.y, fragWeights0.z, fragWeights0.w,
        fragWeights1.x, fragWeights1.y, fragWeights1.z, fragWeights1.w,
        fragWeights2.x, fragWeights2.y, fragWeights2.z, fragWeights2.w,
        fragWeights3.x, fragWeights3.y, fragWeights3.z, fragWeights3.w,
        fragWeights4.x, fragWeights4.y, fragWeights4.z, fragWeights4.w,
        fragWeights5.x, fragWeights5.y, fragWeights5.z, fragWeights5.w,
        fragWeights6.x, fragWeights6.y, fragWeights6.z, fragWeights6.w,
        fragWeights7.x, fragWeights7.y, fragWeights7.z, fragWeights7.w
    );

    // Accumulate weighted texture samples from all active layers.
    vec3  albedo = vec3(0.0);
    float weightSum = 0.0;

    uint count = max(pc.layerCount, 1u);
    for (uint i = 0u; i < count; i++) {
        float w = weights[i];
        if (w <= 0.001) continue;

        // Unpack layer ID: 4 u8 IDs per uint, little-endian.
        uint layerId = (pc.activeLayerIdsPacked[i / 4u] >> ((i % 4u) * 8u)) & 0xFFu;

        albedo += texture(terrainTex, vec3(uv, float(layerId))).rgb * w;
        weightSum += w;
    }

    // Normalize to guard against floating-point drift.
    vec4 baseColor = vec4(albedo / max(weightSum, 0.0001), 1.0);

    vec3 ambient = vec3(0.05);
    outColor = vec4(baseColor.rgb * fragColor * (lighting + ambient), 1.0);
}
