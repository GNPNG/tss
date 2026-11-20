// Isolates Boost.Serialization value/pointer framing; this is not LuxCore's
// portable archive and is not a replacement for an upstream integration test.
#include <boost/archive/binary_iarchive.hpp>
#include <boost/archive/binary_oarchive.hpp>
#include <boost/serialization/unique_ptr.hpp>

#include <iostream>
#include <memory>
#include <sstream>
#include <stdexcept>
#include <string>

struct Sample {
    explicit Sample(int v) : value(v) {}
    int value;
    template <class Archive> void serialize(Archive &, unsigned int) {}
};

namespace boost::serialization {
template <class Archive>
void save_construct_data(Archive &ar, const Sample *sample, unsigned int) {
    ar << sample->value;
}
template <class Archive>
void load_construct_data(Archive &ar, Sample *sample, unsigned int) {
    int value = 0;
    ar >> value;
    ::new (sample) Sample(value);
}
} // namespace boost::serialization

enum class Representation { value, raw_pointer, unique_pointer };

bool roundtrip(Representation mode) {
    std::stringstream stream(std::ios::in | std::ios::out | std::ios::binary);
    {
        boost::archive::binary_oarchive writer(stream);
        if (mode == Representation::unique_pointer) {
            const auto original = std::make_unique<Sample>(42);
            writer << original;
        } else {
            const Sample original(42);
            if (mode == Representation::raw_pointer) {
                const Sample *ptr = &original;
                writer << ptr;
            } else {
                writer << original;
            }
        }
    }
    stream.seekg(0);
    std::unique_ptr<Sample> result;
    try {
        boost::archive::binary_iarchive reader(stream);
        reader >> result;
        return result && result->value == 42;
    } catch (const std::exception &err) {
        std::cout << "READ_EXCEPTION " << err.what() << '\n';
        return false;
    }
}

int main(int argc, char **argv) {
    if (argc != 2) {
        std::cerr << "Usage: serialization_repro value|raw|unique" << std::endl;
        return 2;
    }
    const std::string mode(argv[1]);
    Representation representation;
    if (mode == "value") representation = Representation::value;
    else if (mode == "raw") representation = Representation::raw_pointer;
    else if (mode == "unique") representation = Representation::unique_pointer;
    else return 2;
    std::cout << "BEGIN " << mode << std::endl;
    const bool result = roundtrip(representation);
    std::cout << "ROUNDTRIP " << mode << " " << result << std::endl;
    return result ? 0 : 1;
}
