// Compare 4-connected material footprints between adjacent observations.
// The optional original Basilisk P5 images are independent of the packed cache.
#include <zlib.h>
#include <algorithm>
#include <array>
#include <cmath>
#include <cstdint>
#include <cstdio>
#include <filesystem>
#include <fstream>
#include <iostream>
#include <limits>
#include <stdexcept>
#include <string>
#include <vector>

using Bytes = std::vector<uint8_t>;
using namespace std;
namespace fs = std::filesystem;

struct Summary {
  int min_area = numeric_limits<int>::max(), max_area = 0, max_delta = 0;
  array<int, 2> largest_appearance{}, largest_disappearance{};
  // Phase 0 is carrier, phase 1 is dyed oil. Disjoint footprints can
  // also be small moving islands which travelled more than their diameter.
  void compare(const Bytes& old, const Bytes& now, int w, int h, bool reverse) {
    Bytes seen(now.size(), 0);
    vector<int> stack;
    int total = 0;
    for (int start = 0; start < w*h; ++start) {
      if (seen[start]) continue;
      auto phase = now[start];
      bool overlap = false;
      int area = 0;
      stack.push_back(start);
      seen[start] = 1;
      while (!stack.empty()) {
        int at = stack.back();
        stack.pop_back();
        ++area;
        overlap |= old[at] == phase;
        auto visit = [&](int j) {
          if (!seen[j] && now[j] == phase) {
            seen[j] = 1;
            stack.push_back(j);
          }
        };
        int x = at % w;
        if (x > 0) visit(at-1);
        if (x+1 < w) visit(at+1);
        if (at >= w) visit(at-w);
        if (at+w < w*h) visit(at+w);
      }
      if (phase) total += area;
      if (!overlap) {
        int& peak = (reverse ? largest_disappearance : largest_appearance)[phase];
        peak = max(peak, area);
      }
    }
    min_area = min(min_area, total);
    max_area = max(max_area, total);
  }
  void step(const Bytes& a, const Bytes& b, int w, int h) {
    compare(a, b, w, h, false);
    compare(b, a, w, h, true);
    int delta = 0;
    for (size_t i = 0; i < a.size(); ++i) delta += int(b[i])-int(a[i]);
    max_delta = max(max_delta, abs(delta));
  }
  void print(const char* source, const char* slide, int w, int h) const {
    cout << source << ',' << slide << ',' << w << ',' << h << ',' << min_area << ','
         << max_area << ',' << max_delta << ',' << largest_appearance[1] << ','
         << largest_disappearance[1] << ',' << largest_appearance[0] << ','
         << largest_disappearance[0] << '\n';
  }
};

Bytes pgm(const fs::path& path, int& w, int& h) {
  ifstream input(path, ios::binary);
  string magic;
  int maxval;
  if (!(input >> magic >> w >> h >> maxval) || magic != "P5" || w != 1024 ||
      h != 576 || maxval != 255 || input.get() != '\n')
    throw runtime_error("Invalid solver PGM " + path.string());
  Bytes result(w*h);
  if (!input.read(reinterpret_cast<char*>(result.data()), result.size()))
    throw runtime_error("Truncated solver PGM");
  for (auto& value : result) value = value >= 128;
  return result;
}

Bytes cache(const char* path) {
  constexpr size_t size = 24 + 512*288*96*2;
  Bytes bytes(size);
  gzFile in = gzopen(path, "rb");
  if (!in) throw runtime_error("Cannot open cache");
  int count = gzread(in, bytes.data(), size);
  int trailing = gzgetc(in);
  int result = gzclose(in);
  if (count != int(size) || trailing != -1 || result != Z_OK ||
      string(bytes.begin(), bytes.begin()+8) != "TSSDF001")
    throw runtime_error("Unexpected field cache");
  array<unsigned,4> expected{512,288,96,24};
  for (size_t i=0; i<expected.size(); ++i)
    for (size_t j=0; j<4; ++j)
      if (bytes[8+4*i+j] != uint8_t(expected[i] >> (8*j)))
        throw runtime_error("Unexpected cache dimensions or frame rate");
  return bytes;
}

int main(int argc, char** argv) try {
  if (argc != 2 && argc != 3)
    throw runtime_error("field-continuity slide-fields.rg8.gz [solver-out]");
  Bytes data = cache(argv[1]);
  cout << "source,slide,width,height,min_dyed,max_dyed,max_adjacent_area_change,"
          "max_new_dyed,max_lost_dyed,max_new_carrier,max_lost_carrier\n";
  for (int slide = 0; slide < 2; ++slide) {
    const char* name = slide ? "front" : "rear";
    for (int hz : {24, 40}) {
      constexpr int w=512, h=288;
      Bytes last, frame(w*h);
      Summary summary;
      // One round trip (190 source intervals), including the reversal.
      int frames = static_cast<int>(ceil(190.0*hz/24.0));
      for (int n=0; n<=frames; ++n) {
        double pos=fmod(double(n)*24/hz, 190.0);
        if (pos > 95.0) pos=190.0-pos;
        int first=static_cast<int>(pos), second=min(first+1,95);
        double t=pos-first;
        for (int i=0; i<w*h; ++i) {
          auto a=data[24+2*(first*w*h+i)+slide], b=data[24+2*(second*w*h+i)+slide];
          frame[i] = (1-t)*a+t*b >= 127.5;
        }
        if (!last.empty()) summary.step(last,frame,w,h);
        last=frame;
      }
      summary.print(hz==24 ? "cache-24Hz-roundtrip" : "cache-40Hz-roundtrip", name,w,h);
    }
    if (argc == 3) {
      Summary summary;
      Bytes last;
      int w=0,h=0;
      for (int n=0; n<96; ++n) {
        char file[16];
        snprintf(file,sizeof(file),"%03d.pgm",n);
        auto frame=pgm(fs::path(argv[2])/(slide ? "cyan" : "violet")/file,w,h);
        if (!last.empty()) summary.step(last,frame,w,h);
        last=std::move(frame);
      }
      summary.print("VOF-24Hz-forward",name,w,h);
    }
  }
  return 0;
} catch (const exception& e) {
  cerr << e.what() << '\n';
  return 1;
}
