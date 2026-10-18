/* Extend the field/dictionary checks with paging, totals and projections. */
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
    const tantivy_ohos_open_options o={sizeof(o),TANTIVY_OHOS_READ_ONLY};
    if(AdvancedSmokeMain(argc,argv))return 1;
    snprintf(path,sizeof(path),"%s/default",argv[1]);
    CALL(tantivy_ohos_open(path,&o,NULL,&h,&error));
    CALL(tantivy_ohos_config_json(h,&json,&error));CHECK(strstr(json,"\"fast_id\":true"));
    tantivy_ohos_free_string(json);json=NULL;
    CALL(tantivy_ohos_search(h,"{\"query\":\"*\",\"limit\":3,\"projection\":\"none\",\"sort\":{\"field\":\"n\",\"order\":\"asc\"},\"track_total\":true}",&id,&error));
    CALL(tantivy_ohos_result_count(id,&count,&error));CHECK(count==3);
    CALL(tantivy_ohos_result_total(id,&total,&exact,&error));CHECK(total==8&&exact);
    CALL(tantivy_ohos_search(h,"{\"query\":\"*\",\"limit\":3,\"projection\":\"default\",\"sort\":{\"field\":\"n\",\"order\":\"asc\"}}",&body,&error));
    for(size_t i=0;i<3;i++){
        tantivy_ohos_hit_view a={NULL,0,0},b={NULL,0,0};
        CALL(tantivy_ohos_result_hit(id,i,&a,&error));CALL(tantivy_ohos_result_hit(body,i,&b,&error));
        CHECK(!strcmp(a.id,b.id)&&a.score==b.score&&a.field_count==0&&b.field_count==1);
    }
    CALL(tantivy_ohos_result_total(body,&total,&exact,&error));CHECK(total==0&&!exact);
    CALL(tantivy_ohos_search(h,"{\"query\":\"*\",\"limit\":3,\"offset\":9,\"projection\":\"none\",\"track_total\":true}",&last,&error));
    CALL(tantivy_ohos_result_count(last,&count,&error));CHECK(count==0);
    CALL(tantivy_ohos_result_total(last,&total,&exact,&error));CHECK(total==8&&exact);
    CALL(tantivy_ohos_search(h,"{\"query\":\"*\",\"limit\":3,\"projection\":\"none\"}",&unknown,&error));
    CALL(tantivy_ohos_result_total(unknown,&total,&exact,&error));CHECK(total==0&&!exact);
    tantivy_ohos_close(h);h=NULL;
    CALL(tantivy_ohos_result_total(id,&total,&exact,&error));CHECK(total==8&&exact);
    tantivy_ohos_free_result(id);tantivy_ohos_free_result(body);tantivy_ohos_free_result(last);tantivy_ohos_free_result(unknown);
    printf("PASS Tantivy API %s: page=3 total=8; beyond-end page=0 total=8; count optional; fast ID and body agree\n",argv[2]);return 0;
fail:
    tantivy_ohos_free_string(error);tantivy_ohos_free_string(json);tantivy_ohos_close(h);
    tantivy_ohos_free_result(id);tantivy_ohos_free_result(body);tantivy_ohos_free_result(last);tantivy_ohos_free_result(unknown);return 1;
}
