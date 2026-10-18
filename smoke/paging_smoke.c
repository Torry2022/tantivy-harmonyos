/* Reuse all A assertions, then exercise B only through the public C ABI. */
static int PagingMain(int argc,char** argv);
int main(int argc,char** argv){return PagingMain(argc,argv);}
#ifdef main
#undef main
#endif
#define main AdvancedSmokeMain
#include "advanced_smoke.c"
#undef main

static int PagingMain(int argc,char** argv){
    tantivy_ohos_handle* h=NULL;
    tantivy_ohos_result *id=NULL,*body=NULL,*last=NULL,*unknown=NULL;
    char *error=NULL,*json=NULL,path[2048];size_t count=0;uint64_t total=0;bool exact=false;
    const tantivy_ohos_open_options_v2 o={sizeof(o),2,TANTIVY_OHOS_READ_ONLY};
    const tantivy_ohos_search_options_v2 s={sizeof(s),2,TANTIVY_OHOS_PROJECT_NONE,NULL,0,NULL,0};
    if(AdvancedSmokeMain(argc,argv))return 1;
    CHECK(tantivy_ohos_abi_version()>=4);
    snprintf(path,sizeof(path),"%s/default",argv[1]);
    CALL(tantivy_ohos_open_v2(path,&o,&h,&error));
    CALL(tantivy_ohos_config_json_v2(h,&json,&error));CHECK(strstr(json,"\"fast_id\":true"));
    tantivy_ohos_free_string(json);json=NULL;
    CALL(tantivy_ohos_search_v3(h,"{\"version\":3,\"query\":\"*\",\"limit\":3,\"projection\":\"none\",\"sort\":{\"field\":\"n\",\"order\":\"asc\"},\"track_total\":true}",&id,&error));
    CALL(tantivy_ohos_result_count_v2(id,&count,&error));CHECK(count==3);
    CALL(tantivy_ohos_result_total_v4(id,&total,&exact,&error));CHECK(total==8&&exact);
    CALL(tantivy_ohos_search_v3(h,"{\"version\":3,\"query\":\"*\",\"limit\":3,\"projection\":\"default\",\"sort\":{\"field\":\"n\",\"order\":\"asc\"}}",&body,&error));
    for(size_t i=0;i<3;i++){
        tantivy_ohos_hit_view_v2 a={NULL,0,0},b={NULL,0,0};
        CALL(tantivy_ohos_result_hit_v2(id,i,&a,&error));CALL(tantivy_ohos_result_hit_v2(body,i,&b,&error));
        CHECK(!strcmp(a.id,b.id)&&a.score==b.score&&a.field_count==0&&b.field_count==1);
    }
    CALL(tantivy_ohos_result_total_v4(body,&total,&exact,&error));CHECK(total==0&&!exact);
    CALL(tantivy_ohos_search_v3(h,"{\"version\":3,\"query\":\"*\",\"limit\":3,\"offset\":9,\"projection\":\"none\",\"track_total\":true}",&last,&error));
    CALL(tantivy_ohos_result_count_v2(last,&count,&error));CHECK(count==0);
    CALL(tantivy_ohos_result_total_v4(last,&total,&exact,&error));CHECK(total==8&&exact);
    CALL(tantivy_ohos_search_v2(h,"*",3,&s,&unknown,&error));
    CALL(tantivy_ohos_result_total_v4(unknown,&total,&exact,&error));CHECK(total==0&&!exact);
    tantivy_ohos_close(h);h=NULL;
    CALL(tantivy_ohos_result_total_v4(id,&total,&exact,&error));CHECK(total==8&&exact);
    tantivy_ohos_free_result_v2(id);tantivy_ohos_free_result_v2(body);tantivy_ohos_free_result_v2(last);tantivy_ohos_free_result_v2(unknown);
    printf("PASS Tantivy API v4 %s: page=3 total=8; beyond-end page=0 total=8; count optional; fast ID and body agree\n",argv[2]);return 0;
fail:
    tantivy_ohos_free_string(error);tantivy_ohos_free_string(json);tantivy_ohos_close(h);
    tantivy_ohos_free_result_v2(id);tantivy_ohos_free_result_v2(body);tantivy_ohos_free_result_v2(last);tantivy_ohos_free_result_v2(unknown);return 1;
}
