#version 450
layout(location = 0) in uint data_lo;
layout(location = 1) in uint data_hi;

layout(location = 0) out vec2 fragUV;
layout(location = 1) out flat uint fragTexId;
layout(location = 2) out flat uint fragAtlasTiles;
layout(location = 3) out flat uint fragFace;
layout(location = 4) out float fragAO;

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
  uint u    = (data_lo >> 21u) & 0x3Fu;
  uint v    = (data_lo >> 27u) & 0x1Fu;

  uint tex = data_hi & 0xFFFFu;
  uint ao  = (data_hi >> 16u) & 0x3u;

  fragUV = vec2(float(u), float(v));
  fragTexId = tex;
  fragAtlasTiles = pc.atlas_tiles;
  fragFace = face;
  fragAO = float(ao) / 3.0;

  vec3 world_offset = vec3(pc.world_pos);
  gl_Position = pc.proj_view * vec4(float(x) + world_offset.x,
                                     float(y) + world_offset.y,
                                     float(z) + world_offset.z, 1.0);
}
