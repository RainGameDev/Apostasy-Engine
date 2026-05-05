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
} pc;

layout(location = 0) out vec3 fragNormal;
layout(location = 1) out vec2 fragTexCoord;


vec3 applyQuaternion(vec4 q, vec3 v) {
    vec3 qv = vec3(q.x, q.y, q.z);
    return v + 2.0 * cross(qv, cross(qv, v) + q.w * v);
}

void main() {
    vec3 scale = vec3(pc.scale.x, pc.scale.y, pc.scale.z);
    vec3 scaledPosition = inPosition * scale;
    vec3 rotatedPosition = applyQuaternion(pc.rotation, scaledPosition);
    vec3 offset = vec3(pc.pos.x, pc.pos.y, pc.pos.z);
    vec3 worldPos = rotatedPosition + offset;

    gl_Position = pc.mvp * vec4(worldPos, 1.0);
    fragNormal = normalize(mat3(transpose(inverse(pc.model))) * inNormal);
    fragTexCoord = inTexCoord;
}


