// Art-directed initial conditions for two independent Basilisk VOF solves.
// World and gate match the native optical cache. White denotes dyed material.
#include <cmath>
#include <iostream>
#include <string_view>

namespace {
double bank(double x) {
    return 640. + 56.*std::sin((x - 340.)/111.) + 35.*std::sin((x + 60.)/240.);
}

void droplet(double x, double y, double rx, double ry) {
    std::cout << "<ellipse cx='" << x << "' cy='" << y << "' rx='" << rx
              << "' ry='" << ry << "'/>";
}

// A round bud and a waisted ligament have a single continuous interface.
// The initial neck widths and the subsequent local strain vary along the bank.
void bud(double x, double radius, double length, double neck, double angle) {
    double y = bank(x) + 18.;
    std::cout << "<g transform='translate(" << x << ' ' << y << ") rotate(" << angle << ")'>"
              << "<path d='M " << -1.5*radius << " 12 Q " << -neck << " -16 "
              << -neck << ' ' << -.5*length << " Q " << -neck << ' ' << -.8*length
              << ' ' << -.67*radius << ' ' << -length << " L " << .67*radius << ' ' << -length
              << " Q " << neck << ' ' << -.8*length << ' ' << neck << ' ' << -.5*length
              << " Q " << neck << " -16 " << 1.5*radius << " 12 Z'/>";
    droplet(0., -length-.55*radius, radius, 1.13*radius);
    std::cout << "</g>";
}

void front() {
    std::cout << "<path d='M -8 1032 ";
    for (int x = -8; x <= 1032; x += 4)
        std::cout << "L " << x << ' ' << bank(x) << ' ';
    std::cout << "L 1032 1032 Z'/>";
    bud(185, 9, 44, 3.7, -18);
    bud(254, 18, 81, 3.3, 19);
    bud(324, 12, 121, 2.1, 22);
    bud(369, 8, 100, 2.0, 9);
    bud(387, 14, 142, 2.3, -7);
    bud(439, 23, 108, 2.9, 22);
    bud(515, 10, 61, 4.2, -8);
    bud(586, 16, 89, 4.6, 31);
    bud(684, 24, 107, 3.8, 31);
    bud(775, 9, 42, 4.0, 18);
    bud(852, 10, 64, 3.3, -19);
    // Detached droplets carry the local current before the first neck opens.
    droplet(194,424, 6,6); droplet(219,414, 3,3); droplet(265,463, 7,8);
    droplet(290,392, 4,4); droplet(314,470, 11,12); droplet(349,419, 6,6);
    droplet(389,441, 9,9); droplet(411,500, 4,4); droplet(475,431, 4,4);
    droplet(453,385, 9,10); droplet(538,456, 6,6); droplet(566,478, 10,9);
    droplet(625,459, 4,4); droplet(675,410, 7,7); droplet(738,435, 4,4);
    droplet(804,393, 8,8); droplet(835,372, 4,4); droplet(691,337, 4,4);
    droplet(329,448, 3,3); droplet(336,440, 2.8,2.8); droplet(387,366, 5,5.5);
    droplet(539,559, 3.5,4); droplet(680,439, 3,3); droplet(653,456, 2.8,2.8);
    droplet(296,429, 3.8,3.8); droplet(473,527, 5,6); droplet(393,338, 2.8,2.8);
}
void rear() {
    // Long folds continue beyond the gate. Open carrier channels separate them.
    std::cout << "<path d='M -25 355 C 148 304 189 380 284 402 "
        "S 430 398 472 337 C 530 251 610 236 690 261 "
        "C 563 274 558 369 485 423 C 407 485 320 481 227 444 "
        "C 115 401 54 427 -25 482 Z'/>"
        "<path d='M 1060 325 C 958 335 904 399 867 456 C 800 565 725 574 651 558 "
        "C 569 540 545 497 490 503 C 537 476 577 501 638 513 "
        "C 721 530 755 495 797 430 C 864 327 944 282 1060 270 Z'/>"
        "<path d='M -30 797 C 87 670 153 673 245 681 C 348 692 406 650 429 614 "
        "C 401 706 309 742 246 744 C 145 746 91 768 -30 880 Z'/>"
        "<path d='M 1090 581 C 902 565 881 644 817 682 C 762 716 679 695 615 733 "
        "C 687 718 753 772 844 725 C 943 674 965 648 1090 673 Z'/>";
}
} // namespace

int main(int argc, char** argv) {
    if (argc != 2 || (std::string_view(argv[1]) != "rear" && std::string_view(argv[1]) != "front"))
        return 2;
    std::cout << "<svg xmlns='http://www.w3.org/2000/svg' width='1024' height='1024'>"
                 "<rect width='1024' height='1024' fill='black'/><g fill='white'>";
    if (std::string_view(argv[1]) == "rear") rear(); else front();
    std::cout << "</g></svg>\n";
    return std::cout ? 0 : 1;
}
