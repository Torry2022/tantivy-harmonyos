// Public SDK consumer. Compile with SDK_BASELINE_ABI2 against the saved ABI2 header/library.
#include "tantivy_ohos.h"
#include <chrono>
#include <cstdint>
#include <cstdio>
#include <cstdlib>
#include <cstring>
#include <fstream>
#include <iomanip>
#include <iostream>
#include <map>
#include <memory>
#include <stdexcept>
#include <string>
#include <utility>
#include <vector>

static void Check(int status, char* error) {
    std::string message = error ? error : "";
    tantivy_ohos_free_string(error);
    if (status) throw std::runtime_error(message);
}
static std::vector<std::pair<std::string,std::string>> Read(const char* path) {
    std::ifstream f(path); if (!f) throw std::runtime_error("cannot read TSV");
    std::vector<std::pair<std::string,std::string>> rows;
    std::string line;
    while (std::getline(f,line)) {
        if (!line.empty() && line.back()=='\r') line.pop_back();
        const auto tab=line.find('\t');
        if (tab==std::string::npos || line.find('\t',tab+1)!=std::string::npos) throw std::runtime_error("expected two TSV columns");
        rows.emplace_back(line.substr(0,tab),line.substr(tab+1));
    }
    return rows;
}
#ifndef SDK_BASELINE_ABI2
static std::string Quote(const std::string& value) {
    static const char* hex="0123456789abcdef";
    std::string s="\"";
    for (unsigned char ch:value) {
        if (ch=='"' || ch=='\\') {s+='\\';s+=static_cast<char>(ch);}
        else if (ch<32) {s+="\\u00";s+=hex[ch>>4];s+=hex[ch&15];}
        else s+=static_cast<char>(ch);
    }
    return s+'"';
}
#endif
static std::uint64_t Hash(const char* s) {
    std::uint64_t value=14695981039346656037ULL;
    for (const unsigned char* p=reinterpret_cast<const unsigned char*>(s);*p;++p) value=(value^*p)*1099511628211ULL;
    return value;
}
static long Pss() {
    std::ifstream f("/proc/self/smaps_rollup"); std::string key,line; long value;
    while (std::getline(f,line)) {
        if (line.compare(0,4,"Pss:")==0 && std::sscanf(line.c_str(),"Pss: %ld",&value)==1) return value;
    }
    return -1; // No PSS on Windows; never substitute working set or RSS.
}
static tantivy_ohos_handle* Open(const char* path) {
    tantivy_ohos_handle* h=nullptr; char* e=nullptr;
    const tantivy_ohos_open_options_v2 o={sizeof(o),2,TANTIVY_OHOS_READ_ONLY};
    const int status=tantivy_ohos_open_v2(path,&o,&h,&e); Check(status,e); return h;
}
static tantivy_ohos_result* Search(tantivy_ohos_handle* h,const std::string& query,bool body,bool count,double* apiMicros=nullptr) {
    tantivy_ohos_result* r=nullptr; char* e=nullptr;
#ifdef SDK_BASELINE_ABI2
    if (count) throw std::runtime_error("baseline ABI2 cannot request exact count");
    const tantivy_ohos_search_options_v2 o={sizeof(o),2,body?TANTIVY_OHOS_PROJECT_DEFAULT:TANTIVY_OHOS_PROJECT_NONE,nullptr,0,nullptr,0};
    const auto started=std::chrono::steady_clock::now();
    const int status=tantivy_ohos_search_v2(h,query.c_str(),100,&o,&r,&e);
#else
    const std::string request="{\"version\":3,\"query\":"+Quote(query)+",\"limit\":100,\"projection\":\""+(body?"default":"none")+"\",\"track_total\":"+(count?"true":"false")+"}";
    const auto started=std::chrono::steady_clock::now();
    const int status=tantivy_ohos_search_v3(h,request.c_str(),&r,&e);
#endif
    if (apiMicros) *apiMicros=std::chrono::duration<double,std::micro>(std::chrono::steady_clock::now()-started).count();
    Check(status,e); return r;
}
static void Build(const char* path,const char* docs,const std::string& kind) {
    tantivy_ohos_handle* h=nullptr; char* e=nullptr;
#ifdef SDK_BASELINE_ABI2
    (void)kind;
    const tantivy_ohos_field_spec_v2 spec={"text",TANTIVY_OHOS_TEXT,15};
    const tantivy_ohos_create_options_v2 o={sizeof(o),2,&spec,1};
    const int status=tantivy_ohos_create_v2(path,&o,&h,&e);
#else
    const std::string config="{\"version\":3,\"tokenizer\":\"zh_jieba_v3\",\"fields\":[{\"name\":\"text\",\"kind\":1,\"flags\":15}],\"fast_id\":"+
        std::string(kind=="stored"?"false":"true")+(kind=="custom"?",\"dictionary\":{\"mode\":\"extend_default\",\"entries\":[{\"word\":\"鸿蒙星河检索\",\"frequency\":1000000000}]}":"")+"}";
    const int status=tantivy_ohos_create_v3(path,config.c_str(),&h,&e);
#endif
    Check(status,e);
    std::size_t n=0;
    for (const auto& row:Read(docs)) {
        const tantivy_ohos_field_value_v2 v={"text",TANTIVY_OHOS_TEXT,row.second.c_str(),0,0.0};
        e=nullptr; const int s=tantivy_ohos_upsert_v2(h,row.first.c_str(),&v,1,&e); Check(s,e);
        if (++n%5000==0) {e=nullptr; const int s2=tantivy_ohos_commit(h,&e); Check(s2,e);}
    }
    e=nullptr; const int s=tantivy_ohos_commit(h,&e); Check(s,e); tantivy_ohos_close(h);
    std::cout<<"built\t"<<n<<'\n';
}
static void Query(const char* path,const char* queries,const std::string& mode,int repeats,const char* docs) {
    const auto rows=Read(queries); const bool body=mode.find("body")!=std::string::npos;
    const bool count=mode.find("count")!=std::string::npos;
    std::map<std::string,std::string> originals;
    if (docs) for (const auto& row:Read(docs)) originals.emplace(row);
    auto* h=Open(path);
    // Full identical warmup outside timed samples. Parsing failures remain failures.
    for (const auto& row:rows) {try { auto* r=Search(h,row.second,body,count); tantivy_ohos_free_result_v2(r); } catch (const std::exception&) {}}
    for (int repeat=0;repeat<repeats;++repeat) for (const auto& row:rows) {
        const auto start=std::chrono::steady_clock::now();
        try {
            double searchMicros=0.0;
            std::unique_ptr<tantivy_ohos_result,decltype(&tantivy_ohos_free_result_v2)> owned(Search(h,row.second,body,count,&searchMicros),tantivy_ohos_free_result_v2);
            auto* r=owned.get(); char* e=nullptr; std::size_t n=0;
            const int s=tantivy_ohos_result_count_v2(r,&n,&e); Check(s,e);
            std::string lines; std::uint64_t total=0; bool exact=false;
#ifndef SDK_BASELINE_ABI2
            e=nullptr; const int st=tantivy_ohos_result_total_v4(r,&total,&exact,&e); Check(st,e);
#endif
            for (std::size_t i=0;i<n;++i) {
                tantivy_ohos_hit_view_v2 hit={}; e=nullptr;
                const int s2=tantivy_ohos_result_hit_v2(r,i,&hit,&e); Check(s2,e);
                std::uint32_t bits=0; std::memcpy(&bits,&hit.score,sizeof(bits));
                std::size_t bytes=0; std::uint64_t hash=0;
                if (body) {
                    tantivy_ohos_value_view_v2 v={}; e=nullptr;
                    const int s3=tantivy_ohos_result_value_v2(r,i,0,&v,&e); Check(s3,e);
                    bytes=std::strlen(v.string_value); hash=Hash(v.string_value);
                    if (docs && originals.at(hit.id)!=v.string_value) throw std::runtime_error("returned text differs from frozen original");
                }
                lines+="hit\t"+std::to_string(repeat)+"\t"+row.first+"\t"+std::to_string(i+1)+"\t"+hit.id+"\t"+std::to_string(bits)+"\t"+std::to_string(bytes)+"\t"+std::to_string(hash)+"\n";
            }
            owned.reset();
            const double micros=std::chrono::duration<double,std::micro>(std::chrono::steady_clock::now()-start).count();
            std::cout<<lines<<"query\t"<<repeat<<'\t'<<row.first<<'\t'<<std::setprecision(12)<<searchMicros<<'\t'<<n<<'\t'<<total<<'\t'<<exact<<"\tok\t"<<micros<<'\n';
        } catch (const std::exception& error) {
            std::cout<<"query\t"<<repeat<<'\t'<<row.first<<"\t0\t0\t0\t0\terror\n";
            std::cerr<<row.first<<": "<<error.what()<<'\n';
        }
    }
    tantivy_ohos_close(h);
}
static void Memory(const char* path,const char* queries,int readers,const std::string& mode) {
    const auto rows=Read(queries); std::vector<tantivy_ohos_handle*> handles;
    const bool body=mode.find("body")!=std::string::npos,count=mode.find("count")!=std::string::npos;
    std::cout<<"pss_kib\tbefore_open\t0\t"<<Pss()<<'\n';
    for (int i=0;i<readers;++i) {
        handles.push_back(Open(path)); std::cout<<"pss_kib\topened\t"<<i+1<<'\t'<<Pss()<<'\n';
        std::size_t returned=0,bytes=0,errors=0;
        for (const auto& row:rows) {
            try {
                std::unique_ptr<tantivy_ohos_result,decltype(&tantivy_ohos_free_result_v2)> r(Search(handles.back(),row.second,body,count),tantivy_ohos_free_result_v2);
                char* e=nullptr;std::size_t n=0;const int s=tantivy_ohos_result_count_v2(r.get(),&n,&e);Check(s,e);returned+=n;
                if(body)for(std::size_t k=0;k<n;++k){tantivy_ohos_value_view_v2 value={};e=nullptr;const int s2=tantivy_ohos_result_value_v2(r.get(),k,0,&value,&e);Check(s2,e);bytes+=std::strlen(value.string_value);}
            }catch(const std::exception& error){++errors;std::cerr<<row.first<<": "<<error.what()<<'\n';}
        }
        std::cout<<"pss_kib\twarmed\t"<<i+1<<'\t'<<Pss()<<'\n';
        std::cout<<"warmup_coverage\t"<<i+1<<'\t'<<mode<<'\t'<<rows.size()<<'\t'<<returned<<'\t'<<bytes<<'\t'<<errors<<'\n';
    }
    for (auto* h:handles) tantivy_ohos_close(h);
    std::cout<<"pss_kib\tclosed\t0\t"<<Pss()<<'\n';
}
int main(int argc,char** argv) {
    try {
        if (argc>=5 && std::string(argv[1])=="build") Build(argv[2],argv[3],argv[4]);
        else if (argc>=6 && std::string(argv[1])=="query") Query(argv[2],argv[3],argv[4],std::stoi(argv[5]),argc>6?argv[6]:nullptr);
        else if ((argc==5||argc==6) && std::string(argv[1])=="memory") Memory(argv[2],argv[3],std::stoi(argv[4]),argc==6?argv[5]:"id");
        else throw std::runtime_error("build INDEX DOCS fast|stored|custom; query INDEX QUERIES id|body|countid|countbody REPEATS [DOCS]; memory INDEX QUERIES READERS [id|body|countid|countbody]");
        return 0;
    } catch (const std::exception& e) {std::cerr<<e.what()<<'\n';return 1;}
}
