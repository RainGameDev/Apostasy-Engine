#version 450

layout(push_constant) uniform Push {
    vec3  light_pos;
    float far;
} pc;

layout(location = 0) in vec3 fragWorldPos;

void main() {
    gl_FragDepth = length(fragWorldPos - pc.light_pos) / pc.far;
}
