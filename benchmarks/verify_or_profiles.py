"""Reanalyze stored OR profiles; no engine/device runs."""
import argparse
import json
from pathlib import Path
from analyze import compare, read
from analyze_or import without_body

def main():
    p=argparse.ArgumentParser(description=__doc__)
    p.add_argument('quality',type=Path);p.add_argument('profiles',type=Path);p.add_argument('output',type=Path)
    a=p.parse_args()
    default=read(a.quality/'fast-body.tsv');counted=read(a.quality/'fast-countbody.tsv')
    custom=read(a.profiles/'custom-id-0.tsv')
    profile=json.loads((a.profiles/'OR-HOST-REPORT.json').read_text(encoding='utf-8'))
    report={'scope':'Compare default-dictionary cases with verified full-text quality run; custom dictionary is separately checked for repeat consistency, not claimed to preserve default semantics.',
            'index_total_bytes':{kind:sum(f['bytes'] for f in files.values()) for kind,files in profile['indices'].items()},'checks':[]}
    for path in sorted(a.profiles.glob('*.tsv')):
        q,h=read(path);mode=path.stem.split('-')[1]
        refq,refh=custom if path.name.startswith('custom-') else default
        expectedq={key:refq[(0,key[1])] for key in q}
        expectedh={key:refh[(0,key[1])] for key in q}
        left=(expectedq,expectedh);right=(q,h)
        if 'body' not in mode:left=without_body(left);right=without_body(right)
        comparison=compare(left,right)
        summary={key:value for key,value in comparison.items() if key!='changes'}
        assert not comparison['non_tie_differences'] and not comparison['body_mismatches'],(path.name,summary)
        if mode.startswith('count'):
            assert all(value['exact'] and value['total']==counted[0][(0,key[1])]['total'] for key,value in q.items())
        else:assert all(not value['exact'] and value['total']==0 for value in q.values())
        report['checks'].append({'file':path.name,'queries':len(q),**summary})
    assert len(report['checks'])==24
    a.output.write_text(json.dumps(report,ensure_ascii=False,indent=2)+'\n',encoding='utf-8')
    print('24 stored process outputs: ordered ranks/score bits/body/count-state verification PASS')

if __name__=='__main__':main()
