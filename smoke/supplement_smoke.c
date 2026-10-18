static int SupplementMain(int argc,char** argv);
int main(int argc,char** argv){return SupplementMain(argc,argv);}
#ifdef main
#undef main
#endif
#define main PagingAcceptanceMain
#include "paging_smoke.c"
#undef main
#define main ScoreAcceptanceMain
#include "score_smoke.c"
#undef main
static int SupplementMain(int argc,char** argv){
    char path[2048];char* score_args[3];
    if(PagingAcceptanceMain(argc,argv))return 1;
    if(snprintf(path,sizeof(path),"%s/finite-score",argv[1])<0)return 1;
    score_args[0]=argv[0];score_args[1]=path;score_args[2]=argv[2];
    return ScoreAcceptanceMain(3,score_args);
}
