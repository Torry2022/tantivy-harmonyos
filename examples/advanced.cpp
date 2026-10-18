/* C++11. Run with a NEW index path. Fields and dictionary are persisted. */
#include "tantivy_ohos.h"
#include <iostream>
#include <memory>
#include <stdexcept>
using Handle=std::unique_ptr<tantivy_ohos_handle,decltype(&tantivy_ohos_close)>;
using Result=std::unique_ptr<tantivy_ohos_result,decltype(&tantivy_ohos_free_result_v2)>;
using String=std::unique_ptr<char,decltype(&tantivy_ohos_free_string)>;
static void check(int code,char*& error){String message(error,tantivy_ohos_free_string);error=nullptr;if(code)throw std::runtime_error(message?message.get():"operation failed");}
int main(int argc,char** argv){
    if(argc!=2){std::cerr<<"usage: advanced <new-index-path>\n";return 2;}
    try{
        char* error=nullptr;tantivy_ohos_handle* raw=nullptr;
        const char* config=R"({"version":3,"tokenizer":"zh_jieba_v3","fields":[{"name":"title","kind":1,"flags":15},{"name":"category","kind":2,"flags":3},{"name":"price","kind":4,"flags":3}],"fast_fields":["category","price"],"dictionary":{"mode":"extend_default","entries":[{"word":"鸿蒙星河检索","frequency":1000000000}]}})";
        check(tantivy_ohos_create_v3(argv[1],config,&raw,&error),error);Handle writer(raw,tantivy_ohos_close);
        const char* ids[]={"a","b","c"};const double prices[]={10.0,20.0,-5.0};
        for(size_t i=0;i<3;i++){
            const tantivy_ohos_field_value_v2 values[]={
                {"title",TANTIVY_OHOS_TEXT,"鸿蒙星河检索",0,0},
                {"category",TANTIVY_OHOS_KEYWORD,i==2?"note":"book",0,0},
                {"price",TANTIVY_OHOS_F64,nullptr,0,prices[i]}};
            check(tantivy_ohos_upsert_v2(writer.get(),ids[i],values,3,&error),error);
        }
        check(tantivy_ohos_commit(writer.get(),&error),error);
        tantivy_ohos_result* raw_result=nullptr;
        check(tantivy_ohos_search_v3(writer.get(),R"({"version":3,"query":"*","limit":2,"projection":"none","sort":{"field":"price","order":"desc"},"track_total":true})",&raw_result,&error),error);
        Result result(raw_result,tantivy_ohos_free_result_v2);
        char* raw_json=nullptr;
        check(tantivy_ohos_aggregate_v3(writer.get(),R"({"version":3,"query":"*","aggregations":{"categories":{"terms":{"field":"category"}},"prices":{"stats":{"field":"price"}},"bands":{"range":{"field":"price","ranges":[{"from":-10,"to":0},{"from":10,"to":30}]}}}})",&raw_json,&error),error);
        String json(raw_json,tantivy_ohos_free_string);std::cout<<json.get()<<'\n';
        writer.reset(); // the result owns its hit/field/sort strings
        size_t count=0;check(tantivy_ohos_result_count_v2(result.get(),&count,&error),error);
        uint64_t total=0;bool exact=false;check(tantivy_ohos_result_total_v4(result.get(),&total,&exact,&error),error);
        if(count!=2||total!=3||!exact)throw std::runtime_error("unexpected page or total");
        std::cout<<"page="<<count<<" total="<<total<<" exact="<<exact<<'\n';
        for(size_t i=0;i<count;i++){
            tantivy_ohos_hit_view_v2 hit={};tantivy_ohos_value_view_v2 sort={};bool present=false;
            check(tantivy_ohos_result_hit_v2(result.get(),i,&hit,&error),error);
            check(tantivy_ohos_result_sort_value_v3(result.get(),i,&sort,&present,&error),error);
            if(present)std::cout<<hit.id<<" price="<<sort.f64_value<<" (BM25 not computed)\n";
        }
    }catch(const std::exception& e){std::cerr<<e.what()<<'\n';return 1;}
    return 0;
}
