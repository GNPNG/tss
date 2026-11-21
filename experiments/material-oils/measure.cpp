// CPU contour comparison on the original, independently reconstructed VOF masks.
#define cimg_display 0
#include <CImg.h>
#include <algorithm>
#include <cmath>
#include <iomanip>
#include <iostream>
#include <stdexcept>
#include <vector>
using cimg_library::CImg;

struct Interface {
    CImg<unsigned char> phase, edge;
    CImg<float> distance;
    explicit Interface(const char* file) : phase(file) {
        if (phase.width()!=1024 || phase.height()!=1024 || phase.spectrum()!=1)
            throw std::runtime_error("Expected a full-world 1024-square VOF coverage PGM");
        phase.threshold(128);
        edge.assign(1024,1024,1,1,0);
        cimg_forXY(phase,x,y) {
            // Both pixels adjoining each face are marked. Outside the simulation
            // and projection-window edges are not treated as material boundaries.
            if ((x && phase(x-1,y)!=phase(x,y)) || (y && phase(x,y-1)!=phase(x,y)) ||
                (x+1<1024 && phase(x+1,y)!=phase(x,y)) || (y+1<1024 && phase(x,y+1)!=phase(x,y)))
                edge(x,y)=1;
        }
        distance=CImg<float>(edge).get_distance(1.0f,2);
    }
};

int main(int argc,char** argv) try {
    if(argc!=3) throw std::runtime_error("Usage: measure BASELINE.pgm OTHER.pgm");
    const Interface a(argv[1]),b(argv[2]);
    unsigned count=0, changed=0, a_pixels=0, b_pixels=0;
    double sum=0;
    std::vector<float> samples;
    cimg_forXY(a.phase,x,y) {
        const double x704=(x+.5)*704./1024.,y704=(y+.5)*704./1024.;
        if(x704<96 || x704>=608 || y704<208 || y704>=496) continue;
        ++count;
        changed+=a.phase(x,y)!=b.phase(x,y);
        a_pixels+=a.phase(x,y);b_pixels+=b.phase(x,y);
        if(a.edge(x,y)) samples.push_back(b.distance(x,y));
        if(b.edge(x,y)) samples.push_back(a.distance(x,y));
    }
    if(samples.empty()) throw std::runtime_error("No interface samples in gate");
    std::sort(samples.begin(),samples.end());
    for (float v:samples) sum+=v;
    constexpr double pitch_mm=68.75/1024.;
    std::cout<<std::setprecision(10)<<"{\"gatePixels\":"<<count<<",\"xorPixels\":"<<changed
        <<",\"xorFraction\":"<<double(changed)/count<<",\"coverageA\":"<<double(a_pixels)/count
        <<",\"coverageB\":"<<double(b_pixels)/count<<",\"boundarySamples\":"<<samples.size()
        <<",\"meanBoundaryMm\":"<<pitch_mm*sum/samples.size()<<",\"p95BoundaryMm\":"
        <<pitch_mm*samples[std::ceil(.95*samples.size())-1]<<",\"maxBoundaryMm\":"<<pitch_mm*samples.back()<<"}\n";
} catch(const std::exception& e) {std::cerr<<e.what()<<"\n";return 1;}
