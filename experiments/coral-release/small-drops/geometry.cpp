// Initial dyed phase on a sealed slide, in the 1024 x 1024 source world.
#include <algorithm>
#include <cmath>
#include <iostream>
#include <random>

namespace {
double bank(double x) {
    return 640. + 56. * std::sin((x - 340.) / 111.) + 35. * std::sin((x + 60.) / 240.);
}
void ellipse(double x, double y, double rx, double ry) {
    std::cout << "<ellipse cx='" << x << "' cy='" << y << "' rx='" << rx << "' ry='" << ry << "'/>";
}
void bud(double x, double r, double length, double waist, double tilt) {
    std::cout << "<g transform='translate(" << x << ' ' << bank(x) + 9. << ") rotate(" << tilt
              << ")'>"
              << "<path d='M " << -1.35 * r << " 8 C " << -.85 * r << " -6 " << -waist << ' '
              << -.22 * length << ' ' << -waist << ' ' << -.43 * length << " S " << -.65 * waist
              << ' ' << -.77 * length << ' ' << -.63 * r << ' ' << -length << " L " << .63 * r
              << ' ' << -length << " Q " << waist << ' ' << -.77 * length << ' ' << waist << ' '
              << -.43 * length << " Q " << waist << " -7 " << 1.35 * r << " 8 Z'/>";
    ellipse(0., -length - .5 * r, r, 1.08 * r);
    std::cout << "</g>";
}
void late_bud(double x, double radius, double length, double neck, double angle) {
    double y = bank(x) + 18.;
    std::cout << "<g transform='translate(" << x << ' ' << y << ") rotate(" << angle << ")'>"
              << "<path d='M " << -1.5 * radius << " 12 Q " << -neck << " -16 " << -neck << ' '
              << -.5 * length << " Q " << -neck << ' ' << -.8 * length << ' ' << -.67 * radius
              << ' ' << -length << " L " << .67 * radius << ' ' << -length << " Q " << neck << ' '
              << -.8 * length << ' ' << neck << ' ' << -.5 * length << " Q " << neck << " -16 "
              << 1.5 * radius << " 12 Z'/>";
    ellipse(0., -length - .55 * radius, radius, 1.13 * radius);
    std::cout << "</g>";
}
} // namespace
int main() {
    std::mt19937 rng(2961);
    auto noise = [&]() { return double(rng()) / 4294967296.; };
    std::cout
        << "<svg xmlns='http://www.w3.org/2000/svg' width='1024' height='1024'>"
           "<rect width='1024' height='1024' fill='black'/><g fill='white'><path d='M -8 1032 ";
    for (int x = -8; x <= 1032; x += 4)
        std::cout << "L " << x << ' ' << bank(x) << ' ';
    std::cout << "L 1032 1032 Z'/>";
    for (double x = 65.; x < 980.;) {
        double r = 4.9 + 4.8 * noise(), length = 22. + 51. * noise(), waist = 1.1 + 1.1 * noise();
        double tilt = 7. * std::sin(x / 76.) + 13. * (noise() - .5);
        if (x < 300. || x > 480.) {
            const double cycle = (1. + std::sin(x / 19.)) * .5;
            length = 51. + length * .62 + cycle * 11.;
            r = 5.8 + r * .60;
            waist = .99 + .85 * cycle;
            bud(x, r, length, waist, tilt);
        }

        x += 15. + 17. * noise();
    }
    late_bud(324, 12, 121, 2.1, 22);
    late_bud(369, 8, 100, 2.0, 9);
    late_bud(387, 14, 142, 2.3, -7);
    late_bud(439, 23, 108, 2.9, 22);
    // Free material in the upper carrier is already present in the opening pose.
    for (int i = 0; i < 68; ++i) {
        double x = 66. + 895. * noise(), y = bank(x) - 65. - 355. * noise();
        double r = 1.65 + 3.1 * noise() * noise();
        ellipse(x, y, r, r * (.94 + .12 * noise()));
    }
    std::cout << "</g></svg>\n";
    return std::cout ? 0 : 1;
}
