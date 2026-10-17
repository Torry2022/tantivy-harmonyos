# Embedded Jieba data

The binary embeds `jieba-rs` 0.8.1 `src/data/dict.txt` and the HMM tables generated
from `jieba-macros` 0.8.1 `src/hmm.model`. Both crates originate from:
https://github.com/messense/jieba-rs/tree/ebcbda87aca6f3a7c88cc85ef4fd82670adea994

Their upstream MIT license is preserved as `JIEBA-RS-LICENSE` and in the
supplemental license for jieba-macros. The original Jieba project's MIT notice
(Copyright 2013 Sun Junyi) is also preserved in
`supplemental/JIEBA-ORIGINAL-LICENSE`. Original project reference:
https://github.com/fxsjy/jieba/tree/1e20c89b66f56c9301b0feed211733ffaa1bd72a

The bundled dictionary is the version shipped by jieba-rs, not a claim of byte
identity with the original Python Jieba release. `idf.txt` is not embedded by
this adapter's feature configuration. Release manifests record the SHA-256 of
the exact dictionary and HMM input used for the build.
