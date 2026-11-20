// Pack separately solved full slides, retaining the projected gate in source texels.
#include "optical-distance.hpp"
#include <array>
#include <filesystem>
#include <memory>
#include <vector>
#include <zlib.h>

namespace fs = std::filesystem;
constexpr unsigned pixels = 704, gate_w = 512, gate_h = 288, states = 96;

void append(gzFile fp, const void* p, unsigned n) {
    if (gzwrite(fp, p, n) != int(n)) throw std::runtime_error("Unable to write world cache");
}

int main(int argc, char** argv) try {
    if (argc != 3) throw std::runtime_error("pack-world-cache SOLVER-OUT NEW-CACHE.gz");
    const fs::path root = argv[1], out = argv[2];
    if (fs::exists(out)) throw std::runtime_error("Destination already exists");
    auto temp = out; temp += ".partial";
    // The exclusive-create flag prevents overwriting another running pack.
    std::unique_ptr<gzFile_s, decltype(&gzclose)> gz(gzopen(temp.string().c_str(),"wb6x"),gzclose);
    if (!gz) throw std::runtime_error("Cannot create fresh partial cache");
    append(gz.get(),"TSSDF002",8);
    for (unsigned value : std::array<unsigned,8>{pixels,pixels,states,24,
            (pixels-gate_w)/2,(pixels-gate_h)/2,gate_w,gate_h}) {
        std::array<unsigned char,4> le{};
        for (size_t k=0;k<le.size();++k) le[k]=static_cast<unsigned char>(value>>(k*8));
        append(gz.get(),le.data(),le.size());
    }
    std::vector<unsigned char> rg(2*pixels*pixels);
    for (unsigned frame=0;frame<states;++frame) {
        char file[16];
        std::snprintf(file,sizeof(file),"%03u.pgm",frame);
        for (unsigned channel=0;channel<2;++channel) {
            const auto input=root/(channel ? "cyan" : "violet")/file;
            const auto field=optical_distance(input.string().c_str(),pixels,pixels);
            for (unsigned i=0;i<pixels*pixels;++i) {
                double design_distance=field[i]*720./gate_h;
                rg[2*i+channel]=static_cast<unsigned char>(std::lround(
                        255.*std::clamp(.5+design_distance/96.,0.,1.)));
            }
        }
        append(gz.get(),rg.data(),rg.size());
    }
    if (gzclose(gz.release()) != Z_OK) throw std::runtime_error("Incomplete world cache");
    fs::rename(temp,out);
    return 0;
} catch (const std::exception& err) {
    std::fprintf(stderr,"%s\n",err.what());
    return 1;
}
