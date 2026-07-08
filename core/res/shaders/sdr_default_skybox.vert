#version 450
layout(location = 0) in vec3 inPosition;
layout(location = 1) in vec3 inNormal;
layout(location = 2) in vec2 inTexCoord;
layout(push_constant) uniform PushConstants {
    mat4 mvp;
    mat4 model;
    vec3 pos;
    vec3 scale;
    vec4 rotation;
    vec4 colorModifier;
    uvec4 layerData[2];   
    uint projection;     
} pc;

layout(location = 0) out vec2 fragTexCoord;
layout(location = 1) out vec3 fragDir;

vec3 applyQuaternion(vec4 q, vec3 v) {
    vec3 qv = vec3(q.x, q.y, q.z);
    return v + 2.0 * cross(qv, cross(qv, v) + q.w * v);
}

void main() {
    vec3 dir = applyQuaternion(pc.rotation, inPosition);
    vec4 pos = pc.mvp * vec4(dir, 0.0);
    gl_Position = pos.xyww;
    fragTexCoord = inTexCoord;
    fragDir = inPosition;

}

