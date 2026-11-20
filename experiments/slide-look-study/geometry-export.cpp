#include "geometry.hpp"

#include <iostream>
#include <string_view>

int main(int argc, char** argv) {
    if ((argc != 2 && !(argc == 3 && std::string_view(argv[2]) == "--world")) || (std::string_view(argv[1]) != "violet" &&
                      std::string_view(argv[1]) != "cyan")) {
        std::cerr << "Usage: geometry-export violet|cyan [--world] > plate.svg\n";
        return 2;
    }
    std::cout << tss::look::initial_slide_mask_svg(std::string_view(argv[1]) == "cyan", argc == 3) << '\n';
    return std::cout ? 0 : 1;
}
