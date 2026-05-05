#version 450
layout(location = 0) in vec2 fragUV;
layout(location = 1) in flat uint fragTexId;
layout(location = 2) in flat uint fragAtlasTiles;
layout(location = 3) in flat uint fragFace;
layout(location = 4) in float fragAO;

layout(binding = 0) uniform sampler2D atlas;
layout(location = 0) out vec4 outColor;

void main() {
  float tile_size = 1.0 / float(fragAtlasTiles);
  uint tx = fragTexId % fragAtlasTiles;
  uint ty = fragTexId / fragAtlasTiles;

  vec2 local_uv;
  if (fragFace == 0u) {
    local_uv = vec2(fragUV.y, 1.0 - fragUV.x);
  } else if (fragFace == 1u) {
    local_uv = vec2(1.0 - fragUV.y, 1.0 - fragUV.x);
  } else if (fragFace == 2u) {
    local_uv = vec2(fragUV.x, fragUV.y);
  } else if (fragFace == 3u) {
    local_uv = vec2(fragUV.x, fragUV.y);
  } else if (fragFace == 4u) {
    local_uv = vec2(fragUV.y, 1.0 - fragUV.x);
  } else {
    local_uv = vec2(fragUV.y, 1.0 - fragUV.x);
  }

  vec2 uv = vec2(float(tx), float(ty)) * tile_size + local_uv * tile_size;

  float shade = 1.0;
  if (fragFace == 2u)                   shade = 1.0;
  if (fragFace == 0u || fragFace == 1u) shade = 0.8;
  if (fragFace == 4u || fragFace == 5u) shade = 0.7;
  if (fragFace == 3u)                   shade = 0.4;

  vec4 color = texture(atlas, uv);

  if (color.a < 0.5) discard;
float ao = mix(0.1, 1.0, pow(fragAO, 3.0));
  outColor = vec4(color.rgb * shade * ao, color.a);
}
