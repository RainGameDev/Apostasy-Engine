#version 450

layout(location = 0) in vec2 fragUV;
layout(location = 1) flat in uint fragTexId;
layout(location = 2) flat in uint fragAtlasTiles;
layout(location = 3) flat in uint fragFace;
layout(location = 4) in float fragAO;
layout(location = 5) in vec3 fragTint;
layout(location = 6) in vec3 fragWorldPos;
layout(location = 7) in vec3 fragWorldNormal;

layout(set = 0, binding = 0) uniform sampler2D atlas;

#define LIGHT_DIRECTIONAL 0u
#define LIGHT_POINT       1u
#define LIGHT_SPOT        2u

struct GpuLight {
  vec4  position;
  vec4  direction;
  vec3  color;
  float intensity;
  uint  light_type;
  float radius;
  float angle_cos;
  float length;
};

layout(set = 1, binding = 0, std430) readonly buffer LightBuffer {
  uint     count;
  uint     _pad0;
  uint     _pad1;
  uint     _pad2;
  GpuLight lights[];
} light_buf;

layout(location = 0) out vec4 outColor;

float attenuate(float dist, float radius) {
  float r = dist / radius;
  return clamp(1.0 - r * r, 0.0, 1.0);
}

vec3 compute_lighting(vec3 N) {
  vec3 result = vec3(0.15);
  for (uint i = 0u; i < light_buf.count; i++) {
    GpuLight light = light_buf.lights[i];
    vec3  L;
    float atten = 1.0;

    if (light.light_type == LIGHT_DIRECTIONAL) {
      L = normalize(-light.direction.xyz);

    } else if (light.light_type == LIGHT_POINT) {
      vec3  to_light = light.position.xyz - fragWorldPos;
      float dist     = length(to_light);
      if (dist >= light.radius) continue;
      L     = to_light / dist;
      atten = attenuate(dist, light.radius);

    } else {
      vec3  to_light  = light.position.xyz - fragWorldPos;
      float dist      = length(to_light);
      if (dist >= light.radius) continue;
      L               = to_light / dist;
      float spot_cos  = dot(-L, normalize(light.direction.xyz));
      if (spot_cos < light.angle_cos) continue;
      atten = attenuate(dist, light.radius)
            * smoothstep(light.angle_cos, light.angle_cos + 0.05, spot_cos);
    }

    float diff = max(dot(N, L), 0.0);
    result += light.color * light.intensity * diff * atten;
  }
  return result;
}

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
  vec4 color = texture(atlas, uv);

  float maxC = max(color.r, max(color.g, color.b));
  float minC = min(color.r, min(color.g, color.b));
  float saturation = (maxC < 0.001) ? 0.0 : (maxC - minC) / maxC;
  if (saturation < 0.5) {
    color.rgb *= fragTint;
  }

  float ao    = mix(0.1, 1.0, pow(fragAO, 3.0));
  vec3  light = compute_lighting(fragWorldNormal);
  outColor = vec4(color.rgb * light * ao, 0.5);
}
