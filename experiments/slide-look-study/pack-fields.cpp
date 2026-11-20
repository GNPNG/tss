#include "optical-distance.hpp"

int main(int argc, char** argv) {
    if (argc != 6) {
        std::fprintf(stderr, "pack-fields VIOLET.pgm CYAN.pgm WIDTH composite|violet|cyan OUTPUT.ppm\n");
        return 1;
    }
    try {
        const int width = std::stoi(argv[3]);
        if (width < 16 || width % 16 != 0 || width > 3840)
            throw std::runtime_error("Width must be a multiple of 16, up to 3840");
        const int height = width / 16 * 9;
        const std::string mode = argv[4];
        if (mode != "composite" && mode != "violet" && mode != "cyan")
            throw std::runtime_error("Unknown layer selection");
        CImg<unsigned short> packed(width, height, 1, 3, 0);
        for (int ch = 0; ch < 2; ++ch) {
            if ((ch == 0 && mode == "cyan") || (ch == 1 && mode == "violet"))
                continue;
            const auto d = optical_distance(argv[ch + 1], width, height);
            const double scale = 720.0 / height / 96.0;
            cimg_forXY(packed, x, y) {
                const double value = std::clamp(0.5 + d(x,y)*scale, 0.0, 1.0);
                packed(x,y,0,ch) = static_cast<unsigned short>(std::lround(value * 65535.0));
            }
        }
        packed.save_pnm(argv[5]);
    } catch (const std::exception& e) {
        std::fprintf(stderr, "%s\n", e.what());
        return 1;
    }
}
