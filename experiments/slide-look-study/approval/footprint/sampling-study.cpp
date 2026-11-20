// Controlled rear-slide sampling study. All fixtures use little-endian byte
// order.
#include <Imath/half.h>
#include <algorithm>
#include <array>
#include <cmath>
#include <cstdio>
#include <cstdlib>
#include <cstring>
#include <fstream>
#include <stdexcept>
#include <string>
#include <vector>
#include <zlib.h>

constexpr int sw = 512, sh = 288, mw = 320, mh = 180, w = 640, h = 360;
constexpr float screenW = 1280.f, screenH = 720.f, alpha = 1.53f;
using V = std::array<float, 2>;
using Image = std::vector<float>;
struct Stencil {
  int i, j, k, l;
  float a, b;
};
Stencil stencil(float u, float v, int width, int height) {
  float x = std::clamp(u * width - .5f, 0.f, float(width - 1)),
        y = std::clamp(v * height - .5f, 0.f, float(height - 1));
  int i = int(x), j = int(y);
  return {i,     j,    std::min(i + 1, width - 1), std::min(j + 1, height - 1),
          x - i, y - j};
}
float blend(float a, float b, float t) { return a + t * (b - a); }
float lookup(const Image &im, int width, int height, float u, float v) {
  auto s = stencil(u, v, width, height);
  return blend(blend(im[s.i + s.j * width], im[s.k + s.j * width], s.a),
               blend(im[s.i + s.l * width], im[s.k + s.l * width], s.a), s.b);
}
void inflate(const char *path, void *destination, unsigned length) {
  gzFile file = gzopen(path, "rb");
  if (!file)
    throw std::runtime_error(std::string("Cannot open ") + path);
  bool exact = gzread(file, destination, length) == int(length) &&
               gzgetc(file) == -1 && gzeof(file);
  if (gzclose(file) != Z_OK || !exact)
    throw std::runtime_error(std::string("Bad gzip data or dimensions in ") +
                             path);
}
struct Study {
  Image sdf, uOffset, vOffset, legacy;
  Study(const char *cache, const char *path)
      : sdf(sw * sh), uOffset(mw * mh), vOffset(mw * mh), legacy(sw * sh) {
    std::vector<unsigned char> rear(sw * sh);
    inflate(cache, rear.data(), rear.size());
    for (int i = 0; i < sw * sh; i++)
      sdf[i] = (rear[i] / 255.f - .5f) * 96.f;
    if (strcmp(path, "-") != 0) {
      std::vector<float> m(2 * mw * mh);
      inflate(path, m.data(), m.size() * sizeof(float));
      for (int i = 0; i < mw * mh; i++) {
        uOffset[i] = m[2 * i];
        vOffset[i] = m[2 * i + 1];
      }
    }
    for (int y = 0; y < sh; y++)
      for (int x = 0; x < sw; x++) {
        float d = distance((x + .5f) / sw, (y + .5f) / sh);
        legacy[x + y * sw] = (float(Imath::half(d / 96.f + .5f)) - .5f) * 96.f;
      }
  }
  V mapped(float u, float v) const {
    return {u + lookup(uOffset, mw, mh, u, v),
            v + lookup(vOffset, mw, mh, u, v)};
  }
  float distance(float u, float v) const {
    V z = mapped(u, v);
    return lookup(sdf, sw, sh, z[0], z[1]);
  }
  static float transmit(float d) {
    constexpr float half = .65f * 1.46f;
    if (d < -half)
      return 1.f;
    if (d > half)
      return .21653567f;
    float t = (d + half) / (2 * half);
    return std::exp(-alpha * t * t * (3.f - 2.f * t));
  }
  float integrate(float x, float y, int n) const {
    float total = 0.f;
    for (int sy = 0; sy < n; sy++)
      for (int sx = 0; sx < n; sx++) {
        float u = (x + (sx + .5f) / n) / w, v = (y + (sy + .5f) / n) / h;
        total += transmit(distance(u, v));
      }
    return total / (n * n);
  }
  float sigma(float u, float v) const {
    auto a = mapped(u - .5f / w, v), b = mapped(u + .5f / w, v),
         c = mapped(u, v - .5f / h), d = mapped(u, v + .5f / h);
    float j00 = sw * (b[0] - a[0]), j10 = sh * (b[1] - a[1]),
          j01 = sw * (d[0] - c[0]), j11 = sh * (d[1] - c[1]);
    float p = j00 * j00 + j10 * j10, r = j01 * j01 + j11 * j11,
          q = j00 * j01 + j10 * j11;
    return std::sqrt(.5f * (p + r + std::hypot(p - r, 2 * q)));
  }
};
bool inRing(int x, int y) {
  float nx = ((x + .5f) / w - .5f) / .14f, ny = ((y + .5f) / h - .5f) / .21f;
  float r = nx * nx + ny * ny;
  return r >= .49f && r <= 1.69f;
}
struct Score {
  int n = 0, falseClear = 0, falseOpaque = 0;
  double ae = 0, sq = 0, bias = 0;
  float worst = 0;
  int wx = 0, wy = 0;
  void add(float test, float ref, int x, int y) {
    float delta = test - ref, absd = std::abs(delta);
    ++n;
    ae += absd;
    sq += delta * delta;
    bias += delta;
    if (absd > worst) {
      worst = absd;
      wx = x;
      wy = y;
    }
    if (test > .992165f && ref < .92165f)
      ++falseClear;
    if (test < .22437f && ref > .29488f)
      ++falseOpaque;
  }
  void put(const char *tag) const {
    if (n == 0) {
      printf("%s_n=0\n", tag);
      return;
    }
    printf("%s_n=%d mae=%.6f rms=%.6f bias=%+.6f worst=%.6f at=%d,%d "
           "falseClear=%d falseOpaque=%d\n",
           tag, n, ae / n, sqrt(sq / n), bias / n, worst, wx, wy, falseClear,
           falseOpaque);
  }
};
void writeFrame(const char *path, const Image &im) {
  std::ofstream out(path, std::ios::binary);
  out.write((char *)im.data(), im.size() * 4);
  if (!out)
    throw std::runtime_error("output");
}
int main(int argc, char **argv) {
  if (argc != 3 && argc != 4) {
    fprintf(stderr,
            "sampling-study rear.r8.gz offsets.rgf32.gz [image-prefix]\n");
    return 2;
  }
  Study study(argv[1], argv[2]);
  Image direct(w * h), area(w * h), adaptive(w * h), absSigma(w * h);
  Score currentScore, adaptedScore, stableScore, highScore;
  int active = 0, baseline = 0;
  for (int y = 66; y < 294; y++)
    for (int x = 193; x < 447; x++) {
      float u = (x + .5f) / w, v = (y + .5f) / h, d = study.distance(u, v),
            single = lookup(study.legacy, sw, sh, u, v);
      float old = Study::transmit(single), sig = study.sigma(u, v),
            width = .949f + 2.5f * (2.f + sig);
      bool filter = sig > 1.20f && (std::abs(d) < width ||
                                    std::abs(single) < width || single * d < 0);
      float hi = study.integrate(x, y, 16);
      float improved = filter ? study.integrate(x, y, 4) : old;
      int i = x + y * w;
      direct[i] = old;
      area[i] = hi;
      adaptive[i] = improved;
      absSigma[i] = sig;
      if (inRing(x, y)) {
        currentScore.add(old, hi, x, y);
        adaptedScore.add(improved, hi, x, y);
        highScore.add(study.integrate(x, y, 8), hi, x, y);
        active += filter;
        baseline += sig > 1.2;
        if (sig <= 1.2)
          stableScore.add(old, hi, x, y);
      }
    }
  currentScore.put("current");
  adaptedScore.put("adapted");
  highScore.put("8x8");
  stableScore.put("not_minified");
  std::vector<float> sigma;
  for (int y = 0; y < h; y++)
    for (int x = 0; x < w; x++)
      if (inRing(x, y))
        sigma.push_back(absSigma[x + y * w]);
  std::sort(sigma.begin(), sigma.end());
  printf(
      "minification=%d filtered=%d sigma50=%.5f sigma95=%.5f sigmaMax=%.5f\n",
      baseline, active, sigma[sigma.size() / 2],
      sigma[size_t(sigma.size() * .95)], sigma.back());
  // Fractional-area normalized image-space disk. radius comes from the live
  // thin lens at front focus.
  float rad =
      std::abs(8.f * 1600 * 1.8f * (1.f / 53.4f - 1.f / 51.8f)) * h / screenH;
  struct Tap {
    int x, y;
    float k;
  };
  std::vector<Tap> taps;
  float norm = 0;
  for (int j = -8; j <= 8; j++)
    for (int i = -8; i <= 8; i++) {
      int inside = 0;
      for (int a = 0; a < 8; a++)
        for (int b = 0; b < 8; b++) {
          float dx = i + (a + .5f) / 8 - .5f, dy = j + (b + .5f) / 8 - .5f;
          if (dx * dx + dy * dy < rad * rad)
            inside++;
        }
      if (inside) {
        taps.push_back({i, j, float(inside)});
        norm += inside;
      }
    }
  Score oldBlur, newBlur;
  for (int y = 0; y < h; y++)
    for (int x = 0; x < w; x++)
      if (inRing(x, y)) {
        float r = 0, l = 0, a = 0;
        for (const auto &t : taps) {
          int j = x + t.x + (y + t.y) * w;
          r += t.k * area[j];
          l += t.k * direct[j];
          a += t.k * adaptive[j];
        }
        oldBlur.add(l / norm, r / norm, x, y);
        newBlur.add(a / norm, r / norm, x, y);
      }
  oldBlur.put("legacy_defocus");
  newBlur.put("adapted_defocus");
  printf("pupil radius=%.4f preview pixels\n", rad);
  if (argc == 4)
    for (auto item : {std::pair{"area", &area}, std::pair{"single", &direct},
                      std::pair{"adapt", &adaptive}})
      writeFrame((std::string(argv[3]) + "-" + item.first + ".f32").c_str(),
                 *item.second);
}
