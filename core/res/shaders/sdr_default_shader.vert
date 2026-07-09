#version 450
layout(location = 0) in vec3 inPosition;
layout(location = 1) in vec3 inNormal;
layout(location = 2) in vec2 inTexCoord;
layout(location = 12) in vec4 inTangent;

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
layout(location = 3) out vec3 fragTangent;
layout(location = 4) out vec3 fragBitangent;


vec3 applyQuaternion(vec4 q, vec3 v) {
    vec3 qv = vec3(q.x, q.y, q.z);
    return v + 2.0 * cross(qv, cross(qv, v) + q.w * v);
}
void main() {
    vec3 scale = pc.scale;
    vec3 scaledPosition = inPosition * scale;
    vec3 rotatedPosition = applyQuaternion(pc.rotation, scaledPosition);
    vec3 worldPos = rotatedPosition + pc.pos;

    // Normal: rotate by same quaternion, but divide by scale (inverse-transpose of R*S)
    vec3 N = normalize(applyQuaternion(pc.rotation, inNormal / scale));

    // Tangent: directional vector, transforms like position (scale then rotate), no inverse
    vec3 T = normalize(applyQuaternion(pc.rotation, inTangent.xyz * scale));
    T = normalize(T - dot(T, N) * N); // Gram-Schmidt against N
    vec3 B = cross(N, T) * inTangent.w;

    gl_Position   = pc.mvp * vec4(worldPos, 1.0);
    fragNormal    = N;
    fragTexCoord  = inTexCoord;
    fragWorldPos  = worldPos;
    fragTangent   = T;
    fragBitangent = B;
}
