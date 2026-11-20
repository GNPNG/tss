// Offscreen libplacebo hook runner for a stationary RGBA16 slide field.
// Build: cc -std=c11 -O2 focus-proof.c $(pkg-config --cflags --libs libplacebo) -o focus-proof
// Input is native-endian RGBA16. Output is tightly packed sRGB RGBA8.
#include <libplacebo/renderer.h>
#include <libplacebo/shaders/custom.h>
#include <libplacebo/vulkan.h>
#include <stdint.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>

static void fail(const char *message)
{
    fprintf(stderr, "%s\n", message);
    exit(EXIT_FAILURE);
}

static unsigned char *read_file(const char *path, size_t *length)
{
    FILE *file = fopen(path, "rb");
    if (!file || fseek(file, 0, SEEK_END)) fail("Cannot open proof input");
    long bytes = ftell(file);
    if (bytes <= 0 || fseek(file, 0, SEEK_SET)) fail("Empty proof input");
    *length = (size_t)bytes;
    unsigned char *data = malloc(*length);
    if (!data || fread(data, 1, *length, file) != *length) fail("Incomplete proof input");
    fclose(file);
    return data;
}

static int integer(const char *text, int upper)
{
    char *end;
    long n = strtol(text, &end, 10);
    if (*end || n < 1 || n > upper) fail("Invalid proof dimensions or frame count");
    return (int)n;
}

static struct pl_frame rgb_frame(pl_tex texture)
{
    return (struct pl_frame) {
        .num_planes = 1,
        .planes = {{ .texture = texture, .components = 3, .component_mapping = {0, 1, 2} }},
        .repr = pl_color_repr_rgb,
        .color = pl_color_space_srgb,
    };
}

int main(int argc, char **argv)
{
    if (argc != 7) fail("Usage: focus-proof W H fields.rgba16 sweep.hook frames frames.rgba");
    const int width = integer(argv[1], 8192), height = integer(argv[2], 8192);
    const int frames = integer(argv[5], 10000);
    const size_t pixels = (size_t)width * height;
    size_t field_bytes, hook_bytes;
    unsigned char *field = read_file(argv[3], &field_bytes);
    if (field_bytes != pixels * 8) fail("Expected 16-bit RGBA field data at the requested size");
    unsigned char *source = read_file(argv[4], &hook_bytes);
    pl_log log = pl_log_create(PL_API_VER, &(struct pl_log_params){
        .log_cb = pl_log_simple, .log_priv = stderr, .log_level = PL_LOG_WARN,
    });
    pl_vulkan vk = pl_vulkan_create(log, pl_vulkan_params(
        .device_name = "SwiftShader Device (Subzero)", .allow_software = true));
    if (!vk) fail("Could not open the SwiftShader Vulkan CPU device");
    PFN_vkGetPhysicalDeviceProperties get_props = (PFN_vkGetPhysicalDeviceProperties)
        vk->get_proc_addr(vk->instance, "vkGetPhysicalDeviceProperties");
    VkPhysicalDeviceProperties props;
    get_props(vk->phys_device, &props);
    if (props.deviceType != VK_PHYSICAL_DEVICE_TYPE_CPU) fail("Proof requires a CPU device");
    fprintf(stderr, "%s · libplacebo %s · %dx%d · %d frames\n",
            props.deviceName, pl_version(), width, height, frames);

    pl_gpu gpu = vk->gpu;
    pl_fmt in_fmt = pl_find_named_fmt(gpu, "rgba16");
    pl_fmt out_fmt = pl_find_named_fmt(gpu, "rgba8");
    if (!in_fmt || !out_fmt) fail("Missing RGBA16 or RGBA8 Vulkan texture format");
    pl_tex input = pl_tex_create(gpu, pl_tex_params(
        .w = width, .h = height, .format = in_fmt, .sampleable = true, .initial_data = field));
    pl_tex target = pl_tex_create(gpu, pl_tex_params(
        .w = width, .h = height, .format = out_fmt, .renderable = true, .host_readable = true));
    if (!input || !target) fail("Unable to allocate proof textures");
    free(field);
    const struct pl_hook *hook = pl_mpv_user_shader_parse(gpu, (const char *)source, hook_bytes);
    if (!hook) fail("libplacebo rejected the optics hook");
    free(source);
    struct pl_render_params params = pl_render_default_params;
    params.hooks = &hook;
    params.num_hooks = 1;
    params.dither_params = NULL;
    pl_renderer renderer = pl_renderer_create(log, gpu);
    const struct pl_frame src = rgb_frame(input), dst = rgb_frame(target);
    unsigned char *rgba = malloc(pixels * 4);
    FILE *output = fopen(argv[6], "wb");
    if (!renderer || !rgba || !output) fail("Unable to allocate renderer or open output");
    for (int frame = 0; frame < frames; frame++) {
        if (!pl_render_image(renderer, &src, &dst, &params) || pl_renderer_get_errors(renderer).errors)
            fail("Optical pass failed");
        if (!pl_tex_download(gpu, pl_tex_transfer_params(.tex = target, .ptr = rgba)))
            fail("Frame readback failed");
        if (fwrite(rgba, 4, pixels, output) != pixels) fail("Output closed early");
    }
    if (fclose(output)) fail("Could not flush last frame");
    free(rgba);
    pl_mpv_user_shader_destroy(&hook);
    pl_renderer_destroy(&renderer);
    pl_tex_destroy(gpu, &input);
    pl_tex_destroy(gpu, &target);
    pl_vulkan_destroy(&vk);
    pl_log_destroy(&log);
    return EXIT_SUCCESS;
}
