#include <luxcore/luxcore.h>

#include <chrono>
#include <filesystem>
#include <iostream>
#include <memory>
#include <stdexcept>
#include <string>
#include <thread>

namespace fs = std::filesystem;
int main(int argc, char **argv) {
    try {
        if (argc != 4 || (std::string(argv[1]) != "cpu" && std::string(argv[1]) != "gpu"))
            throw std::runtime_error("Usage: luxcore_session_probe cpu|gpu CONFIG_BCF NEW_OUTPUT_PNG");
        const bool gpu = std::string(argv[1]) == "gpu";
        const fs::path bcf = fs::absolute(argv[2]);
        const fs::path output = fs::absolute(argv[3]);
        if (!fs::is_regular_file(bcf) || bcf.extension() != ".bcf")
            throw std::runtime_error("Expected a serialized LuxCore BCF configuration");
        if (!fs::is_directory(output.parent_path()) || fs::exists(output))
            throw std::runtime_error("Output parent must exist and output must not exist");

        luxcore::Init();
        luxcore::ClearFileNameResolverPaths();
        luxcore::AddFileNameResolverPath(bcf.parent_path().string());

        std::cout << "MODE " << (gpu ? "gpu" : "cpu") << "\nOUTPUT " << output.string() << std::endl;
        std::cout << "CONFIG " << bcf.string() << std::endl;
        const auto before_config = std::chrono::steady_clock::now();
        auto config = luxcore::RenderConfig::Create(bcf.string());
        std::cout << "CONFIG_LOADED_MS "
                  << std::chrono::duration_cast<std::chrono::milliseconds>(
                         std::chrono::steady_clock::now() - before_config).count() << std::endl;
        auto session = luxcore::RenderSession::Create(config);
        const auto before_start = std::chrono::steady_clock::now();
        std::cout << "START_BEGIN" << std::endl;
        session->Start();
        const auto started = std::chrono::steady_clock::now();
        std::cout << "START_RETURNED_MS "
                  << std::chrono::duration_cast<std::chrono::milliseconds>(started - before_start).count() << std::endl;
        std::this_thread::sleep_until(started + std::chrono::seconds(5));
        std::cout << "STOP_BEGIN" << std::endl;
        session->Stop();
        const auto stopped = std::chrono::steady_clock::now();
        std::cout << "STOP_RETURNED_AFTER_START_MS "
                  << std::chrono::duration_cast<std::chrono::milliseconds>(stopped - started).count() << std::endl;
        session->GetFilm().SaveOutputs();
        if (!fs::is_regular_file(output) || fs::file_size(output) == 0)
            throw std::runtime_error("LuxCore returned without a nonempty output file");
        std::cout << "OUTPUT_BYTES " << fs::file_size(output) << "\nDONE" << std::endl;
        return 0;
    } catch (const std::exception &err) {
        std::cerr << "FAILED " << err.what() << std::endl;
        return 1;
    }
}
