"""Separate OR workload quality/coverage report; does not relabel the AND run."""
import argparse
import collections
import copy
import json
import statistics
from pathlib import Path
from analyze import compare, percentile, quality, read

def distribution(values):
    assert values
    return {'n':len(values),'min':min(values),'mean':statistics.mean(values),'p50':percentile(values,.5),
            'p90':percentile(values,.9),'p95':percentile(values,.95),'p99':percentile(values,.99),'max':max(values)}

def coverage(result):
    queries,hits=result
    rows=[(key,q) for key,q in queries.items() if key[0]==0]
    counts=[q['page'] for _,q in rows]
    bytecounts=[sum(h['bytes'] for h in hits[key]) for key,_ in rows]
    return {'queries':len(rows),'errors':[key[1] for key,q in rows if q['status']!='ok'],
            'zero_hit_queries':sum(n==0 for n in counts),'zero_hit_rate':sum(n==0 for n in counts)/len(rows),
            'total_returned_hits':sum(counts),'returned_hits_per_query':distribution(counts),
            'total_returned_body_bytes':sum(bytecounts),'returned_body_bytes_per_query':distribution(bytecounts)}

def without_body(result):
    q,h=copy.deepcopy(result)
    for rows in h.values():
        for hit in rows:hit['bytes']=0;hit['body_hash']=0
    return q,h

def require_compatible(left,right,body=True):
    r=compare(left,right) if body else compare(without_body(left),without_body(right))
    assert r['non_tie_differences']==0,{k:v for k,v in r.items() if k!='changes'}
    assert r['body_mismatches']==0,{k:v for k,v in r.items() if k!='changes'}
    return r

def main():
    p=argparse.ArgumentParser(description=__doc__)
    p.add_argument('inputs',type=Path);p.add_argument('results',type=Path);p.add_argument('output',type=Path)
    a=p.parse_args()
    names=['baseline-body','fast-body','fast-countbody','fast-id','fast-countid']
    runs={name:read(a.results/(name+'.tsv')) for name in names}
    qids=[l.split('\t')[0] for l in (a.inputs/'queries.original.tsv').read_text(encoding='utf-8').splitlines()]
    qrels=collections.defaultdict(dict)
    for line in (a.inputs/'qrels.tsv').read_text(encoding='utf-8').splitlines():
        qid,doc,grade=line.split('\t');qrels[qid][doc]=int(grade)
    report={'workload':'Explicit OR of frozen lowercase Jieba literal terms; public SDK native analyzer re-analyzes literals. Corpus remains original Chinese text.',
            'metrics':{name:quality(*result,qids,qrels) for name,result in runs.items()},
            'coverage':{name:coverage(result) for name,result in runs.items()},
            'comparisons':{
                'baseline_vs_candidate_body':require_compatible(runs['baseline-body'],runs['fast-body']),
                'count_switch_body':require_compatible(runs['fast-body'],runs['fast-countbody']),
                'id_vs_body_rank_score':require_compatible(runs['fast-body'],runs['fast-id'],False),
                'count_switch_id':require_compatible(runs['fast-id'],runs['fast-countid'])},
            'metric_scope':'500-query subset, original qrels, unjudged=nonrelevant, nDCG/MRR/Recall definitions same as AND report. Separate query semantics, not directly comparable with old whitespace/OR diagnostic results.'}
    q=runs['fast-countbody'][0]
    assert all(v['exact'] and v['total']>=v['page'] for v in q.values())
    assert all(not v['exact'] for name,(queries,_) in runs.items() if not name.startswith('fast-count') for v in queries.values())
    assert {k:v['total'] for k,v in q.items()}=={k:v['total'] for k,v in runs['fast-countid'][0].items()}
    report['exact_total_per_query']=distribution([v['total'] for v in q.values()])
    assert all(not value['errors'] for value in report['coverage'].values())
    a.output.write_text(json.dumps(report,ensure_ascii=False,indent=2)+'\n',encoding='utf-8')
    print(json.dumps({k:v for k,v in report.items() if k!='comparisons'},ensure_ascii=False,indent=2))
    print(json.dumps({name:{k:v for k,v in r.items() if k!='changes'} for name,r in report['comparisons'].items()},ensure_ascii=False))

if __name__=='__main__':main()
