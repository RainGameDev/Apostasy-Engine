#version 450
layout(location = 0) in uint data_lo;
layout(location = 1) in uint data_hi;
layout(location = 2) in uint tint;

layout(push_constant) uniform Push {
    mat4  light_space;
    ivec3 world_pos;
    int   _pad;
} pc;

void main() {
    uint x = (data_lo >> 0u)  & 0x3Fu;
    uint y = (data_lo >> 6u)  & 0x3Fu;
    uint z = (data_lo >> 12u) & 0x3Fu;

    vec3 world = vec3(float(x) + float(pc.world_pos.x),
                      float(y) + float(pc.world_pos.y),
                      float(z) + float(pc.world_pos.z));
    gl_Position = pc.light_space * vec4(world, 1.0);
}
