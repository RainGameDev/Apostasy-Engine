#version 450
layout(location = 0) in vec3 inPosition;
layout(location = 1) in vec3 inNormal;
layout(location = 2) in vec2 inTexCoord;
layout(location = 3) in float inWeight0;
layout(location = 4) in float inWeight1;
layout(location = 5) in float inWeight2;
layout(location = 6) in float inWeight3;
layout(location = 7) in float inWeight4;
layout(location = 8) in float inWeight5;

layout(push_constant) uniform PushConstants {
    mat4 mvp;
    mat4 model;
    vec3 pos;
    vec3 scale;
    vec4 rotation;
} pc;

layout(location = 0) out vec3 fragNormal;
layout(location = 1) out vec2 fragTexCoord;
layout(location = 2) out vec3 fragWorldPos;
layout(location = 3) out float fragWeight0;
layout(location = 4) out float fragWeight1;
layout(location = 5) out float fragWeight2;
layout(location = 6) out float fragWeight3;
layout(location = 7) out float fragWeight4;
layout(location = 8) out float fragWeight5;

void main() {
    vec3 worldPos = inPosition;

    gl_Position  = pc.mvp * vec4(worldPos, 1.0);
    fragNormal   = normalize(inNormal);
    fragTexCoord = inTexCoord;
    fragWorldPos = worldPos;
    fragWeight0  = inWeight0;
    fragWeight1  = inWeight1;
    fragWeight2  = inWeight2;
    fragWeight3  = inWeight3;
    fragWeight4  = inWeight4;
    fragWeight5  = inWeight5;
}
