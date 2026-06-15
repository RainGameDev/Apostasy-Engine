#version 450
layout(location = 0) in vec3 inPosition;
layout(location = 1) in vec3 inNormal;
layout(location = 2) in vec2 inTexCoord;
layout(location = 3) in float inTextureIndex;

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
layout(location = 3) out float fragTextureIndex;

void main() {
    // Terrain vertices are in world space already (no transform applied).
    // pos/scale/rotation in push constants are identity.
    vec3 worldPos = inPosition;

    gl_Position  = pc.mvp * vec4(worldPos, 1.0);
    fragNormal   = normalize(inNormal);
    fragTexCoord = inTexCoord;
    fragWorldPos = worldPos;
    fragTextureIndex = inTextureIndex;
}
