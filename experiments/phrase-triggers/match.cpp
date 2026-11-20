// Native byte-pattern to song-time mapping control. Offsets always refer to
// the exact normalized buffer delivered to Vectorscan.
#include <hs.h>
#include <array>
#include <cstdint>
#include <iostream>
#include <memory>
#include <stdexcept>
#include <string>
#include <vector>

struct Token { std::string label; uint64_t start, end; };
struct Match { unsigned id; uint64_t start, end; };
struct Scan { std::vector<uint64_t> starts, ends; std::vector<Match> hits; };
int matched(unsigned id, unsigned long long from, unsigned long long to, unsigned, void* opaque) {
    auto& result=*static_cast<Scan*>(opaque);
    if (to<=from || to>result.ends.size()) return 1;
    result.hits.push_back({id,result.starts[from],result.ends[to-1]});
    return 0;
}
void run(const hs_database_t* db, hs_scratch_t* scratch, const std::vector<Token>& tokens,
         unsigned id, const std::string& kind) {
    std::string bytes;
    Scan state;
    for (const auto& word:tokens) {
        if (word.label.find_first_of("<>")!=std::string::npos || word.end<word.start)
            throw std::runtime_error("Invalid fixture token");
        auto encoded="<"+word.label+">";
        bytes+=encoded;
        state.starts.insert(state.starts.end(),encoded.size(),word.start);
        state.ends.insert(state.ends.end(),encoded.size(),word.end);
    }
    if (hs_scan(db,bytes.data(),static_cast<unsigned>(bytes.size()),0,scratch,matched,&state)!=HS_SUCCESS)
        throw std::runtime_error("Vectorscan scan failed");
    if (state.hits.size()!=2) throw std::runtime_error("Expected two occurrences");
    unsigned count=0;
    for (const auto& hit:state.hits) {
        ++count;
        if (hit.id!=id || hit.start!=(count==1?48000u:192000u) || hit.end!=(count==1?96000u:264000u))
            throw std::runtime_error("Incorrect sample-time mapping");
        // A recorded tug at the third quarter of a matched span.
        auto gesture=hit.start+3*(hit.end-hit.start)/4;
        std::cout<<kind<<','<<count<<','<<hit.start<<','<<hit.end<<','<<gesture<<'\n';
    }
}

int main() try {
    const char* patterns[]={"<turn><the><wheel>","<kick><hat><snare>"};
    const size_t lengths[]={std::string(patterns[0]).size(),std::string(patterns[1]).size()};
    const unsigned flags[]={HS_FLAG_SOM_LEFTMOST,HS_FLAG_SOM_LEFTMOST},ids[]={1,2};
    hs_database_t* raw=nullptr;
    hs_compile_error_t* error=nullptr;
    if (hs_compile_lit_multi(patterns,flags,ids,lengths,2,HS_MODE_BLOCK,nullptr,&raw,&error)!=HS_SUCCESS) {
        std::string message=error?error->message:"Compile failed";
        hs_free_compile_error(error);
        throw std::runtime_error(message);
    }
    std::unique_ptr<hs_database_t,decltype(&hs_free_database)> db(raw,hs_free_database);
    hs_scratch_t* work=nullptr;
    if (hs_alloc_scratch(raw,&work)!=HS_SUCCESS) throw std::runtime_error("Scratch allocation failed");
    std::unique_ptr<hs_scratch_t,decltype(&hs_free_scratch)> scratch(work,hs_free_scratch);
    std::cout<<"source,hit,start_sample,end_sample,gesture_at_75_percent\n";
    for (unsigned i=0;i<2;++i) {
        std::array<std::string,3> labels=i ? std::array<std::string,3>{"kick","hat","snare"} :
                                            std::array<std::string,3>{"turn","the","wheel"};
        run(db.get(),scratch.get(),{{"préface",0,18000},{labels[0],48000,60000},
          {labels[1],64500,70000},{labels[2],71000,96000},{labels[0],120000,135000},
          {"decoy",136000,150000},{labels[0],192000,215000},{labels[1],216000,235000},
          {labels[2],238000,264000}}, i+1,i?"audio-events":"timed-lyrics");
    }
} catch (const std::exception& e) { std::cerr<<e.what()<<'\n'; return 1; }
