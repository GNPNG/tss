#include "../slide-look-study/optical-distance.hpp"
#include <cstdint>

// Rust validates the dimensions and buffer sizes. Fill only this cell's channel.
extern "C" int tss_live_optical_channel(const std::uint8_t* coverage, int source_side,
                                         int side, int gate_height, unsigned channel,
                                         std::uint8_t* rg) noexcept {
    try {
        CImg<float> phase(coverage, source_side, source_side, 1, 1);
        const auto field = optical_distance(phase, side, side);
        for (int i = 0; i < side * side; ++i) {
            double design_distance = field[i] * 720. / gate_height;
            rg[2*i + channel] = static_cast<std::uint8_t>(std::lround(
                255. * std::clamp(.5 + design_distance / 96., 0., 1.)));
        }
        return 0;
    } catch (const std::exception&) {
        return -1;
    }
}
