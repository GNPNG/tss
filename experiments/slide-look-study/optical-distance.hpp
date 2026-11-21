#pragma once
// Pack two independent phase-coverage images for the projector optics shader.
#define cimg_display 0
#include <CImg.h>
#include <algorithm>
#include <cmath>
#include <cstdio>
#include <stdexcept>
#include <string>

using cimg_library::CImg;

// Reconstruct the antialiased crossing before measuring it. Keep distances in
// the same 720p design units even when the physics grid has a different pitch.
inline CImg<float> optical_distance(const CImg<float>& fraction, int width, int height) {
    if (fraction.spectrum() != 1 || fraction.width() * height != fraction.height() * width)
        throw std::runtime_error("Expected a grayscale phase mask matching the output aspect ratio");
    CImg<float> binary = fraction.get_resize(width, height, 1, 1, 5).threshold(127.5f);
    const auto inside = binary.get_distance(0.0f, 2);
    const auto outside = binary.get_distance(1.0f, 2);
    CImg<float> distance(width, height);
    cimg_forXY(distance, x, y) {
        float d = inside(x, y) - outside(x, y);
        distance(x, y) = d - std::copysign(0.5f, d);
    }
    // Reconstruct a smooth optical boundary across cell centers. This has no
    // effect on the phase field used by the solver.
    distance.blur(0.8f, 0.8f, 0.0f, 1);
    return distance;
}

inline CImg<float> optical_distance(const char* path, int width, int height) {
    return optical_distance(CImg<float>(path), width, height);
}

