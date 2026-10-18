use super::*;
use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use tantivy::schema::NumericOptions;

pub const TEXT: u32 = 1;
pub const KEYWORD: u32 = 2;
pub const I64: u32 = 3;
pub const F64: u32 = 4;
pub const STORED_FLAG: u32 = 1;
pub const INDEXED: u32 = 2;
pub const QUERY: u32 = 4;
pub const RETURN: u32 = 8;
const CONFIG_FILE: &str = "tantivy-ohos.json";

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FieldConfig {
    pub name: String,
    pub kind: u32,
    pub flags: u32,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Config {
    pub version: u32,
    pub tokenizer: String,
    pub fields: Vec<FieldConfig>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub fast_fields: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub dictionary: Option<Dictionary>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub dictionary_fingerprint: Option<String>,
    // Missing in old metadata means the stored-ID compatibility path, not true.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub fast_id: Option<bool>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Dictionary {
    pub mode: String,
    pub entries: Vec<DictionaryEntry>,
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DictionaryEntry {
    pub word: String,
    #[serde(default = "default_frequency")]
    pub frequency: usize,
}
fn default_frequency() -> usize {
    100_000
}

impl Config {
    pub fn legacy() -> Self {
        Self::new(vec![FieldConfig {
            name: "text".into(),
            kind: TEXT,
            flags: STORED_FLAG | INDEXED | QUERY | RETURN,
        }])
    }

    pub fn new(fields: Vec<FieldConfig>) -> Self {
        Self {
            version: 2,
            tokenizer: "zh_jieba_v1".into(),
            fields,
            fast_fields: Vec::new(),
            dictionary: None,
            dictionary_fingerprint: None,
            fast_id: None,
        }
    }

    pub fn schema(&self) -> Result<Schema> {
        if !((self.version == 2
            && self.tokenizer == "zh_jieba_v1"
            && self.fast_fields.is_empty()
            && self.dictionary.is_none()
            && self.dictionary_fingerprint.is_none())
            || (self.version == 3 && self.tokenizer == "zh_jieba_v3"))
        {
            return Err("unsupported adapter metadata version/tokenizer".into());
        }
        if self.version == 2 && self.fast_id.is_some() {
            return Err("fast_id requires v3 creation".into());
        }
        if self.version == 3
            && (self.dictionary.is_none()
                || self.dictionary_fingerprint.as_deref() != Some(self.fingerprint()?.as_str()))
        {
            return Err("missing or corrupt saved dictionary snapshot/fingerprint".into());
        }
        let mut fast = HashSet::new();
        for name in &self.fast_fields {
            if !fast.insert(name)
                || !self
                    .fields
                    .iter()
                    .any(|f| f.name == *name && matches!(f.kind, KEYWORD | I64 | F64))
            {
                return Err(format!(
                    "fast_fields requires unique KEYWORD/I64/F64 fields: {name}"
                ));
            }
        }
        if let Some(d) = &self.dictionary {
            if d.mode != "extend_default" || d.entries.len() > 10_000 {
                return Err("dictionary requires extend_default and at most 10000 entries".into());
            }
            let mut words = HashSet::new();
            for e in &d.entries {
                if e.word.is_empty()
                    || e.word.len() > 384
                    || e.word.chars().any(|c| c.is_whitespace() || c == '\0')
                    || !words.insert(&e.word)
                    || !(1..=1_000_000_000).contains(&e.frequency)
                {
                    return Err(
                        "invalid/duplicate dictionary word or frequency (1..1000000000)".into(),
                    );
                }
            }
        }
        if self.fields.len() > 64 {
            return Err("at most 64 additional fields are supported".into());
        }
        let mut names = HashSet::new();
        let mut builder = Schema::builder();
        let id_options = STRING | STORED;
        builder.add_text_field(
            "id",
            if self.fast_id == Some(true) {
                id_options.set_fast(None)
            } else {
                id_options
            },
        );
        for f in &self.fields {
            let valid_name = !f.name.is_empty()
                && f.name.len() <= 64
                && f.name.as_bytes()[0].is_ascii_alphabetic()
                && f.name
                    .bytes()
                    .all(|b| b.is_ascii_alphanumeric() || b == b'_');
            if !valid_name || f.name == "id" || !names.insert(&f.name) {
                return Err(format!("invalid/reserved/duplicate field name: {}", f.name));
            }
            if f.flags & !15 != 0
                || f.flags & (STORED_FLAG | INDEXED) == 0
                || (f.flags & RETURN != 0 && f.flags & STORED_FLAG == 0)
                || (f.flags & QUERY != 0
                    && (f.flags & INDEXED == 0 || !matches!(f.kind, TEXT | KEYWORD)))
            {
                return Err(format!("invalid field flags: {}", f.name));
            }
            match f.kind {
                TEXT | KEYWORD => {
                    let mut opts = TextOptions::default();
                    if f.flags & STORED_FLAG != 0 {
                        opts = opts.set_stored();
                    }
                    if fast.contains(&f.name) {
                        opts = opts.set_fast(None);
                    }
                    if f.flags & INDEXED != 0 {
                        opts = opts.set_indexing_options(if f.kind == TEXT {
                            TextFieldIndexing::default()
                                .set_tokenizer(&self.tokenizer)
                                .set_index_option(IndexRecordOption::WithFreqsAndPositions)
                        } else {
                            TextFieldIndexing::default()
                                .set_tokenizer("raw")
                                .set_index_option(IndexRecordOption::Basic)
                        });
                    }
                    builder.add_text_field(&f.name, opts);
                }
                I64 | F64 => {
                    let mut opts = NumericOptions::default();
                    if fast.contains(&f.name) {
                        opts = opts.set_fast();
                    }
                    if f.flags & STORED_FLAG != 0 {
                        opts = opts.set_stored();
                    }
                    if f.flags & INDEXED != 0 {
                        opts = opts.set_indexed();
                    }
                    if f.kind == I64 {
                        builder.add_i64_field(&f.name, opts);
                    } else {
                        builder.add_f64_field(&f.name, opts);
                    }
                }
                _ => return Err(format!("unknown field type: {}", f.kind)),
            }
        }
        Ok(builder.build())
    }

    pub fn load(path: &str, schema: &Schema) -> Result<Self> {
        let config = match std::fs::read(Path::new(path).join(CONFIG_FILE)) {
            Ok(bytes) => serde_json::from_slice::<Self>(&bytes).map_err(|e| e.to_string())?,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound && *schema == super::schema() => {
                Self::legacy()
            }
            Err(e) => return Err(format!("cannot read adapter metadata: {e}")),
        };
        if config.schema()? != *schema {
            return Err("adapter metadata does not match index schema".into());
        }
        Ok(config)
    }

    pub fn save(&self, path: &str) -> Result<()> {
        use std::io::Write;
        let bytes = serde_json::to_vec_pretty(self).map_err(|e| e.to_string())?;
        let mut file = std::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(Path::new(path).join(CONFIG_FILE))
            .map_err(|e| e.to_string())?;
        file.write_all(&bytes)
            .and_then(|_| file.sync_all())
            .map_err(|e| e.to_string())
    }
    pub fn fingerprint(&self) -> Result<String> {
        // Stable FNV-1a detects accidental snapshot corruption; not authentication.
        let bytes = serde_json::to_vec(&self.dictionary).map_err(|e| e.to_string())?;
        let hash = bytes.iter().fold(0xcbf29ce484222325_u64, |h, b| {
            (h ^ u64::from(*b)).wrapping_mul(0x100000001b3)
        });
        Ok(format!("{hash:016x}"))
    }
}
