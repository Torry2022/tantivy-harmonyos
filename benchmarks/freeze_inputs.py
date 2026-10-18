"""Read-only copy of the approved original Chinese subset; no corpus is committed."""
import argparse
import hashlib
import json
from pathlib import Path

def digest(data):
    return hashlib.sha256(data).hexdigest()

def main():
    p = argparse.ArgumentParser(description=__doc__)
    p.add_argument('source', type=Path)
    p.add_argument('destination', type=Path)
    a = p.parse_args()
    if a.destination.exists():
        raise SystemExit('Destination already exists; preserve the frozen input')
    prior = json.loads((a.source / 'manifest.json').read_text(encoding='utf-8'))
    names = ['documents.original.tsv', 'queries.original.tsv', 'qrels.tsv']
    payloads = {name: (a.source / name).read_bytes() for name in names}
    for name, data in payloads.items():
        assert digest(data) == prior['files'][name], name
    selection = prior['selection']
    manifest = {
        'source_directory': str(a.source.resolve()),
        'dataset': selection['dataset'], 'revision': selection['revision'],
        'source_url': selection['source_url'],
        'license_as_declared_by_dataset_card': selection['license_as_declared_by_dataset_card'],
        'license_verification': 'Declaration inherited from frozen selection provenance; dataset card retained by source project. Not an independent rights clearance.',
        'selection': selection['selection'], 'preprocessing': selection['preprocessing'],
        'files': {name: {'sha256': digest(data), 'bytes': len(data), 'rows': len(data.splitlines())}
                  for name, data in payloads.items()},
        'query_semantics': 'Original query strings passed unchanged to public SDK QueryParser: default Jieba/HMM/lowercase and implicit AND; parser errors reported, not rewritten. No whitespace tokens or diagnostic OR queries.',
        'provenance_manifest_sha256': digest((a.source / 'manifest.json').read_bytes()),
        'not_comparable_to_prior_diagnostic': True,
    }
    assert [manifest['files'][n]['rows'] for n in names] == [20000, 500, 2658]
    a.destination.mkdir(parents=True)
    for name, data in payloads.items():
        (a.destination / name).write_bytes(data)
    (a.destination / 'INPUT-MANIFEST.json').write_text(json.dumps(manifest, ensure_ascii=False, indent=2)+'\n', encoding='utf-8')
    print(json.dumps(manifest, ensure_ascii=False, indent=2))

if __name__ == '__main__':
    main()
