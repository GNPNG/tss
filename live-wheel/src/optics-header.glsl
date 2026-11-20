#version 450
layout(location=0) in vec2 fieldUV;
layout(location=0) out vec4 result;
layout(set=0,binding=0) uniform texture2D field;
layout(set=0,binding=1) uniform sampler fieldSampler;
layout(set=0,binding=2,std140) uniform Material {
    vec4 rear;
    vec4 front;
    vec4 joint;
    vec4 rim;
    vec4 carrier;
    vec4 lens;
} mat;
#define TSS_LIVE_FOCUS 1
#define TSS_PROGRAM_LINEAR (mat.lens.y > 0.5)
#define liveFocus (mat.lens.x)
#define HOOKED_tex(point) textureLod(sampler2D(field,fieldSampler),point,0.0)
#define HOOKED_pos fieldUV
