// Export a fixed, independently solved pair of slide fields for a lighting
// study.
#include <ImfChannelList.h>
#include <ImfFrameBuffer.h>
#include <ImfHeader.h>
#include <ImfInputFile.h>
#include <ImfOutputFile.h>
#include <ImfStringAttribute.h>
#include <algorithm>
#include <array>
#include <cmath>
#include <cstdint>
#include <cstdio>
#include <cstring>
#include <filesystem>
#include <fstream>
#include <iostream>
#include <limits>
#include <png.h>
#include <stdexcept>
#include <string>
#include <unordered_map>
#include <vector>

namespace fs = std::filesystem;
namespace exr = OPENEXR_IMF_NAMESPACE;
using Id = uint32_t;
using Tri = std::array<Id, 3>;
using Vec = std::array<double, 3>;
constexpr double width_m = .160, height_m = .090,
                 meter_per_px = height_m / 720.;
constexpr unsigned cells_x = 512, cells_y = 288;
constexpr double rim_m = .00035, interior_m = .001;
struct Fields {
  unsigned w, h;
  std::array<std::vector<float>, 2> plane;
};

Fields read_source(const fs::path &path) {
  FILE *fp = std::fopen(path.string().c_str(), "rb");
  if (!fp)
    throw std::runtime_error("Cannot open source PNG");
  auto png =
      png_create_read_struct(PNG_LIBPNG_VER_STRING, nullptr, nullptr, nullptr);
  auto info = png ? png_create_info_struct(png) : nullptr;
  if (!png || !info || setjmp(png_jmpbuf(png))) {
    png_destroy_read_struct(&png, &info, nullptr);
    std::fclose(fp);
    throw std::runtime_error("libpng could not read the source");
  }
  png_init_io(png, fp);
  png_read_png(png, info, PNG_TRANSFORM_IDENTITY, nullptr);
  Fields f{png_get_image_width(png, info), png_get_image_height(png, info), {}};
  const auto channels = png_get_channels(png, info);
  if (png_get_bit_depth(png, info) != 16 ||
      png_get_color_type(png, info) != PNG_COLOR_TYPE_RGB || channels != 3 ||
      f.w * 9 != f.h * 16 || f.w > 8192) {
    png_destroy_read_struct(&png, &info, nullptr);
    std::fclose(fp);
    throw std::runtime_error("Expected 16-bit RGB 16:9 PNG");
  }
  for (auto &plane : f.plane)
    plane.resize(size_t(f.w) * f.h);
  auto rows = png_get_rows(png, info);
  for (unsigned y = 0; y < f.h; y++)
    for (unsigned x = 0; x < f.w; x++)
      for (int c = 0; c < 2; c++) {
        const auto *bytes = rows[y] + (x * 3 + c) * 2;
        const unsigned raw = unsigned(bytes[0]) * 256 + bytes[1];
        f.plane[c][size_t(y) * f.w + x] =
            float(96. * (double(raw) / 65535. - .5));
      }
  png_destroy_read_struct(&png, &info, nullptr);
  std::fclose(fp);
  return f;
}

void write_exr(const fs::path &path, unsigned w, unsigned h,
               std::vector<float> &data) {
  exr::Header header(w, h);
  header.insert("tssFieldUnits",
                exr::StringAttribute(
                    "signed 720p design pixels; positive inside; y=0 at top"));
  exr::FrameBuffer fb;
  for (const char *name : {"R", "G", "B"}) {
    header.channels().insert(name, exr::Channel(exr::FLOAT));
    fb.insert(name,
              exr::Slice(exr::FLOAT, reinterpret_cast<char *>(data.data()),
                         sizeof(float), w * sizeof(float)));
  }
  {
    exr::OutputFile output(path.string().c_str(), header);
    output.setFrameBuffer(fb);
    output.writePixels(h);
  }
  exr::InputFile check(path.string().c_str());
  const auto window = check.header().dataWindow();
  if (window.min.x || window.min.y || window.max.x != int(w) - 1 ||
      window.max.y != int(h) - 1)
    throw std::runtime_error("EXR dimensions changed");
  std::vector<float> result(data.size());
  for (const char *name : {"R", "G", "B"}) {
    if (!check.header().channels().findChannel(name) ||
        check.header().channels()[name].type != exr::FLOAT)
      throw std::runtime_error("EXR channel format differs");
    exr::FrameBuffer reader;
    reader.insert(name, exr::Slice(exr::FLOAT,
                                   reinterpret_cast<char *>(result.data()),
                                   sizeof(float), w * sizeof(float)));
    check.setFrameBuffer(reader);
    check.readPixels(0, int(h) - 1);
    if (result != data)
      throw std::runtime_error("EXR float samples differ from source decode");
  }
}

struct Sample {
  double x, y, d;
};
struct Shape {
  std::vector<Sample> point;
  std::vector<Tri> face;
};
uint64_t key(Id a, Id b) {
  return (uint64_t(std::min(a, b)) << 32) | std::max(a, b);
}
double field_at(const Fields &f, int ch, double u, double v) {
  const double px = std::clamp(u * f.w - .5, 0., double(f.w - 1)),
               py = std::clamp(v * f.h - .5, 0., double(f.h - 1));
  const unsigned x = unsigned(px), y = unsigned(py),
                 x1 = std::min(x + 1, f.w - 1), y1 = std::min(y + 1, f.h - 1);
  const auto &a = f.plane[ch];
  auto mix = [](double x, double y, double t) { return x + (y - x) * t; };
  return mix(mix(a[y * f.w + x], a[y * f.w + x1], px - x),
             mix(a[y1 * f.w + x], a[y1 * f.w + x1], px - x), py - y);
}
// Clip a consistently triangulated sheet to the positive linear level set.
// Shared crossing indices close neighboring triangles, including at the image
// crop.
Shape footprint(const Fields &f, int channel) {
  Shape mesh;
  const unsigned nx = cells_x + 1, ny = cells_y + 1;
  std::vector<Sample> lattice(nx * ny);
  for (unsigned y = 0; y < ny; y++)
    for (unsigned x = 0; x < nx; x++) {
      const double u = double(x) / cells_x, v = double(y) / cells_y;
      lattice[y * nx + x] = {(u - .5) * width_m, (v - .5) * height_m,
                             field_at(f, channel, u, 1. - v)};
    }
  const Id empty = std::numeric_limits<Id>::max();
  std::vector<Id> owned(lattice.size(), empty);
  std::unordered_map<uint64_t, Id> cuts;
  auto node = [&](Id a) {
    if (owned[a] == empty) {
      owned[a] = Id(mesh.point.size());
      mesh.point.push_back(lattice[a]);
    }
    return owned[a];
  };
  auto crossing = [&](Id a, Id b) {
    if (a > b)
      std::swap(a, b);
    const auto &p = lattice[a];
    const auto &q = lattice[b];
    const double t = p.d / (p.d - q.d);
    // Snap sub-micron intersections that would otherwise round onto a lattice
    // vertex in PLY.
    if (t <= 0.0001)
      return node(a);
    if (t >= 0.9999)
      return node(b);
    auto [it, is_new] = cuts.emplace(key(a, b), Id(mesh.point.size()));
    if (is_new)
      mesh.point.push_back({p.x + t * (q.x - p.x), p.y + t * (q.y - p.y), 0.});
    return it->second;
  };
  auto clip = [&](Tri input) {
    std::array<Id, 4> ix{};
    unsigned n = 0;
    for (int i = 0; i < 3; i++) {
      Id a = input[i], b = input[(i + 1) % 3];
      bool inside = lattice[a].d >= 0;
      if (inside)
        ix[n++] = node(a);
      if (inside != (lattice[b].d >= 0))
        ix[n++] = crossing(a, b);
    }
    for (unsigned i = 1; i + 1 < n; i++)
      if (ix[0] != ix[i] && ix[0] != ix[i + 1] && ix[i] != ix[i + 1])
        mesh.face.push_back({ix[0], ix[i], ix[i + 1]});
  };
  for (unsigned y = 0; y < cells_y; y++)
    for (unsigned x = 0; x < cells_x; x++) {
      Id a = y * nx + x, b = a + 1, c = b + nx, d = a + nx;
      clip({a, b, c});
      clip({a, c, d});
    }
  return mesh;
}
Vec sub(Vec a, Vec b) { return {a[0] - b[0], a[1] - b[1], a[2] - b[2]}; }
Vec cross(Vec a, Vec b) {
  return {a[1] * b[2] - a[2] * b[1], a[2] * b[0] - a[0] * b[2],
          a[0] * b[1] - a[1] * b[0]};
}
double dot(Vec a, Vec b) { return a[0] * b[0] + a[1] * b[1] + a[2] * b[2]; }
struct Edge {
  Id a, b;
  unsigned count;
  int orientation;
};
using Edges = std::unordered_map<uint64_t, Edge>;
Edges edges_of(const std::vector<Tri> &faces) {
  Edges edges;
  for (auto f : faces)
    for (int i = 0; i < 3; i++) {
      Id a = f[i], b = f[(i + 1) % 3];
      auto it = edges.try_emplace(key(a, b), Edge{a, b, 0, 0}).first;
      it->second.count++;
      it->second.orientation += a < b ? 1 : -1;
    }
  return edges;
}
void write_u32(std::ostream &out, uint32_t v) {
  const std::array<char, 4> bytes{char(v), char(v >> 8), char(v >> 16),
                                  char(v >> 24)};
  out.write(bytes.data(), 4);
}
void write_f32(std::ostream &out, double value) {
  uint32_t bits;
  float narrow = float(value);
  std::memcpy(&bits, &narrow, 4);
  write_u32(out, bits);
}
void solid(const fs::path &path, const Shape &s, double base, double width,
           bool flat) {
  const Id n = Id(s.point.size());
  std::vector<Vec> v(2 * n), normal(2 * n);
  std::vector<Tri> faces = s.face;
  double area = 0, volume = 0;
  for (Id i = 0; i < n; i++) {
    const auto &p = s.point[i];
    double t = std::clamp(p.d * meter_per_px / width, 0., 1.);
    double depth = rim_m + (interior_m - rim_m) * t * t * (3. - 2. * t);
    v[i] = {p.x, p.y, base + depth};
    v[i + n] = {p.x, p.y, base};
  }
  for (auto f : s.face) {
    auto a = v[f[0]], b = v[f[1]], c = v[f[2]];
    double da = cross(sub(b, a), sub(c, a))[2] * .5;
    if (da <= 0)
      throw std::runtime_error("Nonpositive foreground triangle");
    area += da;
    volume += da * ((a[2] + b[2] + c[2]) / 3. - base);
  }
  double flat_depth = volume / area;
  if (flat)
    for (Id i = 0; i < n; i++)
      v[i][2] = base + flat_depth;
  for (auto f : s.face) {
    auto N = cross(sub(v[f[1]], v[f[0]]), sub(v[f[2]], v[f[0]]));
    for (auto id : f)
      for (int k = 0; k < 3; k++)
        normal[id][k] += N[k];
    faces.push_back({f[2] + n, f[1] + n, f[0] + n});
  }
  for (Id i = 0; i < n; i++) {
    const double len = std::sqrt(dot(normal[i], normal[i]));
    if (!(len > 0))
      throw std::runtime_error("Mesh has an unused or coincident vertex");
    for (double &x : normal[i])
      x /= len;
    normal[i + n] = {0., 0., -1.};
  }
  for (const auto &entry : edges_of(s.face)) {
    auto e = entry.second;
    if (e.count == 1) {
      faces.push_back({e.b, e.a, e.a + n});
      faces.push_back({e.b, e.a + n, e.b + n});
    } else if (e.count != 2 || e.orientation != 0)
      throw std::runtime_error("Nonmanifold footprint");
  }
  for (const auto &entry : edges_of(faces))
    if (entry.second.count != 2 || entry.second.orientation != 0)
      throw std::runtime_error("Open or inverted solid edge");
  double check_volume = 0;
  std::ofstream out(path, std::ios::binary);
  out.exceptions(std::ios::badbit | std::ios::failbit);
  out << "ply\nformat binary_little_endian 1.0\ncomment TSS slide meters, X "
         "right Y up\nelement vertex "
      << v.size()
      << "\nproperty float x\nproperty float y\nproperty float z\nproperty "
         "float nx\nproperty float ny\nproperty float nz\nelement face "
      << faces.size()
      << "\nproperty list uchar int vertex_indices\nend_header\n";
  for (size_t i = 0; i < v.size(); i++) {
    for (auto &x : v[i]) {
      x = float(x);
      write_f32(out, x);
    }
    for (double x : normal[i])
      write_f32(out, x);
  }
  for (auto f : faces) {
    auto a = v[f[0]], b = v[f[1]], c = v[f[2]];
    auto twice_area = cross(sub(b, a), sub(c, a));
    if (!(dot(twice_area, twice_area) > 0))
      throw std::runtime_error("Degenerate serialized triangle");
    check_volume += dot(a, twice_area) / 6.;
    out.put(3);
    for (Id id : f)
      write_u32(out, id);
  }
  out.close();
  if (std::abs(volume - check_volume) > volume * 1e-5)
    throw std::runtime_error("Signed enclosed volume mismatch");
  std::cout << path.filename().string() << " vertices=" << v.size()
            << " triangles=" << faces.size() << " dye_area_m2=" << area
            << " volume_m3=" << check_volume
            << " equivalent_depth_mm=" << 1000. * flat_depth
            << " closed_edges=all\n";
}
int main(int argc, char **argv) {
  try {
    if (argc < 3 || argc > 4)
      throw std::runtime_error(
          "Usage: export-lighting input.png new-directory [meniscus_width_mm]");
    const fs::path dir = argv[2];
    const std::string width_arg = argc == 4 ? argv[3] : "1";
    size_t consumed = 0;
    const double width_mm = std::stod(width_arg, &consumed);
    if (consumed != width_arg.size() || !std::isfinite(width_mm) ||
        width_mm <= 0)
      throw std::runtime_error("Invalid meniscus width");
    auto f = read_source(argv[1]);
    if (!fs::create_directory(dir))
      throw std::runtime_error("Output directory must be new");
    for (int c = 0; c < 2; c++) {
      const std::string id = c ? "front" : "rear";
      auto &data = f.plane[c];
      write_exr(dir / (id + ".exr"), f.w, f.h, data);
      auto [minimum, maximum] = std::minmax_element(data.begin(), data.end());
      std::cout << id << ".exr " << f.w << "x" << f.h
                << " range_px=" << *minimum << "," << *maximum
                << " lossless_float_roundtrip=all_RGB_samples\n";
      auto shape = footprint(f, c);
      for (bool flat : {false, true})
        solid(dir / (id + (flat ? "-flat.ply" : ".ply")), shape, c ? 0. : .0025,
              width_mm / 1000., flat);
    }
  } catch (const std::exception &e) {
    std::cerr << e.what() << "\n";
    return 1;
  }
}
