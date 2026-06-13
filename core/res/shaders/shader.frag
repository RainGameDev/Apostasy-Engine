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
    uint     _pad0;
    uint     _pad1;
    uint     _pad2;
    GpuLight lights[];
} light_buf;

layout(location = 0) out vec4 outColor;

float attenuate(float dist, float radius) {
    float r = dist / radius;
    return clamp(1.0 - r * r, 0.0, 1.0);
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

        float diff = max(dot(N, L), 0.0);
        result += light.color * light.intensity * diff * atten;
    }
    return result;
}

void main() {
    vec3 N     = normalize(fragNormal);
    vec3 light = compute_lighting(N);
    outColor   = vec4(vec3(0.8) * light, 1.0);
}
