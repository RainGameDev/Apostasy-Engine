#version 450
layout(location = 0) in uint data_lo;
layout(location = 1) in uint data_hi;
layout(location = 2) in uint tint;

layout(location = 0) out vec2 fragUV;
layout(location = 1) out flat uint fragTexId;
layout(location = 2) out flat uint fragAtlasTiles;
layout(location = 3) out flat uint fragFace;
layout(location = 4) out float fragAO;
layout(location = 5) out vec3 fragTint;
layout(location = 6) out vec3 fragWorldPos;
layout(location = 7) out vec3 fragWorldNormal;

layout(push_constant) uniform Push {
  mat4 proj_view;
  mat4 model;
  uint atlas_tiles;
  uint _pad0;
  uint _pad1;
  uint _pad2;
  ivec3 world_pos;
} pc;

void main() {
  uint x    = (data_lo >> 0u)  & 0x3Fu;
  uint y    = (data_lo >> 6u)  & 0x3Fu;
  uint z    = (data_lo >> 12u) & 0x3Fu;
  uint face = (data_lo >> 18u) & 0x7u;
  uint u    = (data_lo >> 21u) & 0x3u;
  uint v    = (data_lo >> 23u) & 0x3u;
  uint top  = (data_lo >> 25u) & 0x1u;


  uint tex = data_hi & 0xFFFFu;
  uint ao  = (data_hi >> 16u) & 0x3u;

  
  vec3 decoded = vec3(
      float((tint >> 0u) & 0xFu),
      float((tint >> 4u) & 0xFu),
      float((tint >> 8u) & 0xFu)
      ) / 15.0;

  fragTint = (tint == 0u) ? vec3(1.0) : decoded;
  fragUV = vec2(float(u), float(v));
  fragTexId = tex;
  fragAtlasTiles = pc.atlas_tiles;
  fragFace = face;
  fragAO = float(ao) / 3.0;

  vec3 faceNormals[6] = vec3[6](
    vec3( 1.0,  0.0,  0.0),
    vec3(-1.0,  0.0,  0.0),
    vec3( 0.0,  1.0,  0.0),
    vec3( 0.0, -1.0,  0.0),
    vec3( 0.0,  0.0,  1.0),
    vec3( 0.0,  0.0, -1.0)
  );
  fragWorldNormal = faceNormals[face];

  vec3 world_offset = vec3(pc.world_pos);
  fragWorldPos = vec3(float(x) + world_offset.x,
      float(y) + world_offset.y,
      float(z) + world_offset.z);
  gl_Position = pc.proj_view * vec4(fragWorldPos, 1.0);
}
