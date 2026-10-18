"""Serial independent processes on the explicit OR workload; preserve AND evidence."""
import argparse
import hashlib
import json
import platform
import subprocess
from pathlib import Path
from analyze import percentile, read
from analyze_or import coverage

def sha(p):return hashlib.sha256(p.read_bytes()).hexdigest()
def snapshot(index):
    return {str(p.relative_to(index)):{'sha256':sha(p),'bytes':p.stat().st_size,'mtime_ns':p.stat().st_mtime_ns}
            for p in sorted(index.rglob('*')) if p.is_file()}

def main():
    p=argparse.ArgumentParser(description=__doc__)
    p.add_argument('work',type=Path);p.add_argument('workload',type=Path);p.add_argument('candidate',type=Path);p.add_argument('baseline',type=Path);p.add_argument('output',type=Path)
    a=p.parse_args();work=a.work.resolve();queries=(a.workload/'queries.explicit-or.tsv').resolve();a.output.mkdir()
    cases=[('baseline','id'),('baseline','body'),('fast','id'),('fast','body'),('fast','countid'),('fast','countbody'),('stored','id'),('custom','id')]
    before={kind:snapshot(work/f'b-index-{kind}') for kind in ['baseline','fast','stored','custom']}
    report={'workload':'explicit OR literals, native Jieba public SDK over original corpus','query_file_sha256':sha(queries),
            'platform':platform.platform(),'processes':[],'indices':before,
            'scope':'Host only; library-call latency excludes caller formatting; successful samples include all queries, with body bytes/count choice explicit. Windows has no PSS. No device claim.'}
    for run in range(3):
        for kind,mode in (cases if run%2==0 else list(reversed(cases))):
            exe=(a.baseline if kind=='baseline' else a.candidate).resolve()
            args=[str(exe),'query',str(work/f'b-index-{kind}'),str(queries),mode,'3']
            target=a.output/f'{kind}-{mode}-{run}.tsv'
            with target.open('w',encoding='utf-8') as stdout,target.with_suffix('.errors.log').open('w',encoding='utf-8') as stderr:
                child=subprocess.Popen(args,stdout=stdout,stderr=stderr);code=child.wait()
            assert code==0,(kind,mode,code)
            result=read(target);q,h=result;assert len(q)==1500
            assert all(value['status']=='ok' for value in q.values())
            assert all(value['exact']==mode.startswith('count') and (not value['exact'] or value['total']>=value['page']) for value in q.values())
            values=[value['api_us'] for value in q.values()]
            row={'kind':kind,'mode':mode,'run':run,'pid':child.pid,'args':args,'executable_sha256':sha(exe),
                 'library_sha256':sha(exe.parent/'tantivy_ohos.dll'),'output_sha256':sha(target),'successful_samples':len(q),'errors':0,
                 'p50_api_us':percentile(values,.5),'p95_api_us':percentile(values,.95),'one_round_coverage':coverage(result),
                 'returned_hits_all_rounds':sum(value['page'] for value in q.values()),
                 'returned_body_bytes_all_rounds':sum(hit['bytes'] for rows in h.values() for hit in rows)}
            report['processes'].append(row)
            print(f"{kind}/{mode} process {run}: hits={row['returned_hits_all_rounds']}, bytes={row['returned_body_bytes_all_rounds']}, P50={row['p50_api_us']:.2f}us P95={row['p95_api_us']:.2f}us",flush=True)
    assert before=={kind:snapshot(work/f'b-index-{kind}') for kind in before}
    report['readonly_all_file_bytes_and_mtime_unchanged']=True
    (a.output/'OR-HOST-REPORT.json').write_text(json.dumps(report,ensure_ascii=False,indent=2)+'\n',encoding='utf-8')

if __name__=='__main__':main()
