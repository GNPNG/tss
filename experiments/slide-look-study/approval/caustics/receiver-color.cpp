// Apply the ratio of two neutral, scene-linear Cycles receiver exposures.
#include <ImfChannelList.h>
#include <ImfFrameBuffer.h>
#include <ImfHeader.h>
#include <ImfInputFile.h>
#include <algorithm>
#include <array>
#include <cmath>
#include <cstdint>
#include <cstdio>
#include <filesystem>
#include <fstream>
#include <iomanip>
#include <iostream>
#include <numeric>
#include <png.h>
#include <stdexcept>
#include <string>
#include <vector>

namespace exr = OPENEXR_IMF_NAMESPACE;
namespace fs = std::filesystem;
struct Plane {
  int width, height;
  std::vector<float> data;
};

Plane neutral_plane(const fs::path &path, bool signed_distance = false) {
  exr::InputFile in(path.string().c_str());
  const auto &box = in.header().dataWindow();
  if (box != in.header().displayWindow() || box.min.x || box.min.y ||
      box.max.x < 15 || box.max.x > 8191 ||
      (box.max.x + 1) * 9 != (box.max.y + 1) * 16)
    throw std::runtime_error("Expected uncropped 16:9 receiver EXR");
  const int w = box.max.x + 1, h = box.max.y + 1;
  std::vector<std::array<float, 3>> pixels(size_t(w) * h);
  exr::FrameBuffer channels;
  for (int c = 0; c < 3; c++) {
    const std::string channel(1, "RGB"[c]);
    auto entry = in.header().channels().findChannel(channel.c_str());
    if (!entry || entry->type != exr::FLOAT || entry->xSampling != 1 ||
        entry->ySampling != 1)
      throw std::runtime_error("Expected full-rate float32 R,G,B channels");
    channels.insert(
        channel, exr::Slice(exr::FLOAT, reinterpret_cast<char *>(&pixels[0][c]),
                            sizeof(pixels[0]), w * sizeof(pixels[0])));
  }
  in.setFrameBuffer(channels);
  in.readPixels(0, h - 1);
  Plane plane{w, h, {}};
  plane.data.reserve(pixels.size());
  for (const auto &p : pixels) {
    const auto limits = std::minmax_element(p.begin(), p.end());
    for (float value : p)
      if (!std::isfinite(value) || (!signed_distance && value < 0))
        throw std::runtime_error("Invalid EXR sample");
    if (*limits.second - *limits.first > 1e-4f * std::max(1.f, *limits.second))
      throw std::runtime_error(
          "A neutral receiver is required for scalar application");
    plane.data.push_back(.2126f * p[0] + .7152f * p[1] + .0722f * p[2]);
  }
  return plane;
}

float stats(const char *name, std::vector<float> values) {
  std::sort(values.begin(), values.end());
  const auto q = [&](double v) {
    return values[size_t(std::round(v * (values.size() - 1)))];
  };
  std::cout << name << " min=" << values.front() << " p01=" << q(.01)
            << " p05=" << q(.05) << " p50=" << q(.5) << " p95=" << q(.95)
            << " p99=" << q(.99) << " max=" << values.back() << "\n";
  return q(.95);
}

double sample(const Plane &p, double u, double v) {
  const double x = std::clamp(u * p.width - .5, 0., double(p.width - 1)),
               y = std::clamp(v * p.height - .5, 0., double(p.height - 1));
  const int x0 = int(x), y0 = int(y), x1 = std::min(x0 + 1, p.width - 1),
            y1 = std::min(y0 + 1, p.height - 1);
  const auto at = [&](int a, int b) { return p.data[size_t(b) * p.width + a]; };
  const auto mix = [](double a, double b, double t) { return a + (b - a) * t; };
  return mix(mix(at(x0, y0), at(x1, y0), x - x0),
             mix(at(x0, y1), at(x1, y1), x - x0), y - y0);
}

void registration(const fs::path &geometry, const Plane &gain,
                  const Plane &flat, float reference) {
  auto rear = neutral_plane(geometry / "rear.exr", true);
  auto front = neutral_plane(geometry / "front.exr", true);
  for (const std::string name : {"registered", "mirror_x", "mirror_y"}) {
    double near_sum = 0, far_sum = 0;
    size_t near_count = 0, far_count = 0, interior = 0, interior_dark = 0;
    size_t near_zero = 0, interior_zero = 0;
    for (int y = 0; y < gain.height; y++)
      for (int x = 0; x < gain.width; x++) {
        const double u = (x + .5) / gain.width, v = (y + .5) / gain.height;
        const double su = name == "mirror_x" ? 1 - u : u;
        const double sv = name == "mirror_y" ? 1 - v : v;
        const double d = std::min(std::abs(sample(rear, su, sv)),
                                  std::abs(sample(front, su, sv)));
        const auto value = gain.data[size_t(y) * gain.width + x];
        const double change = std::abs(value - 1.);
        if (d <= 8.) {
          near_sum += change;
          near_count++;
          near_zero += value == 0.;
        }
        if (d >= 20.) {
          far_sum += change;
          far_count++;
          if (name == "registered" && u >= .01 && u <= .99 && v >= .01 &&
              v <= .99) {
            interior++;
            interior_zero += value == 0.;
            interior_dark +=
                flat.data[size_t(y) * flat.width + x] < .25f * reference;
          }
        }
      }
    if (!near_count || !far_count)
      throw std::runtime_error("No support for registration test");
    if (name == "registered") {
      std::cout << "curved_zero_pixels="
                << std::count(gain.data.begin(), gain.data.end(), 0.f)
                << " curved_zero_near=" << near_zero
                << " curved_zero_interior=" << interior_zero << "\n";
      std::cout << "interior_below_quarter_p95=" << interior_dark
                << " interior_total=" << interior << "\n";
      if (!interior || interior_dark > .01 * interior)
        throw std::runtime_error(
            "Planar control loses illumination across slide interiors");
    }
    std::cout << name << " near_n=" << near_count
              << " near_mean_abs_change=" << near_sum / near_count
              << " far_n=" << far_count
              << " far_mean_abs_change=" << far_sum / far_count << "\n";
  }
}

void color_plate(const fs::path &base, const fs::path &dest, const Plane &g) {
  if (fs::exists(dest))
    throw std::runtime_error("Use a fresh output path");
  FILE *fi = std::fopen(base.string().c_str(), "rb");
  if (!fi)
    throw std::runtime_error("Cannot open color plate");
  auto rd =
      png_create_read_struct(PNG_LIBPNG_VER_STRING, nullptr, nullptr, nullptr);
  auto info = rd ? png_create_info_struct(rd) : nullptr;
  if (!rd || !info || setjmp(png_jmpbuf(rd)))
    throw std::runtime_error("Cannot read PNG");
  png_init_io(rd, fi);
  png_read_png(rd, info, PNG_TRANSFORM_IDENTITY, nullptr);
  const auto width = png_get_image_width(rd, info),
             height = png_get_image_height(rd, info);
  int intent;
  if (width != 3840 || height != 2160 || png_get_bit_depth(rd, info) != 16 ||
      png_get_color_type(rd, info) != PNG_COLOR_TYPE_RGB ||
      !png_get_sRGB(rd, info, &intent))
    throw std::runtime_error("Expected tagged 3840x2160 16-bit sRGB RGB plate");
  const auto rows = png_get_rows(rd, info);
  FILE *fo = std::fopen(dest.string().c_str(), "wb");
  if (!fo)
    throw std::runtime_error("Cannot open output");
  auto wr =
      png_create_write_struct(PNG_LIBPNG_VER_STRING, nullptr, nullptr, nullptr);
  auto meta = wr ? png_create_info_struct(wr) : nullptr;
  if (!wr || !meta || setjmp(png_jmpbuf(wr)))
    throw std::runtime_error("Cannot write output PNG");
  png_init_io(wr, fo);
  png_set_IHDR(wr, meta, width, height, 16, PNG_COLOR_TYPE_RGB,
               PNG_INTERLACE_NONE, PNG_COMPRESSION_TYPE_DEFAULT,
               PNG_FILTER_TYPE_DEFAULT);
  png_set_sRGB_gAMA_and_cHRM(wr, meta, intent);
  png_write_info(wr, meta);
  std::array<double, 65536> linear;
  for (unsigned i = 0; i < linear.size(); i++) {
    double encoded = i / 65535.;
    linear[i] = encoded <= .04045 ? encoded / 12.92
                                  : std::pow((encoded + .055) / 1.055, 2.4);
  }
  uint64_t clipped = 0, changed = 0;
  for (unsigned y = 0; y < height; y++) {
    for (unsigned x = 0; x < width; x++) {
      const double gain = sample(g, (x + .5) / width, (y + .5) / height);
      bool pixel_clipped = false, pixel_changed = false;
      for (int c = 0; c < 3; c++) {
        auto p = rows[y] + (x * 3 + c) * 2;
        unsigned before = p[0] * 256u + p[1];
        double value = linear[before] * gain;
        pixel_clipped |= value > 1.;
        value = std::min(value, 1.);
        double encoded = value <= .0031308
                             ? value * 12.92
                             : 1.055 * std::pow(value, 1. / 2.4) - .055;
        unsigned after = unsigned(std::round(encoded * 65535.));
        p[0] = png_byte(after >> 8);
        p[1] = png_byte(after & 255);
        pixel_changed |= after != before;
      }
      clipped += pixel_clipped;
      changed += pixel_changed;
    }
    png_write_row(wr, rows[y]);
  }
  png_write_end(wr, meta);
  png_destroy_write_struct(&wr, &meta);
  png_destroy_read_struct(&rd, &info, nullptr);
  std::fclose(fi);
  std::fclose(fo);
  std::cout << "output=" << width << "x" << height
            << " display_clipped_pixels=" << clipped
            << " changed_pixels=" << changed << "\n";
}

int main(int argc, char **argv) {
  try {
    if (argc != 6)
      throw std::runtime_error("Usage: receiver-color base-srgb.png curved.exr "
                               "flat.exr new-output.png geometry-directory");
    auto curved = neutral_plane(argv[2]), flat = neutral_plane(argv[3]);
    if (curved.width != flat.width || curved.height != flat.height)
      throw std::runtime_error("Receiver dimensions differ");
    std::cout << std::setprecision(9) << "receiver=" << flat.width << "x"
              << flat.height << "\n";
    stats("curved", curved.data);
    const auto reference = stats("flat", flat.data);
    size_t zero = 0, dark = 0;
    for (float f : flat.data) {
      zero += f <= 0;
      dark += f < .25f * reference;
    }
    std::cout << "flat_zero_pixels=" << zero
              << " flat_below_quarter_p95=" << dark
              << " total=" << flat.data.size() << "\n";
    if (reference <= 0 || zero)
      throw std::runtime_error(
          "Planar control has insufficient support for a full-frame ratio");
    std::cout << "integrated_curved_over_flat="
              << std::accumulate(curved.data.begin(), curved.data.end(), 0.) /
                     std::accumulate(flat.data.begin(), flat.data.end(), 0.)
              << "\n";
    for (size_t i = 0; i < curved.data.size(); i++)
      curved.data[i] /= flat.data[i];
    stats("curved_over_flat", curved.data);
    registration(argv[5], curved, flat, reference);
    color_plate(argv[1], argv[4], curved);
  } catch (const std::exception &e) {
    std::cerr << e.what() << "\n";
    return 1;
  }
}
