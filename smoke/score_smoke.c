/* Actual public C ABI consumer for score overflow; create/reopen/readonly. */
#include "tantivy_ohos.h"
#include <math.h>
#include <stdio.h>
#include <string.h>
#define SCORE_CALL(x) do{if((x)!=0)goto score_fail;}while(0)
#define SCORE_CHECK(x) do{if(!(x))goto score_fail;}while(0)
int main(int argc,char** argv){
    tantivy_ohos_handle* h=NULL;tantivy_ohos_result* result=NULL;
    char *error=NULL,*output=NULL;float base=0;int creating;
    const char* bad[]={"alpha^10000000000000000000000000000000000000000","(alpha^100000000000000000000)^100000000000000000000"};
    const tantivy_ohos_open_options_v2 open_options={sizeof(open_options),2,TANTIVY_OHOS_READ_ONLY};
    const tantivy_ohos_search_options_v2 search_options={sizeof(search_options),2,TANTIVY_OHOS_PROJECT_NONE,NULL,0,NULL,0};
    if(argc!=3)return 2;creating=!strcmp(argv[2],"create");
    if(creating){
        SCORE_CALL(tantivy_ohos_open(argv[1],true,&h,&error));
        SCORE_CALL(tantivy_ohos_upsert(h,"a","alpha beta",&error));SCORE_CALL(tantivy_ohos_commit(h,&error));
    }else SCORE_CALL(tantivy_ohos_open_v2(argv[1],&open_options,&h,&error));
    for(size_t k=0;k<2;k++){
        tantivy_ohos_hit_view_v2 hit={NULL,0,0};
        SCORE_CALL(tantivy_ohos_search_v2(h,k?"alpha^2":"alpha",10,&search_options,&result,&error));
        SCORE_CALL(tantivy_ohos_result_hit_v2(result,0,&hit,&error));
        SCORE_CHECK(isfinite(hit.score)&&hit.score>0&&!strcmp(hit.id,"a"));
        if(k)SCORE_CHECK(hit.score==2*base);else base=hit.score;
        tantivy_ohos_free_result_v2(result);result=NULL;
    }
    for(size_t k=0;k<2;k++){
        char request[512];
        SCORE_CHECK(tantivy_ohos_search_v2(h,bad[k],10,&search_options,&result,&error)==1&&result==NULL&&error&&strstr(error,"non-finite search score"));
        tantivy_ohos_free_string(error);error=NULL;
        snprintf(request,sizeof(request),"{\"version\":3,\"query\":\"%s\",\"limit\":10,\"projection\":\"none\",\"track_total\":true}",bad[k]);
        SCORE_CHECK(tantivy_ohos_search_v3(h,request,&result,&error)==1&&result==NULL&&error&&strstr(error,"non-finite search score"));
        tantivy_ohos_free_string(error);error=NULL;
        SCORE_CHECK(tantivy_ohos_search(h,bad[k],10,&output,&error)==1&&output==NULL&&error&&strstr(error,"non-finite search score"));
        tantivy_ohos_free_string(error);error=NULL;
    }
    SCORE_CALL(tantivy_ohos_search(h,"alpha^2",10,&output,&error));SCORE_CHECK(strstr(output,"\"score\":")&&!strstr(output,"null"));
    tantivy_ohos_free_string(output);tantivy_ohos_close(h);printf("PASS finite score boundary %s: v2/v3/legacy, direct and composed overflow rejected; finite boost preserved\n",argv[2]);return 0;
score_fail:
    fprintf(stderr,"score boundary failure: %s\n",error?error:"assertion");tantivy_ohos_free_string(error);tantivy_ohos_free_string(output);tantivy_ohos_free_result_v2(result);tantivy_ohos_close(h);return 1;
}
