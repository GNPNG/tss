// Independent, art-directed initial phase masks for the confined slide study.
#pragma once

#include <cmath>
#include <cstdint>
#include <iomanip>
#include <initializer_list>
#include <locale>
#include <sstream>
#include <stdexcept>
#include <string>
#include <vector>

namespace tss::look {

namespace detail {

constexpr double tau = 6.2831853071795864769;

struct Point {
    double x, y;
};

inline Point mix(Point a, Point b, double t) {
    return {a.x + (b.x - a.x) * t, a.y + (b.y - a.y) * t};
}

inline Point junction(Point a, Point b, Point c) {
    return {(a.x + 4*b.x + c.x)/6, (a.y + 4*b.y + c.y)/6};
}

class Plate {
public:
    explicit Plate(bool world = false) {
        svg.imbue(std::locale::classic());
        svg << std::fixed << std::setprecision(3);
        if (world) {
            svg << "<svg xmlns='http://www.w3.org/2000/svg' width='1320' height='1320' viewBox='-180 -390 1320 1320'>"
                   "<rect x='-180' y='-390' width='1320' height='1320' fill='black'/>";
        } else {
            svg << "<svg xmlns='http://www.w3.org/2000/svg' width='960' height='540' viewBox='0 0 960 540'>"
                   "<rect width='960' height='540' fill='black'/>";
        }
    }

    void contour(std::initializer_list<Point> cage, bool dye=true) {
        const std::vector<Point> points(cage);
        const std::size_t count = points.size();
        svg << "<path fill='" << (dye ? "white" : "black") << "' d='";
        for (std::size_t i=0; i<count; ++i) {
            Point a = points[(i + count - 1) % count];
            Point b = points[i], c = points[(i + 1) % count], d = points[(i + 2) % count];
            Point start = junction(a,b,c), end = junction(b,c,d);
            Point first = mix(b,c,1.0/3), second = mix(b,c,2.0/3);
            if (i == 0) {
                svg << 'M';
                append(start);
            }
            svg << 'C'; append(first); append(second); append(end);
        }
        svg << "Z'/>";
    }

    void bead(double x, double y, double rx, double ry, bool dye=true) {
        svg << "<ellipse cx='" << x << "' cy='" << y << "' rx='" << rx << "' ry='" << ry
            << "' fill='" << (dye ? "white" : "black") << "'/>";
    }

    void inclusions(std::uint32_t seed, int count, Point centre, Point extent, bool dye) {
        auto sample = [&seed]() {
            seed = 1664525u*seed + 1013904223u;
            return static_cast<double>(seed) / 4294967296.0;
        };
        for (int i=0; i<count; ++i) {
            double r = std::sqrt(sample()), a = tau * sample();
            double radius = 0.55 + 3.0 * std::pow(sample(), 5.0);
            double x = centre.x + std::cos(a)*extent.x*r;
            double y = centre.y + std::sin(a)*extent.y*r;
            double aspect = 0.94 + 0.15*sample();
            bead(x, y, radius, radius*aspect, dye);
        }
    }

    std::string finish() {
        svg << "</svg>";
        return svg.str();
    }

private:
    void append(Point p) {
        svg << p.x << ' ' << p.y << ' ';
    }

    std::ostringstream svg;
};

inline void pink(Plate& s) {
    // A ramified film enters from the left. Its upper peninsula closes around a clear
    // inlet. The narrow southern throat terminates in a much broader lower puddle.
    s.contour({{-160,-118},{-63,-76},{15,-27},{44,41},{97,89},{163,127},
               {213,143},{255,139},{277,112},{280,66},{285,16},{327,-42},
               {442,-66},{551,-37},{597,6},{585,38},{552,66},{502,91},
               {464,125},{435,172},{389,214},{332,241},{303,264},{292,290},
               {295,306},{287,319},{271,317},{257,296},{242,285},{227,287},
               {213,301},{208,333},{214,373},{231,409},{261,431},{309,446},
               {335,466},{351,520},{326,595},{48,609},{-133,584},{-123,412},
               {-31,356},{34,307},{84,277},{125,240},{136,202},{126,164},
               {96,140},{61,123},{5,121},{-63,131},{-137,156}});
    // A separate mid-depth patch leaves a slim clear trough along the large lower field.
    s.contour({{476,224},{496,181},{530,152},{571,151},{605,166},{624,197},
               {628,238},{618,262},{602,290},{575,315},{547,319},{522,309},
               {506,290},{486,259}});
    s.contour({{265,326},{289,300},{318,293},{352,298},{376,317},{374,342},
               {352,365},{322,377},{288,373},{268,358}});
    s.contour({{484,607},{478,552},{471,518},{448,490},{426,468},{426,451},
               {442,435},{486,426},{546,427},{589,420},{621,402},{642,384},
               {653,357},{651,328},{644,294},{651,274},{670,264},{697,272},
               {726,296},{751,321},{780,340},{831,350},{878,345},{937,318},
               {992,294},{1047,334},{1115,585},{917,634},{638,628}});
    s.contour({{888,8},{902,-51},{1036,-80},{1055,249},{999,265},{953,239},
               {931,203},{925,169},{933,128},{934,92},{923,58},{896,33}});
    // Detached low-area droplets between the lobes.
    s.bead(402,381,14,11); s.bead(433,369,6.2,7.5); s.bead(383,406,5.2,4.7);
    s.bead(269,273,5.2,5.7,false); s.bead(328,198,2.7,2.5,false);
    s.inclusions(3077,18,{188,195},{97,42},false);
    s.inclusions(803,22,{490,102},{85,26},false);
    s.inclusions(920,20,{345,405},{60,21},true);
    s.inclusions(429,24,{752,427},{74,32},false);
    s.inclusions(4007,32,{719,195},{96,52},true);
}

inline void cyan(Plate& s) {
    // The upper bank and right-hand sheet share one continuous dyed region, with
    // a peninsula curling below the central optical cross-over.
    s.contour({{321,-99},{501,-100},{718,-105},{1049,-98},{1093,225},{1074,372},
               {1008,393},{963,383},{918,362},{892,334},{889,291},{894,251},
               {875,224},{852,223},{833,243},{817,279},{800,317},{779,354},
               {755,374},{723,382},{687,371},{664,348},{649,317},{639,288},
               {628,272},{611,271},{595,283},{584,302},{559,317},{522,322},
               {490,315},{461,296},{444,267},{442,232},{448,197},{465,170},
               {494,151},{531,145},{569,143},{597,132},{617,108},{632,81},
               {643,50},{642,25},{628,8},{608,1},{581,2},{548,13},
               {518,27},{478,40},{446,42},{418,37},{398,26},{385,6},
               {384,-22},{371,-53}});
    // A curved, narrower body passes beneath the pink patch and carries a string of beads.
    s.contour({{272,347},{280,318},{301,298},{333,290},{367,290},{394,282},
               {412,263},{427,240},{444,227},{465,228},{485,241},{493,271},
               {499,300},{514,315},{539,326},{567,333},{590,351},{603,377},
               {602,397},{584,410},{560,408},{531,396},{505,381},{477,369},
               {442,365},{404,366},{367,379},{329,388},{295,384},{278,371}});
    s.contour({{703,594},{703,542},{716,507},{746,486},{791,472},
               {828,456},{857,432},{885,418},{913,416},{945,428},{988,427},
               {1041,401},{1102,431},{1086,616},{878,634}});
    s.contour({{-94,180},{-43,177},{8,190},{39,217},{48,247},{40,278},
               {21,298},{-10,309},{-49,307},{-90,285}});
    s.contour({{82,-67},{123,-73},{183,-50},{209,-11},{211,24},{198,49},
               {174,61},{145,54},{125,38},{118,14},{115,-10}});
    s.contour({{153,473},{160,444},{177,425},{198,426},{223,451},{231,486},
               {227,510},{214,522},{192,516},{164,499}});
    s.contour({{622,473},{630,451},{650,432},{679,411},{701,405},{719,412},
               {723,433},{715,456},{700,474},{677,490},{650,493},{631,487}});
    s.bead(412,396,16.5,16); s.bead(437,385,7.5,7.8); s.bead(466,400,4.5,4.8);
    s.bead(258,394,7,6.5); s.bead(247,406,2.7,2.5);
    s.bead(305,59,5.5,4.6); s.bead(311,53,1.5,1.7);
    s.bead(903,143,8.8,8.4,false); s.bead(891,151,2.2,2.0,false);
    s.inclusions(3041,26,{316,66},{99,30},true);
    s.inclusions(1422,21,{251,406},{55,64},true);
    s.inclusions(5618,20,{704,66},{82,36},false);
    s.inclusions(6126,19,{950,309},{58,21},false);
}

} // namespace detail

inline std::string initial_slide_mask_svg(unsigned layer, bool world = false) {
    if (layer > 1) throw std::invalid_argument("Expected slide 0 or 1");
    detail::Plate slide(world);
    if (layer == 0) detail::pink(slide);
    else detail::cyan(slide);
    return slide.finish();
}

} // namespace tss::look
