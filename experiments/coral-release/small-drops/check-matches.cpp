// Compare the same detached daughter in the source and optical recordings.
#define cimg_display 0
#include <CImg.h>
#include <algorithm>
#include <cmath>
#include <cstdio>
#include <filesystem>
#include <fstream>
#include <iostream>
#include <memory>
#include <stdexcept>
#include <string>
#include <vector>
#include <zlib.h>

using cimg_library::CImg;
namespace {
struct Region {
    std::vector<unsigned> pixels;
    double x = 0., y = 0.;
};

struct Frame {
    CImg<unsigned char> mask;
    CImg<unsigned> labels;
    std::vector<Region> regions;
    unsigned bank = 0;

    explicit Frame(const CImg<unsigned char> &bytes)
        : mask(bytes.get_threshold(128)), labels(mask.get_label()), regions(labels.max() + 1) {
        cimg_forXY(mask, x, y) if (mask(x, y)) {
            auto &r = regions[labels(x, y)];
            r.pixels.push_back(y * mask.width() + x);
            r.x += x;
            r.y += y;
        }
        for (unsigned i = 0; i < regions.size(); ++i) {
            auto &r = regions[i];
            const auto n = r.pixels.size();
            if (!n)
                continue;
            r.x = ((r.x / n + .5) * 704 / mask.width() - 96) / 512;
            r.y = ((r.y / n + .5) * 704 / mask.height() - 208) / 288;
            if (n > regions[bank].pixels.size())
                bank = i;
        }
    }
};

struct Track {
    int start;
    double x, y;
    unsigned label = 0;
    std::vector<unsigned> pixels;

    void advance(const Frame &state, int frame) {
        if (frame < start)
            return;
        if (frame == start) {
            // Published centroids are rounded to seven decimals in gate units.
            auto found =
                std::find_if(state.regions.begin(), state.regions.end(), [&](const Region &r) {
                    return !r.pixels.empty() && std::hypot(r.x - x, r.y - y) < 1e-5;
                });
            if (found == state.regions.end())
                throw std::runtime_error("Release centroid missing");
            label = unsigned(found - state.regions.begin());
        } else {
            std::vector<unsigned> hits(state.regions.size());
            for (auto i : pixels)
                if (state.mask[i])
                    ++hits[state.labels[i]];
            auto best = std::max_element(hits.begin(), hits.end());
            if (*best < .55 * pixels.size())
                throw std::runtime_error("Daughter overlap lost");
            label = unsigned(best - hits.begin());
        }
        if (label == state.bank)
            throw std::runtime_error("Daughter rejoined bank");
        pixels = state.regions[label].pixels;
    }
};

struct Site {
    Track source, optical;
    int comparison;
    double expected_overlap;
};

double overlap(const Frame &source, const Frame &optical, const Site &site) {
    unsigned intersection = 0, combined = 0;
    cimg_forXY(optical.mask, x, y) {
        int sx = int((x + .5) * source.mask.width() / optical.mask.width());
        int sy = int((y + .5) * source.mask.height() / optical.mask.height());
        bool a = source.mask(sx, sy) && source.labels(sx, sy) == site.source.label;
        bool b = optical.mask(x, y) && optical.labels(x, y) == site.optical.label;
        intersection += a && b;
        combined += a || b;
    }
    return combined ? double(intersection) / combined : 0.;
}
} // namespace

int main(int argc, char **argv) try {
    if (argc != 4)
        throw std::runtime_error("check-matches MATCHED-SITES.csv SOURCE-FRAMES FRONT.r8.gz");
    std::ifstream input(argv[1]);
    std::string line;
    if (!std::getline(input, line))
        throw std::runtime_error("Missing site table");
    std::vector<Site> sites;
    int end = 0;
    while (std::getline(input, line)) {
        Site site{};
        if (std::sscanf(line.c_str(), "%d,%lf,%lf,%d,%lf,%lf,%d,%lf", &site.source.start,
                        &site.source.x, &site.source.y, &site.optical.start, &site.optical.x,
                        &site.optical.y, &site.comparison, &site.expected_overlap) != 8 ||
            std::min(site.source.start, site.optical.start) < 0 || site.comparison >= 96 ||
            site.comparison != std::max(site.source.start, site.optical.start) + 6)
            throw std::runtime_error("Invalid site table");
        end = std::max(end, site.comparison);
        sites.push_back(site);
    }
    if (sites.empty())
        throw std::runtime_error("Empty site table");
    std::unique_ptr<gzFile_s, decltype(&gzclose)> gz(gzopen(argv[3], "rb"), gzclose);
    if (!gz)
        throw std::runtime_error("Front cache unavailable");
    CImg<unsigned char> optical_bytes(704, 704);
    for (int i = 0; i <= end; ++i) {
        char name[16];
        std::snprintf(name, sizeof name, "%03d.pgm", i);
        const auto path = std::filesystem::path(argv[2]) / name;
        Frame source(CImg<unsigned char>(path.string().c_str()));
        if (gzread(gz.get(), optical_bytes.data(), optical_bytes.size()) !=
            int(optical_bytes.size()))
            throw std::runtime_error("Truncated optical recording");
        Frame optical(optical_bytes);
        for (auto &site : sites) {
            if (i > site.comparison)
                continue;
            site.source.advance(source, i);
            site.optical.advance(optical, i);
            if (i == site.comparison) {
                double observed = overlap(source, optical, site);
                if (observed < .9 || std::abs(observed - site.expected_overlap) > .000001)
                    throw std::runtime_error(
                        "Source/optical daughter overlap differs from site table");
                std::printf("source=%d optical=%d comparison=%d overlap=%.6f\n", site.source.start,
                            site.optical.start, i, observed);
            }
        }
    }
} catch (const std::exception &e) {
    std::cerr << e.what() << '\n';
    return 1;
}
