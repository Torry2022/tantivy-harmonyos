"""Score original-query public SDK results, retaining errors and actual tie order."""
import argparse
import collections
import json
import math
import statistics
import struct
from pathlib import Path

def read(path):
    queries, hits = {}, collections.defaultdict(list)
    for line in path.read_text(encoding='utf-8-sig').splitlines():
        row = line.split('\t')
        assert row[0] in ('hit','query'), row[0]
        assert len(row) in ((8,) if row[0]=='hit' else (8,9)), row
        key = (int(row[1]), row[2])
        assert key[0]>=0 and key[1]
        if row[0] == 'hit':
            hits[key].append({'rank': int(row[3]), 'id': row[4], 'bits': int(row[5]),
                              'bytes': int(row[6]), 'body_hash': int(row[7])})
        elif row[0] == 'query':
            assert key not in queries, ('duplicate query record',key)
            queries[key] = {'api_us': float(row[3]), 'page': int(row[4]), 'total': int(row[5]),
                            'exact': row[6] == '1', 'status': row[7]}
    assert set(hits)<=set(queries), 'hits without query record'
    for key, query in queries.items():
        assert query['status'] in ('ok','error') and 0<=query['page']<=100
        assert len(hits[key]) == query['page']
        validate_hits(hits[key])
        assert descending(hits[key]), ('scores not descending',key)
    return queries, hits

def score(hit):
    assert 0<=hit['bits']<2**32
    value=struct.unpack('<f',struct.pack('<I',hit['bits']))[0]
    assert math.isfinite(value), 'non-finite score'
    return value

def validate_hits(rows):
    assert [h['rank'] for h in rows]==list(range(1,len(rows)+1)), 'invalid rank positions'
    assert len({h['id'] for h in rows})==len(rows), 'duplicate document ID'
    assert all(h['id'] and h['bytes']>=0 and 0<=h['body_hash']<2**64 for h in rows)
    for h in rows:score(h)

def descending(rows):
    return all(score(a)>=score(b) for a,b in zip(rows,rows[1:]))

def quality(queries, hits, qids, qrels):
    assert set(q for r, q in queries if r == 0) == set(qids)
    values = collections.defaultdict(list)
    for qid in qids:
        found = hits[(0, qid)]
        relevant = qrels[qid]
        scores = [relevant.get(h['id'], 0) for h in found]
        dcg = sum((2**g-1)/math.log2(i+2) for i, g in enumerate(scores[:10]))
        ideal = sum((2**g-1)/math.log2(i+2) for i, g in enumerate(sorted(relevant.values(), reverse=True)[:10]))
        values['nDCG@10'].append(dcg / ideal if ideal else 0.0)
        values['MRR@10'].append(next((1/(i+1) for i, g in enumerate(scores[:10]) if g>0), 0.0))
        for k in [10, 100]:
            values[f'Recall@{k}'].append(sum(g>0 for g in scores[:k])/len(relevant))
    return {name: statistics.mean(rows) for name, rows in values.items()}

def percentile(values, p):
    values = sorted(values)
    position = (len(values)-1)*p
    lo, hi = math.floor(position), math.ceil(position)
    return values[lo] + (values[hi]-values[lo])*(position-lo)

def compare(left, right):
    aq, ah = left; bq, bh = right
    assert aq.keys() == bq.keys()
    changes = []
    for key in aq:
        assert aq[key]['status'] == bq[key]['status'], key
        a, b = ah[key], bh[key]
        validate_hits(a);validate_hits(b)
        amap = {h['id']: h for h in a}; bmap = {h['id']: h for h in b}
        mismatch = [i for i in amap.keys() & bmap.keys() if amap[i]['bits'] != bmap[i]['bits']]
        order_error=not descending(a) or not descending(b)
        if a != b or order_error:
            # A top-K boundary tie may have different members; retain the evidence.
            score_sequence_equal=[h['bits'] for h in a]==[h['bits'] for h in b]
            boundary = a[-1]['bits'] if score_sequence_equal and len(a)==100 and len(b)==100 else None
            def groups(rows):
                result = collections.defaultdict(set)
                for h in rows:
                    if h['bits'] != boundary: result[h['bits']].add(h['id'])
                return dict(result)
            non_tie = order_error or not score_sequence_equal or bool(mismatch) or groups(a) != groups(b)
            body_mismatch = [i for i in amap.keys() & bmap.keys()
                             if (amap[i]['bytes'],amap[i]['body_hash']) != (bmap[i]['bytes'],bmap[i]['body_hash'])]
            changes.append({'repeat':key[0], 'qid':key[1], 'non_tie_difference':non_tie,
                            'score_bit_mismatch':mismatch, 'body_mismatch':body_mismatch,
                            'ordered_score_sequence_equal':score_sequence_equal,'score_order_error':order_error,
                            'boundary_tie_bits': boundary,
                            'left':[(h['id'],h['bits']) for h in a],
                            'right':[(h['id'],h['bits']) for h in b]})
    return {'changed_queries':len(changes), 'non_tie_differences':sum(c['non_tie_difference'] for c in changes),
            'body_mismatches':sum(len(c['body_mismatch']) for c in changes), 'changes':changes}

def main():
    p=argparse.ArgumentParser(description=__doc__)
    p.add_argument('inputs',type=Path); p.add_argument('baseline',type=Path); p.add_argument('candidate',type=Path);p.add_argument('output',type=Path)
    a=p.parse_args()
    qids=[line.split('\t')[0] for line in (a.inputs/'queries.original.tsv').read_text(encoding='utf-8').splitlines()]
    qrels=collections.defaultdict(dict)
    for line in (a.inputs/'qrels.tsv').read_text(encoding='utf-8').splitlines():
        qid,doc,grade=line.split('\t');qrels[qid][doc]=int(grade)
    left, right=read(a.baseline),read(a.candidate)
    report={'query_count':len(qids), 'semantics':'Public QueryParser, unchanged original queries, implicit AND; parser errors contribute zero quality.',
            'metrics':{}, 'errors':{}, 'comparison':compare(left,right),
            'performance_scope':'Host diagnostic only. Library call timer excludes request construction and result formatting; no device PSS or Pura performance inferred.'}
    for name,(queries,hits) in [('baseline',left),('candidate',right)]:
        report['metrics'][name]=quality(queries,hits,qids,qrels)
        report['errors'][name]=[q for (r,q),v in queries.items() if r==0 and v['status']!='ok']
    a.output.write_text(json.dumps(report,ensure_ascii=False,indent=2)+'\n',encoding='utf-8')
    print(json.dumps({k:v for k,v in report.items() if k!='comparison'},ensure_ascii=False,indent=2))
    print(json.dumps({k:v for k,v in report['comparison'].items() if k!='changes'}))
    assert report['comparison']['non_tie_differences']==0
    assert report['comparison']['body_mismatches']==0

if __name__=='__main__': main()
