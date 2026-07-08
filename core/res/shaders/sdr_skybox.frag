#version 450
// Layer 0 = day sky, layer 1 = night sky (equirectangular).
layout(set = 1, binding = 0) uniform sampler2DArray skyMaps;

layout(push_constant) uniform PushConstants {
    mat4 mvp;
    mat4 model;
    vec3 pos;
    vec3 scale;
    vec4 rotation;
    vec4 colorModifier;
} pc;

layout(location = 0) in vec2 fragTexCoord;
layout(location = 0) out vec4 outColor;

void main() {
    vec3 day = texture(skyMaps, vec3(fragTexCoord, 0.0)).rgb;
    vec3 night = texture(skyMaps, vec3(fragTexCoord, 1.0)).rgb;
    float blend = clamp(pc.colorModifier.a, 0.0, 1.0);
    outColor = vec4(mix(day, night, blend) * pc.colorModifier.rgb, 1.0);
}
