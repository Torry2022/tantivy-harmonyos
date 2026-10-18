#include "napi/native_api.h"
#include <cstdio>
#include <filesystem>
#include <fstream>
#include <string>
#include <vector>
#include <unistd.h>
extern int TantivyBenchmarkMain(int argc,char** argv);
static std::string Read(napi_env env,napi_value value){
    size_t size=0;napi_get_value_string_utf8(env,value,nullptr,0,&size);
    std::vector<char> data(size+1);napi_get_value_string_utf8(env,value,data.data(),data.size(),&size);
    return std::string(data.data(),size);
}
static napi_value Run(napi_env env,napi_callback_info info){
    size_t argc=2;napi_value args[2];napi_get_cb_info(env,info,&argc,args,nullptr,nullptr);
    if(argc!=2){napi_throw_error(env,nullptr,"root and phase required");return nullptr;}
    const std::string root=Read(env,args[0]),phase=Read(env,args[1]);
    std::vector<std::string> parts;size_t start=0;
    for(size_t at=0;at<=phase.size();++at)if(at==phase.size()||phase[at]=='_'){parts.push_back(phase.substr(start,at-start));start=at+1;}
    if(parts.size()<2||(parts[1]!="fast"&&parts[1]!="stored"&&parts[1]!="custom")){
        napi_throw_error(env,nullptr,"invalid benchmark phase");return nullptr;
    }
    std::error_code ec;std::filesystem::create_directories(root,ec);
    if(ec){napi_throw_error(env,nullptr,"cannot create benchmark root");return nullptr;}
    std::vector<std::string> words={"sdk_bench"};
    const std::string index=root+"/bench-"+parts[1];
    const std::string queries=root+((parts[0]=="orquery"||parts[0]=="ormemory")?"/queries.explicit-or.tsv":"/queries.original.tsv");
    if(parts[0]=="prepare"&&parts.size()==2){}
    else if(parts[0]=="build"&&parts.size()==2)words.insert(words.end(),{"build",index,root+"/documents.original.tsv",parts[1]});
    else if((parts[0]=="query"||parts[0]=="orquery")&&parts.size()==4&&(parts[2]=="id"||parts[2]=="body"||parts[2]=="countid"||parts[2]=="countbody")&&(parts[3]=="1"||parts[3]=="2"||parts[3]=="3"))
        words.insert(words.end(),{"query",index,queries,parts[2],"3"});
    else if(parts[0]=="memory"&&parts.size()==4&&(parts[2]=="1"||parts[2]=="3")&&(parts[3]=="1"||parts[3]=="2"||parts[3]=="3"))
        words.insert(words.end(),{"memory",index,root+"/queries.original.tsv",parts[2]});
    else if(parts[0]=="ormemory"&&parts.size()==5&&(parts[2]=="1"||parts[2]=="3")&&(parts[3]=="id"||parts[3]=="body"||parts[3]=="countid"||parts[3]=="countbody")&&(parts[4]=="1"||parts[4]=="2"||parts[4]=="3"))
        words.insert(words.end(),{"memory",index,queries,parts[2],parts[3]});
    else{napi_throw_error(env,nullptr,"phase: build_KIND / query_KIND_MODE_RUN / memory_KIND_READERS_RUN");return nullptr;}
    std::freopen((root+"/details-api-"+phase+".log").c_str(),"w",stdout);
    std::freopen((root+"/errors-api-"+phase+".log").c_str(),"w",stderr);
    std::vector<char*> pointers;for(auto& word:words)pointers.push_back(&word[0]);
    const int code=parts[0]=="prepare"?0:TantivyBenchmarkMain(static_cast<int>(pointers.size()),pointers.data());
    std::fflush(stdout);std::fflush(stderr);
    const std::string result="SDK benchmark "+phase+": "+(code==0?"PASS":"FAIL")+" pid="+std::to_string(getpid())+"\n";
    std::ofstream(root+"/report-api-"+phase+".txt")<<result;
    napi_value output;napi_create_string_utf8(env,result.c_str(),result.size(),&output);return output;
}
static napi_value Init(napi_env env,napi_value exports){
    napi_property_descriptor property={"run",nullptr,Run,nullptr,nullptr,nullptr,napi_default,nullptr};napi_define_properties(env,exports,1,&property);return exports;
}
static napi_module module={1,0,nullptr,Init,"tantivy_probe",nullptr,{0}};
extern "C" __attribute__((constructor)) void Register(){napi_module_register(&module);}
