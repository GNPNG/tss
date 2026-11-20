// Read display-encoded RGB24 frames on stdin. Find one-frame colour excursions
// which return close to the preceding colour. Motion can also satisfy this test.
#include <algorithm>
#include <cmath>
#include <cstdint>
#include <iostream>
#include <stdexcept>
#include <string>
#include <vector>
#ifdef _WIN32
#include <fcntl.h>
#include <io.h>
#endif

using Frame = std::vector<uint8_t>;

bool read_frame(Frame& frame) {
  std::cin.read(reinterpret_cast<char*>(frame.data()), frame.size());
  if (std::cin.gcount() == static_cast<std::streamsize>(frame.size())) return true;
  if (std::cin.gcount() || !std::cin.eof()) throw std::runtime_error("Incomplete RGB frame");
  return false;
}

void measure(const Frame& a, const Frame& b, const Frame& c, int w, int h,
             int frame, int threshold) {
  Frame mask(w*h);
  int pixels = 0;
  for (int i=0; i<w*h; ++i) {
    int end_delta=0, excursion=0;
    for (int k=3*i; k<3*i+3; ++k) {
      end_delta=std::max(end_delta,std::abs(a[k]-c[k]));
      int l=b[k]-a[k], r=b[k]-c[k];
      if ((l>0 && r>0) || (l<0 && r<0))
        excursion=std::max(excursion,std::min(std::abs(l),std::abs(r)));
    }
    if (end_delta<=4 && excursion>=threshold) { mask[i]=1; ++pixels; }
  }
  std::vector<int> queue;
  int largest=0, largest_x=0, largest_y=0;
  for (int i=0; i<w*h; ++i) {
    if (!mask[i]) continue;
    queue.clear();
    queue.push_back(i);
    mask[i]=0;
    for (size_t j=0; j<queue.size(); ++j) {
      int p=queue[j];
      auto visit=[&](int n) {
        if (mask[n]) { mask[n]=0; queue.push_back(n); }
      };
      if (p%w>0) visit(p-1);
      if (p%w+1<w) visit(p+1);
      if (p>=w) visit(p-w);
      if (p+w<w*h) visit(p+w);
    }
    if (int(queue.size())>largest) {
      largest=int(queue.size());
      largest_x=i%w;
      largest_y=i/w;
    }
  }
  std::cout<<frame<<','<<threshold<<','<<pixels<<','<<largest<<','<<largest_x<<','<<largest_y<<'\n';
}

int main(int argc, char** argv) try {
  if (argc!=3 && argc!=4)
    throw std::runtime_error("temporal-contrast width height [temporal_radius] < capture.rgb24");
  int w=std::stoi(argv[1]),h=std::stoi(argv[2]);
  int radius=argc==4 ? std::stoi(argv[3]) : 1;
  if (w<1||h<1||w>8192||h>8192) throw std::runtime_error("Invalid dimensions");
  if (radius<1||radius>8) throw std::runtime_error("Temporal radius must be 1 to 8 frames");
#ifdef _WIN32
  _setmode(_fileno(stdin),_O_BINARY);
#endif
  std::ios::sync_with_stdio(false);
  const int span=2*radius+1;
  std::vector<Frame> frames(span,Frame(3*w*h));
  for (auto& f : frames)
    if (!read_frame(f)) throw std::runtime_error("Input shorter than requested time window");
  std::cout<<"center_frame,excursion_codes,pixels,largest_component,first_x,first_y\n";
  int start=0;
  do {
    for (int threshold : {16,32,64})
      measure(frames[start%span], frames[(start+radius)%span],
              frames[(start+2*radius)%span], w,h,start+radius,threshold);
    ++start;
  } while (read_frame(frames[(start+2*radius)%span]));
  return 0;
} catch (const std::exception& e) {
  std::cerr<<e.what()<<'\n';
  return 1;
}
