// Compare the two 48 kHz mono PCM decodes with the FFT supplied by FFmpeg.
extern "C" {
#include <libavutil/tx.h>
}
#include <algorithm>
#include <cmath>
#include <cstdio>
#include <fstream>
#include <stdexcept>
#include <vector>

constexpr int bins = 8192;
constexpr int rate = 48000;

static double cue_peak(const char *path, double start) {
    std::ifstream file(path, std::ios::binary);
    file.seekg(static_cast<std::streamoff>(std::llround(start * rate)) * sizeof(float));
    std::vector<float> pcm(bins);
    if (!file.read(reinterpret_cast<char *>(pcm.data()), bins * sizeof(float)))
        throw std::runtime_error("Short PCM window");
    std::vector<AVComplexFloat> in(bins), out(bins);
    const double pi = std::acos(-1.0);
    for (int i = 0; i < bins; ++i)
        in[i].re = pcm[i] * (.5 - .5 * std::cos(2 * pi * i / (bins - 1)));
    av_tx_fn fft{};
    AVTXContext *context{};
    if (av_tx_init(&context, &fft, AV_TX_FLOAT_FFT, 0, bins, nullptr, AV_TX_UNALIGNED) < 0)
        throw std::runtime_error("FFmpeg FFT initialization failed");
    fft(context, out.data(), in.data(), sizeof(AVComplexFloat));
    av_tx_uninit(&context);
    auto power = [&](int k) {return std::hypot(static_cast<double>(out[k].re),out[k].im);};
    int peak = 1;
    for (int k = 2; k < bins/2; ++k) if (power(k) > power(peak)) peak = k;
    const auto log_power = [&](int k) {return std::log(std::max(power(k),1e-30));};
    const double a=log_power(peak-1), b=log_power(peak), c=log_power(peak+1);
    const double offset = .5 * (a-c) / (a-2*b+c);
    return (peak + offset) * rate / bins;
}
int main(int argc, char **argv) {
    if (argc != 3) return 2;
    std::puts("start_s,expected_hz,source_peak_hz,export_peak_hz");
    for (int i=0; i<2; ++i) {
        const double start = i==0 ? .35 : 1.20;
        const double expected = i==0 ? 220 : 330;
        const double src=cue_peak(argv[1],start), dst=cue_peak(argv[2],start);
        std::printf("%.2f,%.2f,%.5f,%.5f\n",start,expected,src,dst);
        if (std::abs(src-expected)>1. || std::abs(dst-expected)>1.) return 1;
    }
}
