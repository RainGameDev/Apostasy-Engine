#version 450
layout(set = 1, binding = 0) uniform sampler2D skyMap;

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

layout(location = 0) in vec2 fragTexCoord;
layout(location = 0) out vec4 outColor;
layout(location = 1) in vec3 fragDir;


vec3 applyQuaternion(vec4 q, vec3 v) {
    vec3 qv = vec3(q.x, q.y, q.z);
    return v + 2.0 * cross(qv, cross(qv, v) + q.w * v);
}

void main() {
  vec2 uv = fragTexCoord;
  float horizonFade = 1.0;

  if (pc.projection == 1u) {
    vec3 dir = normalize(fragDir);
    vec3 wdir = normalize(applyQuaternion(pc.rotation, dir)); 
    float uvScale = uintBitsToFloat(pc.layerData[0].x);
    uv = dir.xz / (1.0 + abs(dir.y)) * uvScale + 0.5;
    horizonFade = smoothstep(-0.05, 0.25, wdir.y);
  }
  else if (pc.projection == 2u) {
    vec3 dir = normalize(fragDir);
    float uvScale = uintBitsToFloat(pc.layerData[0].x);
    uv = dir.xy / (1.0 + max(dir.z, -0.99)) * uvScale + 0.5;
    horizonFade = smoothstep(0.0, 0.2, dir.z);
  }
  vec4 tex = texture(skyMap, uv);
  outColor = vec4(tex.rgb * pc.colorModifier.rgb, tex.a * pc.colorModifier.a * horizonFade);

}
