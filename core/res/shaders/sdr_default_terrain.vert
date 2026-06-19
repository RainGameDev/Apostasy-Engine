#version 450
layout(location = 0) in vec3 inPosition;
layout(location = 1) in vec3 inNormal;
layout(location = 2) in vec2 inTexCoord;
layout(location = 3) in vec4 inWeights0;
layout(location = 4) in vec4 inWeights1;
layout(location = 5) in vec4 inWeights2;
layout(location = 6) in vec4 inWeights3;
layout(location = 7) in vec4 inWeights4;
layout(location = 8) in vec4 inWeights5;
layout(location = 9) in vec4 inWeights6;
layout(location = 10) in vec4 inWeights7;
layout(location = 11) in vec3 inColor;

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
layout(location = 3) out vec4 fragWeights0;
layout(location = 4) out vec4 fragWeights1;
layout(location = 5) out vec4 fragWeights2;
layout(location = 6) out vec4 fragWeights3;
layout(location = 7) out vec4 fragWeights4;
layout(location = 8) out vec4 fragWeights5;
layout(location = 9) out vec4 fragWeights6;
layout(location = 10) out vec4 fragWeights7;
layout(location = 11) out vec3 fragColor;

void main() {
    vec3 worldPos = inPosition;

    gl_Position  = pc.mvp * vec4(worldPos, 1.0);
    fragNormal   = normalize(inNormal);
    fragTexCoord = inTexCoord;
    fragWorldPos = worldPos;
    fragWeights0 = inWeights0;
    fragWeights1 = inWeights1;
    fragWeights2 = inWeights2;
    fragWeights3 = inWeights3;
    fragWeights4 = inWeights4;
    fragWeights5 = inWeights5;
    fragWeights6 = inWeights6;
    fragWeights7 = inWeights7;
    fragColor    = inColor;
}
