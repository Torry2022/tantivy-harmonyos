"""Freeze the explicitly ORed workload without changing original input or SDK."""
import argparse
import hashlib
import json
import subprocess
import tomllib
from pathlib import Path

def digest(data): return hashlib.sha256(data).hexdigest()

def main():
    p=argparse.ArgumentParser(description=__doc__)
    p.add_argument('frozen_inputs',type=Path);p.add_argument('same_source_data',type=Path)
    p.add_argument('generator',type=Path);p.add_argument('destination',type=Path)
    a=p.parse_args();root=Path(__file__).resolve().parent.parent
    original=(a.frozen_inputs/'queries.original.tsv').read_bytes()
    prior=json.loads((a.same_source_data/'manifest.json').read_text(encoding='utf-8'))
    assert digest(original)==prior['files']['queries.original.tsv']
    assert (a.same_source_data/'queries.original.tsv').read_bytes()==original
    lock=tomllib.loads((root/'Cargo.lock').read_text())
    assert any(x['name']=='jieba-rs' and x['version']=='0.8.1' for x in lock['package'])
    a.destination.mkdir()  # No overwrite of any frozen derivation.
    result=subprocess.run([str(a.generator.resolve()),str((a.frozen_inputs/'queries.original.tsv').resolve()),
                           str((a.destination/'queries.tokens.tsv').resolve()),str((a.destination/'queries.explicit-or.tsv').resolve())],
                          capture_output=True,text=True,encoding='utf-8')
    if result.returncode:
        raise RuntimeError(result.stderr)
    (a.destination/'native-literal-reanalysis.tsv').write_text(result.stderr,encoding='utf-8')
    audit=json.loads(result.stdout)
    generated=(a.destination/'queries.tokens.tsv').read_bytes()
    reference=(a.same_source_data/'queries.tokens.tsv').read_bytes()
    assert digest(reference)==prior['files']['queries.tokens.tsv']
    assert generated==reference  # Independently regenerate every original query, not trust labels alone.
    queries=(a.destination/'queries.explicit-or.tsv').read_bytes()
    assert audit['rows']==500
    manifest={'workload':'explicit-or-native-jieba-public-sdk', 'source':json.loads((a.frozen_inputs/'INPUT-MANIFEST.json').read_text(encoding='utf-8')),
              'original_queries_sha256':digest(original),'tokens_sha256':digest(generated),'derived_queries_sha256':digest(queries),
              'generator_source_sha256':digest((root/'benchmarks/or_queries.rs').read_bytes()),'generator_executable_sha256':digest(a.generator.read_bytes()),
              'cargo_lock_sha256':digest((root/'Cargo.lock').read_bytes()),'same_source_tokens_verified_byte_for_byte':True,
              'rule':'Jieba 0.8.1 embedded default dictionary, Default/HMM; drop punctuation-only tokens with Rust is_alphanumeric; lowercase; deduplicate preserving first occurrence; double-quote each literal, escape backslash and quote; explicit OR. Every literal and OR AST roundtrips through pinned grammar. Native QueryParser re-analysis of contextual HMM literals can split tokens; all such literals are retained and audited, not replaced by private exact-term queries.',
              'corpus':'UNCHANGED original, not pretokenized; indexed by each public SDK native Jieba analyzer.',
              'default_sdk_semantics_changed':False,'not_a_reuse_of_prior_diagnostic_scores':True,'audit':audit}
    (a.destination/'OR-WORKLOAD-MANIFEST.json').write_text(json.dumps(manifest,ensure_ascii=False,indent=2)+'\n',encoding='utf-8')
    print(json.dumps(manifest,ensure_ascii=False,indent=2))

if __name__=='__main__':main()
