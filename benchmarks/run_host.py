"""Serial host-only repeats on already-built frozen indexes; never operates a device."""
import argparse
import hashlib
import json
import platform
import subprocess
import time
from pathlib import Path
from analyze import percentile, read

def main():
    p=argparse.ArgumentParser(description=__doc__)
    p.add_argument('work',type=Path)
    p.add_argument('--output',type=Path)
    a=p.parse_args(); w=a.work.resolve()
    output=a.output.resolve() if a.output else w/'b-host-repeats'
    output.mkdir()  # A rerun needs a distinct output folder, never overwrites evidence.
    cases=[('baseline','id'),('baseline','body'),('fast','id'),('fast','body'),('fast','countid'),('fast','countbody'),('stored','id'),('custom','id')]
    report={'platform':platform.platform(),'scope':'Host diagnostic, not Pura/device performance. Original queries, Top100, all cases warm all queries before timing. No cold cache claim.',
            'api_timing':'Inside Search C ABI invocation through return; excludes request construction, result iteration/formatting, output and explicit free. Includes SDK parsing, collectors and materialization.',
            'processes':[], 'indexes':{}, 'pss':'unavailable on Windows; target benchmark emits /proc/self/smaps_rollup Pss in KiB'}
    for name in ['baseline','fast','stored','custom']:
        index=w/f'b-index-{name}'
        files={str(f.relative_to(index)):{'bytes':f.stat().st_size,'sha256':hashlib.sha256(f.read_bytes()).hexdigest(),'mtime_ns':f.stat().st_mtime_ns} for f in sorted(index.iterdir()) if f.is_file()}
        report['indexes'][name]={'bytes':sum(f['bytes'] for f in files.values()),'files':files}
    for run in range(3):
        for kind,mode in (cases if run%2==0 else list(reversed(cases))):
            exe=w/('b-baseline' if kind=='baseline' else 'v4-host')/'sdk_bench.exe'
            args=[str(exe),'query',str(w/f'b-index-{kind}'),str(w/'b-inputs/queries.original.tsv'),mode,'3']
            target=output/f'{kind}-{mode}-{run}.tsv'
            started=time.time()
            with target.open('w',encoding='utf-8') as stdout, target.with_suffix('.errors.log').open('w',encoding='utf-8') as stderr:
                child=subprocess.Popen(args,stdout=stdout,stderr=stderr)
                code=child.wait()
            assert code==0,(kind,mode,code)
            queries,hits=read(target)
            samples=[q['api_us'] for q in queries.values() if q['status']=='ok']
            assert len(queries)==1500
            row={'kind':kind,'projection':mode,'run':run,'pid':child.pid,'started_unix':started,'args':args,
                 'executable_sha256':hashlib.sha256(exe.read_bytes()).hexdigest(),'output_sha256':hashlib.sha256(target.read_bytes()).hexdigest(),
                 'library_sha256':hashlib.sha256((exe.parent/'tantivy_ohos.dll').read_bytes()).hexdigest(),
                 'successful_samples':len(samples),'errors':len(queries)-len(samples),'p50_api_us':percentile(samples,.5),'p95_api_us':percentile(samples,.95),
                 'returned_hits':sum(q['page'] for q in queries.values()),'returned_body_bytes':sum(h['bytes'] for rows in hits.values() for h in rows),
                 'exact_count':mode.startswith('count')}
            report['processes'].append(row)
            print(f"{kind} {mode} process {run}: P50={row['p50_api_us']:.2f}us P95={row['p95_api_us']:.2f}us",flush=True)
    for kind,before in report['indexes'].items():
        index=w/f'b-index-{kind}'
        after={str(f.relative_to(index)):{'bytes':f.stat().st_size,'sha256':hashlib.sha256(f.read_bytes()).hexdigest(),'mtime_ns':f.stat().st_mtime_ns} for f in sorted(index.iterdir()) if f.is_file()}
        assert before['files']==after,kind
    report['readonly_all_index_files_bytes_and_mtime_unchanged']=True
    (output/'HOST-REPORT.json').write_text(json.dumps(report,ensure_ascii=False,indent=2)+'\n',encoding='utf-8')

if __name__=='__main__': main()
