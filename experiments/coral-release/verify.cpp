// Inspect connectivity on both the full-resolution solve and the cache the GPU uses.
#define cimg_display 0
#include <CImg.h>
#include <zlib.h>
#include <algorithm>
#include <array>
#include <cmath>
#include <cstdio>
#include <filesystem>
#include <iomanip>
#include <iostream>
#include <stdexcept>
#include <string>
#include <string_view>
#include <vector>

using cimg_library::CImg;
struct Region {
    unsigned pixels = 0, overlap = 0;
    double x = 0., y = 0.;
};
struct Candidate {
    int start;
    double x, y, fraction, initial_overlap;
    CImg<unsigned char> last;
};
struct Sequence {
    CImg<unsigned char> previous_bank;
    std::vector<Candidate> pending;
    unsigned releases = 0;
    bool staggered_sites = false;
    struct Site { int frame; double x, y; };
    std::vector<Site> sites;

    void sample(CImg<unsigned char> mask, int frame) {
        mask.threshold(128);
        auto labels = mask.get_label();
        std::vector<Region> regions(labels.max()+1);
        cimg_forXY(mask, x, y) {
            if (!mask(x,y)) continue;
            auto& region = regions[labels(x,y)];
            ++region.pixels;
            region.x += x;
            region.y += y;
            if (frame && previous_bank(x,y)) ++region.overlap;
        }
        auto bank = std::max_element(regions.begin(), regions.end(),
            [](const Region& a, const Region& b){ return a.pixels < b.pixels; });
        const auto bank_label = static_cast<size_t>(bank-regions.begin());
        auto region_mask = [&](size_t label) {
            CImg<unsigned char> image(mask.width(),mask.height(),1,1,0);
            cimg_foroff(mask,i) image[i] = mask[i] && labels[i]==label;
            return image;
        };
        // A pinched subpixel neck can reconnect at a later binary sample.
        // Track each daughter by pixel overlap for six more frames before counting it.
        for (auto it=pending.begin(); it!=pending.end();) {
            std::vector<unsigned> hits(regions.size());
            unsigned last_area=0;
            cimg_foroff(mask,i) if (it->last[i]) {
                ++last_area;
                if (mask[i]) ++hits[labels[i]];
            }
            auto best=std::max_element(hits.begin(),hits.end());
            const size_t label=best-hits.begin();
            if (label==bank_label || *best < .55*last_area) {
                it=pending.erase(it);
            } else if (frame-it->start>=6) {
                std::cout << "release," << it->start << ',' << it->start/24. << ','
                          << it->x << ',' << it->y << ',' << it->fraction << ','
                          << it->initial_overlap << '\n';
                ++releases;
                for (const auto& prior : sites)
                    if (it->start-prior.frame>=3 && std::hypot(it->x-prior.x,it->y-prior.y)>.08)
                        staggered_sites=true;
                sites.push_back({it->start,it->x,it->y});
                it=pending.erase(it);
            } else {
                it->last=region_mask(label);
                ++it;
            }
        }
        for (const auto& r : regions) {
            if (&r == &*bank || r.pixels < 0.00010*mask.size() || r.overlap < 0.55*r.pixels) continue;
            double wx = (r.x/r.pixels + .5)/mask.width(), wy = (r.y/r.pixels + .5)/mask.height();
            // Report position as fractions of the visible gate from its upper left.
            pending.push_back({frame,(wx*704.-96.)/512.,(wy*704.-208.)/288.,
                double(r.pixels)/mask.size(),double(r.overlap)/r.pixels,
                region_mask(static_cast<size_t>(&r-regions.data()))});
        }
        previous_bank=region_mask(bank_label);
    }
};

struct Archive {
    gzFile stream;
    explicit Archive(const char* path) : stream(gzopen(path,"rb")) {
        if (!stream) throw std::runtime_error("Could not open cache");
    }
    ~Archive(){ gzclose(stream); }
    void read(void* dst, unsigned bytes) {
        if (gzread(stream,dst,bytes) != static_cast<int>(bytes))
            throw std::runtime_error("Incomplete cache");
    }
    unsigned integer() {
        std::array<unsigned char,4> b{}; read(b.data(),4);
        return unsigned(b[0]) | (unsigned(b[1])<<8) | (unsigned(b[2])<<16) | (unsigned(b[3])<<24);
    }
};
void check_cache(const char* path, Sequence& seq) {
    Archive data(path);
    std::array<char,8> magic{}; data.read(magic.data(),8);
    if (std::string_view(magic.data(),8)!="TSSDF002") throw std::runtime_error("Wrong cache format");
    for (auto expected : {704u,704u,96u,24u,96u,208u,512u,288u})
        if (data.integer()!=expected) throw std::runtime_error("Unexpected cache geometry or cadence");
    CImg<unsigned char> front(704,704);
    std::vector<unsigned char> rg(2*704*704);
    std::array<unsigned,2> start{}, max_delta{};
    for (int i=0; i<96; ++i) {
        data.read(rg.data(),rg.size());
        std::array<unsigned,2> area{};
        cimg_foroff(front,p) {
            front[p]=rg[p*2+1];
            for (int c=0; c<2; ++c) area[c]+=rg[p*2+c]>=128;
        }
        if (i==0) start=area;
        for (int c=0; c<2; ++c) {
            unsigned diff=static_cast<unsigned>(std::abs(int(area[c])-int(start[c])));
            max_delta[c]=std::max(max_delta[c],diff);
        }
        seq.sample(front,i);
    }
    if (gzgetc(data.stream)!=-1 || !gzeof(data.stream)) throw std::runtime_error("Cache length mismatch");
    for (int c=0; c<2; ++c) {
        std::cout << "encoded_area," << c << ',' << start[c] << ',' << max_delta[c] << ','
                  << double(max_delta[c])/start[c] << '\n';
        if (!start[c] || max_delta[c]>start[c]*.005) throw std::runtime_error("Encoded area drift >0.5%");
    }
}
int main(int argc, char** argv) try {
    if (argc!=3) throw std::runtime_error("verify --cache CACHE.gz | --solve FRONT_FRAME_DIR");
    std::cout << std::fixed << std::setprecision(7);
    Sequence seq;
    if (std::string_view(argv[1])=="--cache") check_cache(argv[2],seq);
    else if (std::string_view(argv[1])=="--solve") {
        for (int i=0; i<96; ++i) {
            char name[16]; std::snprintf(name,sizeof name,"%03d.pgm",i);
            const auto file = std::filesystem::path(argv[2])/name;
            seq.sample(CImg<unsigned char>(file.string().c_str()),i);
        }
    } else return 2;
    std::cout << "substantial_release_events," << seq.releases << '\n';
    return seq.staggered_sites ? 0 : 1;
} catch (const std::exception& ex) {
    std::cerr << ex.what() << '\n';
    return 2;
}
