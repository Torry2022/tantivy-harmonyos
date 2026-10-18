"""Independent injections for the benchmark acceptance checker, not SDK tests."""
import argparse
import copy
import json
import struct
import tempfile
import types
import unittest
import zipfile
from pathlib import Path
import analyze

def hit(name, value, rank):
    return {'rank':rank,'id':name,'bits':struct.unpack('<I',struct.pack('<f',value))[0],'bytes':1,'body_hash':1}
def result(rows):
    key=(0,'q')
    return ({key:{'status':'ok','page':len(rows)}},{key:rows})
def accepts(a,b):
    try:
        r=analyze.compare(result(a),result(b))
        return r['non_tie_differences']==0 and r['body_mismatches']==0
    except AssertionError:
        return False

class CheckerInjections(unittest.TestCase):
    def test_non_tie_inversion_rejected(self):
        a=[hit('high',3,1),hit('low',2,2)]
        b=[hit('low',2,1),hit('high',3,2)]
        self.assertFalse(accepts(a,b))
    def test_score_and_body_corruption_rejected(self):
        a=[hit('a',3,1),hit('b',2,2)]
        b=copy.deepcopy(a);b[0]['bits']=hit('x',4,1)['bits'];self.assertFalse(accepts(a,b))
        b=copy.deepcopy(a);b[1]['body_hash']=2;self.assertFalse(accepts(a,b))
        b=copy.deepcopy(a);b[1]['bytes']=2;self.assertFalse(accepts(a,b))
    def test_same_score_exchange_accepted(self):
        self.assertTrue(accepts([hit('a',3,1),hit('b',3,2),hit('c',2,3)],
                                [hit('b',3,1),hit('a',3,2),hit('c',2,3)]))
    def test_boundary_tie_only(self):
        a=[hit(f'h{i}',3,i+1) for i in range(99)]+[hit('boundary-a',2,100)]
        b=copy.deepcopy(a);b[-1]['id']='boundary-b';self.assertTrue(accepts(a,b))
        b=copy.deepcopy(a);b[-1]=hit('wrong-score',1,100);self.assertFalse(accepts(a,b))
        b=copy.deepcopy(a);b[98]['id']='wrong-high-group';self.assertFalse(accepts(a,b))
        # A short result is complete, so its last tie is not an arbitrary Top100 cutoff.
        self.assertFalse(accepts([hit('a',2,1)],[hit('b',2,1)]))
    def test_duplicate_id_and_rank_rejected(self):
        a=[hit('a',3,1),hit('b',2,2)]
        self.assertFalse(accepts(a,[hit('a',3,1),hit('a',2,2)]))
        self.assertFalse(accepts(a,[hit('a',3,1),hit('b',2,1)]))
    def test_read_rejects_malformed_records(self):
        with tempfile.TemporaryDirectory() as directory:
            p=Path(directory)/'in.tsv'
            good='hit\t0\tq\t1\ta\t1077936128\t1\t1\nhit\t0\tq\t2\tb\t1073741824\t1\t1\nquery\t0\tq\t1\t2\t0\t0\tok\n'
            p.write_text(good,encoding='utf-8');analyze.read(p)
            for bad in [good.replace('\t2\tb\t','\t2\ta\t'),good.replace('\t2\tb\t','\t1\tb\t'),
                        good+good.splitlines()[-1]+'\n',good.replace('\t2\t0\t0\tok','\t1\t0\t0\tok')]:
                p.write_text(bad,encoding='utf-8')
                with self.assertRaises(AssertionError):analyze.read(p)

def main():
    p=argparse.ArgumentParser();p.add_argument('--frozen-review',type=Path,required=True);p.add_argument('--evidence',type=Path,required=True);a=p.parse_args()
    old=types.ModuleType('frozen_checker')
    with zipfile.ZipFile(a.frozen_review) as z:exec(z.read('source/benchmarks/analyze.py'),old.__dict__)
    left=[hit('high',3,1),hit('low',2,2)];right=[hit('low',2,1),hit('high',3,2)]
    before=old.compare(result(left),result(right));after=analyze.compare(result(left),result(right))
    assert before['non_tie_differences']==0 and after['non_tie_differences']==1
    suite=unittest.defaultTestLoader.loadTestsFromTestCase(CheckerInjections)
    report=unittest.TextTestRunner(verbosity=2).run(suite)
    evidence={'injected':{'baseline':left,'candidate':right},'frozen_checker_false_pass':before,
              'fixed_checker_rejection':after,'tests_run':report.testsRun,'failures':len(report.failures),'errors':len(report.errors),
              'success':report.wasSuccessful()}
    a.evidence.write_text(json.dumps(evidence,ensure_ascii=False,indent=2)+'\n',encoding='utf-8')
    assert report.wasSuccessful()

if __name__=='__main__':main()
