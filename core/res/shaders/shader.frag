#version 450

layout(location = 0) in vec3 fragNormal;
layout(location = 1) in vec2 fragTexCoord;
layout(location = 2) in vec3 fragWorldPos;

#define LIGHT_DIRECTIONAL 0u
#define LIGHT_POINT       1u
#define LIGHT_SPOT        2u

struct GpuLight {
    vec4  position;    // offset  0
    vec4  direction;   // offset 16
    vec3  color;       // offset 32
    float intensity;   // offset 44
    uint  light_type;  // offset 48
    float radius;      // offset 52
    float angle_cos;   // offset 56
    float length;      // offset 60
};

layout(set = 0, binding = 0, std430) readonly buffer LightBuffer {
    uint     count;
    uint     shadow_enabled;           // 0=off, 1=spot, 2=directional CSM
    uint     cascade_count;
    float    shadow_distance;
    vec4     camera_world_pos;
    vec4     camera_world_dir;
    mat4     light_space[4];
    float    cascade_splits[4];
    uint     shadow_light_index;       // which light casts the directional/spot shadow
    uint     point_shadow_enabled;     // 0=off, 1=enabled
    uint     point_shadow_light_index; // which light casts the point shadow
    float    point_shadow_far;
    GpuLight lights[];
} light_buf;

layout(set = 0, binding = 1) uniform sampler2DArrayShadow shadowMap;
layout(set = 0, binding = 2) uniform samplerCubeShadow pointShadowMap;

layout(location = 0) out vec4 outColor;

float attenuate(float dist, float radius) {
    float r = dist / radius;
    return clamp(1.0 - r * r, 0.0, 1.0);
}

float compute_shadow(vec3 worldPos, vec3 N) {
    if (light_buf.shadow_enabled == 0u) return 0.0;

    int cascade = 0;
    if (light_buf.shadow_enabled == 2u) {
        // Directional CSM: select cascade by view-space depth.
        float depth = dot(worldPos - light_buf.camera_world_pos.xyz, light_buf.camera_world_dir.xyz);
        if (depth <= 0.0) return 0.0;
        cascade = int(light_buf.cascade_count) - 1;
        for (int i = 0; i < int(light_buf.cascade_count); i++) {
            if (depth < light_buf.cascade_splits[i]) { cascade = i; break; }
        }
    }
    // For spot (shadow_enabled == 1u), cascade stays at 0.

    vec4 sc = light_buf.light_space[cascade] * vec4(worldPos, 1.0);
    sc.xyz /= sc.w;
    sc.xy = sc.xy * 0.5 + 0.5;
    if (sc.z > 1.0 || sc.z < 0.0 || sc.x < 0.0 || sc.x > 1.0 || sc.y < 0.0 || sc.y > 1.0)
        return 0.0;
    // Slope-scaled bias: larger bias on surfaces oblique to the light to prevent acne.
    vec3 L = light_buf.shadow_enabled == 2u
        ? normalize(-light_buf.camera_world_dir.xyz)
        : normalize(light_buf.lights[light_buf.shadow_light_index].position.xyz - worldPos);
    float cosTheta = clamp(dot(N, L), 0.0, 1.0);
    float bias = mix(0.005, 0.0005, cosTheta);
    return 1.0 - texture(shadowMap, vec4(sc.xy, float(cascade), sc.z - bias));
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
    vec3 result = vec3(0.15);
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
            shadow = compute_shadow(fragWorldPos, N);
        } else if (i == light_buf.point_shadow_light_index && light_buf.point_shadow_enabled != 0u) {
            shadow = compute_point_shadow(fragWorldPos, N);
        }
        result += light.color * light.intensity * diff * atten * (1.0 - shadow);
    }
    return result;
}

void main() {
    vec3 N     = normalize(fragNormal);
    vec3 light = compute_lighting(N);
    outColor   = vec4(vec3(0.8) * light, 1.0);
}
