/* C99/C++11 public v3 acceptance consumer. Separate create/reopen/readonly processes.
 * Uses only public C API; creates new indexes under its private root. */
#include "tantivy_ohos.h"
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <errno.h>
#ifdef _WIN32
#include <direct.h>
#define MKDIR(p) _mkdir(p)
#else
#include <sys/stat.h>
#define MKDIR(p) mkdir(p,0700)
#endif
#define CHECK(x) do { if (!(x)) { fprintf(stderr,"assertion line %d: %s\n",__LINE__,#x); goto fail; } } while(0)
#define CALL(x) do { if ((x)!=0) { fprintf(stderr,"API line %d: %s\n",__LINE__,error?error:"unknown"); goto fail; } } while(0)
static const char* ids_data[]={"a","b","c","d","e","f","g","h"};
static const int ranks[]={9,-2,0,15,4,10,0,3};
static const char* tags[]={"高","低","高","低","","高",NULL,"中"};
static const char* base_config=
"{\"tokenizer\":\"zh_jieba_v3\",\"fields\":["
"{\"name\":\"title\",\"kind\":1,\"flags\":15},{\"name\":\"n\",\"kind\":3,\"flags\":3},"
"{\"name\":\"x\",\"kind\":4,\"flags\":3},{\"name\":\"tag\",\"kind\":2,\"flags\":3}],"
"\"fast_fields\":[\"n\",\"x\",\"tag\"]%s}";
static int put(tantivy_ohos_handle* h,const char* id,const char* text,int n,int present,const char* tag,char** error){
    tantivy_ohos_field_value values[4]={{"title",1,text,0,0.0},{"n",3,NULL,n,0.0},{"x",4,NULL,0,n/2.0},{"tag",2,tag,0,0.0}};
    return tantivy_ohos_upsert(h,id,values,present?4:1,error);
}
static int query_count(tantivy_ohos_handle* h,const char* query,size_t expected,char** error){
    char request[2048];tantivy_ohos_result* r=NULL;size_t count=0;
    snprintf(request,sizeof(request),"{\"query\":\"%s\",\"limit\":100,\"projection\":\"none\"}",query);
    if(tantivy_ohos_search(h,request,&r,error))return 1;
    if(tantivy_ohos_result_count(r,&count,error)){tantivy_ohos_free_result(r);return 1;}
    tantivy_ohos_free_result(r);printf("query %s -> %zu expected %zu\n",query,count,expected);return count==expected?0:1;
}
static int sorted(tantivy_ohos_handle* h,const char* field,const char* order,const char* expected,char** error){
    char request[1024];size_t total=strlen(expected);
    for(size_t offset=0;offset<total;offset+=3){
        tantivy_ohos_result* r=NULL;size_t count=0;
        snprintf(request,sizeof(request),"{\"query\":\"*\",\"limit\":3,\"offset\":%zu,\"projection\":\"none\",\"sort\":{\"field\":\"%s\",\"order\":\"%s\"}}",offset,field,order);
        if(tantivy_ohos_search(h,request,&r,error))return 1;
        if(tantivy_ohos_result_count(r,&count,error)){tantivy_ohos_free_result(r);return 1;}
        if(count!=(total-offset<3?total-offset:3)){tantivy_ohos_free_result(r);return 1;}
        for(size_t i=0;i<count;i++){
            tantivy_ohos_hit_view hit={NULL,0,0};tantivy_ohos_value_view value={NULL,0,NULL,0,0};bool present=false;
            if(tantivy_ohos_result_hit(r,i,&hit,error)||tantivy_ohos_result_sort_value(r,i,&value,&present,error)){tantivy_ohos_free_result(r);return 1;}
            if(strlen(hit.id)!=1||hit.id[0]!=expected[offset+i]||hit.score!=0||hit.field_count!=0||present!=(hit.id[0]!='g')){tantivy_ohos_free_result(r);return 1;}
            if(present){int n=ranks[(size_t)(hit.id[0]-'a')];if((!strcmp(field,"n")&&(value.kind!=3||value.i64_value!=n))||(!strcmp(field,"x")&&(value.kind!=4||value.f64_value!=n/2.0))){tantivy_ohos_free_result(r);return 1;}}
        }
        tantivy_ohos_free_result(r);
    }
    printf("native sorted %s %s across pages: %s PASS\n",field,order,expected);return 0;
}
static int keyword_sort(tantivy_ohos_handle* h,char** error){
    tantivy_ohos_result* r=NULL;size_t count=0;char previous[64]="";
    if(tantivy_ohos_search(h,"{\"query\":\"*\",\"limit\":100,\"projection\":\"none\",\"sort\":{\"field\":\"tag\",\"order\":\"asc\"}}",&r,error))return 1;
    if(tantivy_ohos_result_count(r,&count,error)){tantivy_ohos_free_result(r);return 1;}
    for(size_t i=0;i<count;i++){
        tantivy_ohos_value_view v={NULL,0,NULL,0,0};bool present=false;
        if(tantivy_ohos_result_sort_value(r,i,&v,&present,error)){tantivy_ohos_free_result(r);return 1;}
        if(!present){if(i+1!=count){tantivy_ohos_free_result(r);return 1;}continue;}
        if(v.kind!=2||!v.string_value||strcmp(previous,v.string_value)>0||strlen(v.string_value)>=sizeof(previous)){tantivy_ohos_free_result(r);return 1;}
        strcpy(previous,v.string_value);
    }
    tantivy_ohos_free_result(r);puts("keyword raw UTF-8 ordering and missing-last PASS");return 0;
}
int main(int argc,char** argv){
    tantivy_ohos_handle* h[3]={NULL,NULL,NULL};tantivy_ohos_result* retained=NULL;char *error=NULL,*json=NULL;
    char path[3][2048];char config[4096];int creating,readonly;
    const char* suffixes[]={"default","custom","other"};
    const char* dictionaries[]={"",",\"dictionary\":{\"mode\":\"extend_default\",\"entries\":[{\"word\":\"鸿蒙星河检索\",\"frequency\":1000000000}]}",",\"dictionary\":{\"mode\":\"extend_default\",\"entries\":[{\"word\":\"端侧向量检索\",\"frequency\":1000000000}]}"};
    const char* aggregation="{\"query\":\"*\",\"aggregations\":{\"groups\":{\"terms\":{\"field\":\"tag\",\"size\":10,\"segment_size\":100}},\"stats\":{\"stats\":{\"field\":\"n\"}},\"gaps\":{\"range\":{\"field\":\"n\",\"ranges\":[{\"from\":-2,\"to\":0,\"key\":\"low\"},{\"from\":10,\"to\":16,\"key\":\"high\"}]}}}}";
    if(argc!=3)return 2;
    creating=!strcmp(argv[2],"create");readonly=!strcmp(argv[2],"readonly");
    if(!creating&&!readonly&&strcmp(argv[2],"reopen"))return 2;
    if(creating)CHECK(MKDIR(argv[1])==0||errno==EEXIST);
    for(size_t k=0;k<3;k++){
        CHECK(snprintf(path[k],sizeof(path[k]),"%s/%s",argv[1],suffixes[k])>0);
        if(creating){
            snprintf(config,sizeof(config),base_config,dictionaries[k]);CALL(tantivy_ohos_create(path[k],config,NULL,&h[k],&error));memset(config,0,sizeof(config));
            for(size_t i=0;i<8;i++){CALL(put(h[k],ids_data[i],i==1?"鸿蒙 星河 检索":"鸿蒙星河检索",ranks[i],i!=6,tags[i],&error));if(i==2)CALL(tantivy_ohos_commit(h[k],&error));}
            CALL(tantivy_ohos_commit(h[k],&error));
        }else{const tantivy_ohos_open_options o={sizeof(o),readonly?TANTIVY_OHOS_READ_ONLY:TANTIVY_OHOS_READ_WRITE};CALL(tantivy_ohos_open(path[k],&o,NULL,&h[k],&error));}
    }
    CALL(query_count(h[0],"鸿蒙星河检索",8,&error));CALL(query_count(h[2],"鸿蒙星河检索",8,&error));CALL(query_count(h[1],"鸿蒙星河检索",readonly?5:7,&error));
    CALL(query_count(h[1],"ghostword",0,&error));
    CALL(sorted(h[1],"n","asc",readonly?"bceafdg":"bcheafdg",&error));
    CALL(sorted(h[1],"n","desc",readonly?"dfaecbg":"dfaehcbg",&error));
    CALL(sorted(h[1],"x","asc",readonly?"bceafdg":"bcheafdg",&error));
    CALL(keyword_sort(h[1],&error));
    CALL(tantivy_ohos_aggregate(h[1],aggregation,&json,&error));printf("AGGREGATE %s\n",json);
    CHECK(strstr(json,readonly?"\"count\":6":"\"count\":7"));CHECK(strstr(json,readonly?"\"sum\":36.0":"\"sum\":39.0"));CHECK(strstr(json,"\"key\":\"low\"")&&strstr(json,"\"key\":\"high\""));tantivy_ohos_free_string(json);json=NULL;
    CHECK(tantivy_ohos_aggregate(h[1],"{\"query\":\"*\",\"memory_limit_bytes\":1,\"aggregations\":{\"t\":{\"terms\":{\"field\":\"tag\"}}}}",&json,&error)==1&&json==NULL&&error);
    tantivy_ohos_free_string(error);error=NULL;
    CHECK(tantivy_ohos_aggregate(h[1],"{\"query\":\"*\",\"aggregations\":{\"r\":{\"range\":{\"field\":\"n\",\"ranges\":[{\"from\":9007199254740993}]}}}}",&json,&error)==1&&error&&json==NULL);tantivy_ohos_free_string(error);error=NULL;
    CALL(tantivy_ohos_search(h[1],"{\"query\":\"*\",\"limit\":1,\"sort\":{\"field\":\"n\",\"order\":\"asc\"}}",&retained,&error));
    if(!strcmp(argv[2],"reopen")){
        CALL(tantivy_ohos_delete(h[1],"h",&error));CALL(put(h[1],"a","更新文档",9,1,"高",&error));CALL(tantivy_ohos_commit(h[1],&error));CALL(put(h[1],"ghost","ghostword",123,1,"高",&error));
    }
    if(readonly){CHECK(tantivy_ohos_commit(h[1],&error)==1&&error);tantivy_ohos_free_string(error);error=NULL;CALL(tantivy_ohos_reload(h[1],&error));}
    for(size_t k=0;k<3;k++){tantivy_ohos_close(h[k]);h[k]=NULL;}
    {tantivy_ohos_value_view v={NULL,0,NULL,0,0};bool present=false;CALL(tantivy_ohos_result_sort_value(retained,0,&v,&present,&error));CHECK(present&&v.kind==3&&v.i64_value==-2);}
    tantivy_ohos_free_result(retained);printf("PASS Tantivy API %s\n",argv[2]);return 0;
fail:
    tantivy_ohos_free_string(json);tantivy_ohos_free_string(error);tantivy_ohos_free_result(retained);for(size_t k=0;k<3;k++)tantivy_ohos_close(h[k]);return 1;
}
