#version 450
layout(location = 0) in vec3 inPosition;
layout(location = 1) in vec3 inNormal;
layout(location = 2) in vec2 inTexCoord;

layout(push_constant) uniform Push {
    vec3  light_pos;
    float far;
    mat4  light_space;
    vec3  pos;
    float _pad0;
    vec3  scale;
    float _pad1;
    vec4  rotation;
} pc;

layout(location = 0) out vec3 outWorldPos;

vec3 applyQuaternion(vec4 q, vec3 v) {
    vec3 qv = vec3(q.x, q.y, q.z);
    return v + 2.0 * cross(qv, cross(qv, v) + q.w * v);
}

void main() {
    vec3 scaled  = inPosition * pc.scale;
    vec3 rotated = applyQuaternion(pc.rotation, scaled);
    vec3 world   = rotated + pc.pos;
    outWorldPos  = world;
    gl_Position  = pc.light_space * vec4(world, 1.0);
}
