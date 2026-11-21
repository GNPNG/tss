#include <OpenCL/opencl.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#define CL_OK(call)                                                            \
  do {                                                                         \
    cl_int rc = (call);                                                        \
    if (rc) {                                                                  \
      fprintf(stderr, "%s returned %d\n", #call, rc);                          \
      return 1;                                                                \
    }                                                                          \
  } while (0)
enum {
  nx = 41,
  pitch = 48,
  height = 7,
  sites = pitch * height,
  directions = 9,
  words = sites * directions
};
int main(int argc, char **argv) {
  if (argc != 3) {
    fprintf(stderr, "Usage: marker-test FILE.cl EXACT-DEVICE-NAME\n");
    return 2;
  }
  FILE *src = fopen(argv[1], "rb");
  if (!src || fseek(src, 0, SEEK_END))
    return 2;
  long length = ftell(src);
  if (length <= 0)
    return 2;
  size_t len = (size_t)length;
  rewind(src);
  char *text = malloc(len + 1);
  if (!text || fread(text, 1, len, src) != len)
    return 2;
  text[len] = 0;
  fclose(src);
  cl_platform_id platform;
  cl_device_id device = 0;
  cl_uint count;
  char name[512];
  CL_OK(clGetPlatformIDs(1, &platform, &count));
  if (count != 1)
    return 2;
  cl_device_id devices[16];
  CL_OK(clGetDeviceIDs(platform, CL_DEVICE_TYPE_GPU, 16, devices, &count));
  for (cl_uint i = 0; i < count && i < 16; i++) {
    CL_OK(clGetDeviceInfo(devices[i], CL_DEVICE_NAME, sizeof(name), name, 0));
    if (!strcmp(name, argv[2]))
      device = devices[i];
  }
  if (!device) {
    fprintf(stderr, "OpenCL GPU not found: %s\n", argv[2]);
    return 2;
  }
  CL_OK(clGetDeviceInfo(device, CL_DEVICE_NAME, sizeof(name), name, 0));
  printf("%s\n", name);
  cl_int error;
  cl_context ctx = clCreateContext(0, 1, &device, 0, 0, &error);
  CL_OK(error);
  cl_command_queue q = clCreateCommandQueue(ctx, device, 0, &error);
  CL_OK(error);
  const char *str = text;
  cl_program p = clCreateProgramWithSource(ctx, 1, &str, &len, &error);
  CL_OK(error);
  if (clBuildProgram(p, 1, &device, "-cl-std=CL1.2", 0, 0)) {
    size_t size = 0;
    CL_OK(clGetProgramBuildInfo(p, device, CL_PROGRAM_BUILD_LOG, 0, NULL, &size));
    char *log = calloc(size + 1, 1);
    if (!log)
      return 2;
    CL_OK(clGetProgramBuildInfo(p, device, CL_PROGRAM_BUILD_LOG, size, log, NULL));
    fprintf(stderr, "%s\n", log);
    free(log);
    return 1;
  }
  int map[sites] = {0};
  const int excluded = 3 * pitch + 27, prop_only = 3 * pitch + 26;
  map[excluded] = 1;
  map[prop_only] = 2;
  cl_mem m = clCreateBuffer(ctx, CL_MEM_READ_ONLY | CL_MEM_COPY_HOST_PTR,
                            sizeof(map), map, &error);
  CL_OK(error);
  float out[words];
  cl_mem f = clCreateBuffer(ctx, CL_MEM_READ_WRITE, sizeof(out), 0, &error);
  CL_OK(error);
  size_t group[2] = {8, 1}, all[2] = {pitch, height};
  const int opposite[9] = {0, 3, 4, 1, 2, 7, 8, 5, 6};
  const int dx[9] = {0, 1, 0, -1, 0, 1, -1, -1, 1},
            dy[9] = {0, 0, 1, 0, -1, 1, 1, -1, -1};
  for (int j = 0; j < 3; j++) {
    char kernel_name[15];
    snprintf(kernel_name, sizeof(kernel_name), "Probe%d", j);
    cl_kernel k = clCreateKernel(p, kernel_name, &error);
    CL_OK(error);
    size_t lim[3], km, pref, fixed[3];
    CL_OK(clGetDeviceInfo(device, CL_DEVICE_MAX_WORK_ITEM_SIZES, sizeof(lim),
                          lim, 0));
    CL_OK(clGetKernelWorkGroupInfo(k, device, CL_KERNEL_WORK_GROUP_SIZE,
                                   sizeof(km), &km, 0));
    CL_OK(clGetKernelWorkGroupInfo(k, device, CL_KERNEL_COMPILE_WORK_GROUP_SIZE,
                                   sizeof(fixed), fixed, 0));
    CL_OK(clGetKernelWorkGroupInfo(k, device,
                                   CL_KERNEL_PREFERRED_WORK_GROUP_SIZE_MULTIPLE,
                                   sizeof(pref), &pref, 0));
    printf("limit %zu,%zu,%zu kernel %zu preferred %zu required %zu,%zu,%zu\n",
           lim[0], lim[1], lim[2], km, pref, fixed[0], fixed[1], fixed[2]);
    CL_OK(clSetKernelArg(k, 0, sizeof(m), &m));
    CL_OK(clSetKernelArg(k, 1, sizeof(f), &f));
    for (unsigned iteration = 0; iteration < 2; iteration++) {
      for (int i = 0; i < words; i++)
        out[i] = -54321.0f;
      CL_OK(clEnqueueWriteBuffer(q, f, CL_TRUE, 0, sizeof(out), out, 0, 0, 0));
      CL_OK(clSetKernelArg(k, 2, sizeof(iteration), &iteration));
      CL_OK(clEnqueueNDRangeKernel(q, k, 2, 0, all, group, 0, 0, 0));
      CL_OK(clEnqueueReadBuffer(q, f, CL_TRUE, 0, sizeof(out), out, 0, 0, 0));
      for (int d = 0; d < directions; d++)
        for (int i = 0; i < sites; i++) {
          const int x = i % pitch, y = i / pitch;
          // Every padded lane retains its marker. Odd-step interior populations
          // must come from the neighbouring site in the recorded direction.
          float expected;
          if (x >= nx || (iteration == 0 && (i == excluded || i == prop_only)))
            expected = -54321.0f;
          else if (iteration == 0)
            expected = (opposite[d] + 1) * 1000 + i;
          else if (x >= 4 && x < 19 && y >= 1 && y < height - 1)
            expected = (d + 1) * 1000 + i - dx[d] - dy[d] * pitch;
          else
            continue;
          if (out[d * sites + i] != expected) {
            fprintf(
                stderr,
                "kernel=%d parity=%u x=%d y=%d d=%d expected %.0f got %.0f\n",
                j, iteration, x, y, d, expected, out[d * sites + i]);
            return 1;
          }
        }
      printf("%s parity %u passed\n", kernel_name, iteration);
    }
    clReleaseKernel(k);
  }
  clReleaseMemObject(m);
  clReleaseMemObject(f);
  clReleaseProgram(p);
  clReleaseCommandQueue(q);
  clReleaseContext(ctx);
  free(text);
}
