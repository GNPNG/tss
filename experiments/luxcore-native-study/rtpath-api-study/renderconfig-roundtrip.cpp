#include <luxcore/luxcore.h>
#include <luxrays/utils/properties.h>

#include <filesystem>
#include <iostream>
#include <memory>
#include <stdexcept>
#include <string>

namespace fs = std::filesystem;

// CPU-only archive regression: this code does not create a render session.
int main(int argc, char **argv) {
    try {
        if (argc != 5)
            throw std::runtime_error("usage: test CONFIG SCENE OUTPUT_DIRECTORY internal|external");
        const fs::path configFile = fs::absolute(argv[1]);
        const fs::path sceneFile = fs::absolute(argv[2]);
        const fs::path outputDirectory = fs::absolute(argv[3]);
        const std::string mode(argv[4]);
        if ((mode != "internal" && mode != "external") || !fs::is_regular_file(configFile) ||
            !fs::is_regular_file(sceneFile) || !fs::is_directory(outputDirectory))
            throw std::runtime_error("invalid inputs");
        const fs::path archive = outputDirectory / (mode + ".bcf");
        if (fs::exists(archive))
            throw std::runtime_error("archive already exists");

        luxcore::Init();
        auto props = std::make_unique<luxrays::Properties>(configFile.string());
        props->Set(luxrays::Property("scene.file")(sceneFile.string()));
        props->Set(luxrays::Property("renderengine.type")("PATHCPU"));
        props->Set(luxrays::Property("film.hw.enable")(false));
        props->Set(luxrays::Property("film.width")(64));
        props->Set(luxrays::Property("film.height")(48));

        std::unique_ptr<luxcore::Scene> externalScene;
        std::unique_ptr<luxcore::RenderConfig> original;
        if (mode == "external") {
            externalScene = luxcore::Scene::Create(sceneFile.string());
            original = luxcore::RenderConfig::Create(std::move(props), externalScene);
        } else {
            original = luxcore::RenderConfig::Create(std::move(props));
        }
        const auto scenePropertyCount = original->GetScene().ToProperties()->GetAllNames().size();
        if (!scenePropertyCount)
            throw std::runtime_error("original scene is empty");
        original->Save(archive.string());
        if (!fs::is_regular_file(archive) || fs::file_size(archive) == 0)
            throw std::runtime_error("archive was not written");
        original.reset();
        externalScene.reset();

        auto loaded = luxcore::RenderConfig::Create(archive.string());
        if (loaded->GetProperty("renderengine.type").Get<std::string>() != "PATHCPU" ||
            loaded->GetProperty("film.width").Get<unsigned int>() != 64 ||
            loaded->GetProperty("film.height").Get<unsigned int>() != 48 ||
            loaded->GetScene().ToProperties()->GetAllNames().empty())
            throw std::runtime_error("loaded properties or scene do not match expectations");
        std::cout << "ROUNDTRIP_OK mode=" << mode << " archive_bytes=" << fs::file_size(archive)
                  << " original_scene_property_count=" << scenePropertyCount << std::endl;
        return 0;
    } catch (const std::exception &e) {
        std::cerr << "ROUNDTRIP_FAILED " << e.what() << std::endl;
        return 1;
    }
}
