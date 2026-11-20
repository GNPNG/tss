// Controlled, layer-local resampling experiment. The sharp slide is sampled
// once.
#include <ImfChannelList.h>
#include <ImfFrameBuffer.h>
#include <ImfHeader.h>
#include <ImfInputFile.h>
extern "C" {
#include <libavutil/pixfmt.h>
#include <libswscale/swscale.h>
}
#include <algorithm>
#include <array>
#include <cmath>
#include <filesystem>
#include <fstream>
#include <iomanip>
#include <iostream>
#include <iterator>
#include <memory>
#include <regex>
#include <stdexcept>
#include <string>
#include <vector>

namespace exr = OPENEXR_IMF_NAMESPACE;
namespace fs = std::filesystem;
struct Plane {
  int w, h;
  std::vector<float> f;
};
using RGB = std::array<Plane, 3>;
using Ray = std::array<double, 2>;
Plane plane(int w, int h) { return {w, h, std::vector<float>(size_t(w) * h)}; }
RGB rgb(int w, int h) { return {plane(w, h), plane(w, h), plane(w, h)}; }

Plane read_field(const fs::path &path) {
  exr::InputFile reader(path.string().c_str());
  const auto box = reader.header().dataWindow();
  if (box.min.x || box.min.y || box.max.x != 1023 || box.max.y != 575 ||
      reader.header().channels()["R"].type != exr::FLOAT)
    throw std::runtime_error("Expected 1024 x 576 signed float EXR");
  auto p = plane(box.max.x + 1, box.max.y + 1);
  exr::FrameBuffer fb;
  fb.insert("R", exr::Slice(exr::FLOAT, reinterpret_cast<char *>(p.f.data()),
                            sizeof(float), p.w * sizeof(float)));
  reader.setFrameBuffer(fb);
  reader.readPixels(0, p.h - 1);
  return p;
}
std::vector<Ray> pupil(const std::string &text) {
  const auto first = text.find("const vec2 aperture[24] = vec2[24](");
  const auto last = text.find(");", first);
  if (first == std::string::npos || last == std::string::npos)
    throw std::runtime_error("Aperture absent");
  const auto section = text.substr(first, last - first);
  std::regex pair(R"(vec2\(\s*(-?[0-9]+\.[0-9]+),\s*(-?[0-9]+\.[0-9]+)\))");
  std::vector<Ray> rays;
  for (auto i = std::sregex_iterator(section.begin(), section.end(), pair);
       i != std::sregex_iterator(); ++i)
    rays.push_back({std::stod((*i)[1]), std::stod((*i)[2])});
  if (rays.size() != 24)
    throw std::runtime_error("Expected the shared 24-ray aperture");
  return rays;
}
std::array<double, 3> absorption(const std::string &src,
                                 const std::string &name) {
  const std::regex pattern(
      "const vec3 " + name +
      R"( = vec3\(([0-9.]+),\s*([0-9.]+),\s*([0-9.]+)\);)");
  std::smatch match;
  if (!std::regex_search(src, match, pattern))
    throw std::runtime_error("Missing baseline dye " + name);
  return {std::stod(match[1]), std::stod(match[2]), std::stod(match[3])};
}
double sample(const Plane &p, double x, double y) {
  x = std::clamp(x, 0., double(p.w - 1));
  y = std::clamp(y, 0., double(p.h - 1));
  const int x0 = int(x), y0 = int(y), x1 = std::min(x0 + 1, p.w - 1),
            y1 = std::min(y0 + 1, p.h - 1);
  const double tx = x - x0, ty = y - y0;
  return (1 - ty) * ((1 - tx) * p.f[y0 * p.w + x0] + tx * p.f[y0 * p.w + x1]) +
         ty * ((1 - tx) * p.f[y1 * p.w + x0] + tx * p.f[y1 * p.w + x1]);
}
RGB transmit(const Plane &d, const std::array<double, 3> &alpha,
             const std::vector<Ray> &rays, double radius_design_px,
             double rim_width, int w, int h) {
  auto result = rgb(w, h);
  const double radius = radius_design_px * d.h / 720.;
  for (int y = 0; y < h; y++)
    for (int x = 0; x < w; x++) {
      const double cx = (x + .5) * d.w / w - .5, cy = (y + .5) * d.h / h - .5;
      std::array<double, 3> sum{};
      for (auto a : rays) {
        const double q =
            std::clamp(.5 + sample(d, cx + radius * a[0], cy + radius * a[1]) /
                                (1.3 * rim_width),
                       0., 1.);
        const double coverage = q * q * (3. - 2. * q);
        for (size_t c = 0; c < 3; c++)
          sum[c] += std::exp(-alpha[c] * coverage);
      }
      for (size_t c = 0; c < 3; c++)
        result[c].f[y * w + x] = float(sum[c] / rays.size());
    }
  return result;
}
RGB upscale(const RGB &source, int w, int h, int flags) {
  auto result = rgb(w, h);
  const double catmull_rom[] = {0., .5};
  std::unique_ptr<SwsContext, decltype(&sws_freeContext)> ctx(
      sws_getContext(source[0].w, source[0].h, AV_PIX_FMT_GRAYF32, w, h,
                     AV_PIX_FMT_GRAYF32, flags, nullptr, nullptr,
                     flags == SWS_BICUBIC ? catmull_rom : nullptr),
      sws_freeContext);
  if (!ctx)
    throw std::runtime_error("Unable to configure float resampler");
  for (int c = 0; c < 3; c++) {
    const uint8_t *from[4] = {
        reinterpret_cast<const uint8_t *>(source[c].f.data())};
    uint8_t *to[4] = {reinterpret_cast<uint8_t *>(result[c].f.data())};
    int from_stride[4] = {source[c].w * 4}, to_stride[4] = {w * 4};
    if (sws_scale(ctx.get(), from, from_stride, 0, source[c].h, to,
                  to_stride) != h)
      throw std::runtime_error("Incomplete upscale");
    for (float &v : result[c].f)
      v = std::clamp(v, 0.f, 1.f);
  }
  return result;
}
RGB combine(const RGB &sharp, const RGB &rear) {
  auto out = rgb(sharp[0].w, sharp[0].h);
  for (int c = 0; c < 3; c++)
    for (size_t i = 0; i < out[c].f.size(); i++)
      out[c].f[i] = sharp[c].f[i] * rear[c].f[i];
  return out;
}
void check_registration(int divisor, int filter) {
  const int w = 128, h = 64;
  auto ramp = rgb(w / divisor, h / divisor);
  for (int c = 0; c < 3; c++) {
    auto &p = ramp[c];
    for (int y = 0; y < p.h; y++)
      for (int x = 0; x < p.w; x++)
        p.f[y * p.w + x] = float(c == 0   ? (x + .5) / p.w
                                 : c == 1 ? (y + .5) / p.h
                                          : .5);
  }
  auto test = upscale(ramp, w, h, filter);
  for (int y = 2 * divisor; y < h - 2 * divisor; y++)
    for (int x = 2 * divisor; x < w - 2 * divisor; x++)
      for (int c = 0; c < 3; c++) {
        double expected = c == 0 ? (x + .5) / w : c == 1 ? (y + .5) / h : .5;
        if (std::abs(test[c].f[y * w + x] - expected) > .001)
          throw std::runtime_error(
              "Float resampler failed center registration");
      }
}
int srgb(double x) {
  return int(
      std::round(255. * (x < .0031308 ? 12.92 * x
                                      : 1.055 * std::pow(x, 1. / 2.4) - .055)));
}
void save_error(const fs::path &path, const RGB &reference, const RGB &test) {
  std::ofstream out(path, std::ios::binary);
  out.exceptions(std::ios::failbit | std::ios::badbit);
  out << "P5\n" << test[0].w << " " << test[0].h << "\n255\n";
  for (size_t i = 0; i < test[0].f.size(); i++) {
    float peak = 0;
    for (int c = 0; c < 3; c++)
      peak = std::max(peak, std::abs(test[c].f[i] - reference[c].f[i]));
    out.put(char(srgb(std::min(1.f, 8.f * peak))));
  }
}
void save_ppm(const fs::path &path, const RGB &img) {
  std::ofstream out(path, std::ios::binary);
  out.exceptions(std::ios::failbit | std::ios::badbit);
  out << "P6\n" << img[0].w << " " << img[0].h << "\n255\n";
  for (size_t i = 0; i < img[0].f.size(); i++)
    for (int c = 0; c < 3; c++)
      out.put(char(srgb(img[c].f[i])));
}
struct Stats {
  std::vector<float> max_errors;
  double sum_sq = 0;
};
void metrics(std::ofstream &csv, const std::string &variant,
             const RGB &reference, const RGB &test, const Plane &front,
             const Plane &rear, double blur) {
  std::array<Stats, 4> regions;
  for (size_t i = 0; i < front.f.size(); i++) {
    float err = 0;
    double sq = 0;
    for (int c = 0; c < 3; c++) {
      double e = std::abs(reference[c].f[i] - test[c].f[i]);
      err = std::max(err, float(e));
      sq += e * e;
    }
    const bool rear_rim = std::abs(rear.f[i]) <= blur + 1;
    const bool front_rim = std::abs(front.f[i]) <= 3;
    const std::array<bool, 4> active{true, rear_rim && front_rim,
                                     rear_rim && !front_rim,
                                     !rear_rim && !front_rim};
    for (size_t k = 0; k < active.size(); k++)
      if (active[k]) {
        regions[k].max_errors.push_back(err);
        regions[k].sum_sq += sq;
      }
  }
  const std::array<const char *, 4> names{"all", "crossing", "rear_edge",
                                          "plain"};
  for (size_t j = 0; j < regions.size(); j++) {
    auto &m = regions[j];
    auto &e = m.max_errors;
    if (e.empty())
      throw std::runtime_error("Empty test region");
    std::sort(e.begin(), e.end());
    csv << variant << "," << names[j] << "," << e.size() << ","
        << std::sqrt(m.sum_sq / (3 * e.size())) << ","
        << e[size_t(std::ceil(.95 * e.size())) - 1] << "," << e.back() << "\n";
  }
}
int main(int argc, char **argv) {
  try {
    if (argc != 4)
      throw std::runtime_error(
          "Usage: compare geometry-directory optics.hook new-output-dir");
    fs::path geo = argv[1], out = argv[3];
    std::ifstream hook(argv[2]);
    std::string src((std::istreambuf_iterator<char>(hook)), {});
    const auto rays = pupil(src);
    auto far = read_field(geo / "rear.exr"),
         near = read_field(geo / "front.exr");
    const auto pink = absorption(src, "pinkAbsorption"),
               cyan = absorption(src, "cyanAbsorption");
    const double blur = 8. * 1600. * 1.8 * (1. / 53.4 - 1. / 51.8);
    auto sharp = transmit(near, cyan, {{0., 0.}}, 0., 1.15, near.w, near.h);
    auto full =
        combine(sharp, transmit(far, pink, rays, blur, 1.46, far.w, far.h));
    if (!fs::create_directory(out))
      throw std::runtime_error("Use a fresh output directory");
    save_ppm(out / "reference.ppm", full);
    std::ofstream csv(out / "error.csv");
    csv << std::setprecision(9)
        << "variant,region,pixels,rgb_rmse,max_channel_abs_p95,max_channel_abs_"
           "peak\n";
    for (int divisor : {2, 4}) {
      auto reduced = transmit(far, pink, rays, blur, 1.46, far.w / divisor,
                              far.h / divisor);
      for (auto entry : {std::pair<int, const char *>{SWS_BILINEAR, "linear"},
                         {SWS_BICUBIC, "cubic"}}) {
        check_registration(divisor, entry.first);
        auto image =
            combine(sharp, upscale(reduced, far.w, far.h, entry.first));
        auto name =
            std::string(entry.second) + "-" + std::to_string(divisor) + "x";
        metrics(csv, name, full, image, near, far, std::abs(blur));
        save_ppm(out / (name + ".ppm"), image);
        if (entry.first == SWS_BILINEAR)
          save_error(out / (name + "-error.pgm"), full, image);
      }
    }
    std::cout << "Rear pupil radius in 720p design px: " << blur
              << "; native source px: " << blur * far.h / 720. << "\n";
  } catch (const std::exception &e) {
    std::cerr << e.what() << "\n";
    return 1;
  }
}
