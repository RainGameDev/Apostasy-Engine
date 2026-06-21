#version 450
layout(location = 0) in vec3 inPosition;

layout(push_constant) uniform PushConstants {
    mat4 light_space;
    mat4 model;
    vec3 pos;
    vec3 scale;
    vec4 rotation;
} pc;

void main() {
    gl_Position = pc.light_space * vec4(inPosition, 1.0);
}
