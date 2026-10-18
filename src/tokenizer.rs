use crate::config::Dictionary;
use jieba_rs::{Jieba, TokenizeMode};
use std::sync::{Arc, Mutex, OnceLock, Weak};
use tantivy::tokenizer::{Token, TokenStream, Tokenizer};

static JIEBA: OnceLock<Jieba> = OnceLock::new();

#[derive(Clone)]
pub struct ChineseTokenizer;

pub struct ChineseStream {
    tokens: std::vec::IntoIter<Token>,
    current: Token,
}

impl Tokenizer for ChineseTokenizer {
    type TokenStream<'a> = ChineseStream;
    fn token_stream<'a>(&'a mut self, text: &'a str) -> ChineseStream {
        stream(JIEBA.get_or_init(Jieba::new), text)
    }
}

// Configuration lives inside the shared allocation: dead weak entries retain neither
// dictionary strings nor Jieba's heap. Only live instances participate in equality.
pub struct ConfiguredJieba {
    dictionary: Dictionary,
    jieba: Jieba,
}
static CONFIGURED: Mutex<Vec<Weak<ConfiguredJieba>>> = Mutex::new(Vec::new());

pub fn configured_jieba(dictionary: &Dictionary) -> Arc<ConfiguredJieba> {
    let mut instances = CONFIGURED.lock().unwrap();
    instances.retain(|entry| entry.strong_count() != 0);
    for entry in instances.iter() {
        if let Some(instance) = entry.upgrade() {
            if instance.dictionary == *dictionary {
                return instance;
            }
        }
    }
    // Serialize cold construction, including concurrent first opens. Mutation is
    // complete before publication; token streams only borrow the immutable Jieba.
    let mut jieba = Jieba::new();
    for entry in &dictionary.entries {
        jieba.add_word(&entry.word, Some(entry.frequency), None);
    }
    let instance = Arc::new(ConfiguredJieba {
        dictionary: dictionary.clone(),
        jieba,
    });
    instances.push(Arc::downgrade(&instance));
    instance
}

#[derive(Clone)]
pub struct ConfiguredTokenizer(pub Arc<ConfiguredJieba>);
impl Tokenizer for ConfiguredTokenizer {
    type TokenStream<'a> = ChineseStream;
    fn token_stream<'a>(&'a mut self, text: &'a str) -> ChineseStream {
        stream(&self.0.jieba, text)
    }
}
fn stream(jieba: &Jieba, text: &str) -> ChineseStream {
    // Jieba offsets count Unicode characters; Tantivy requires UTF-8 byte offsets.
    let offsets: Vec<usize> = text
        .char_indices()
        .map(|(i, _)| i)
        .chain([text.len()])
        .collect();
    let tokens = jieba
        .tokenize(text, TokenizeMode::Default, true)
        .into_iter()
        .filter(|item| item.word.chars().any(char::is_alphanumeric))
        .enumerate()
        .map(|(position, item)| Token {
            offset_from: offsets[item.start],
            offset_to: offsets[item.end],
            position,
            text: item.word.to_owned(),
            position_length: 1,
        })
        .collect::<Vec<_>>();
    ChineseStream {
        tokens: tokens.into_iter(),
        current: Token::default(),
    }
}

impl TokenStream for ChineseStream {
    fn advance(&mut self) -> bool {
        if let Some(token) = self.tokens.next() {
            self.current = token;
            true
        } else {
            false
        }
    }
    fn token(&self) -> &Token {
        &self.current
    }
    fn token_mut(&mut self) -> &mut Token {
        &mut self.current
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn offsets_round_trip_chinese_and_emoji() {
        let input = "鸿蒙🙂端侧检索 Qwen3-4B 16GB";
        let mut tokenizer = ChineseTokenizer;
        let mut stream = tokenizer.token_stream(input);
        let mut count = 0;
        while stream.advance() {
            let token = stream.token();
            assert_eq!(&input[token.offset_from..token.offset_to], token.text);
            assert_eq!(token.position, count);
            count += 1;
        }
        assert!(count >= 5);
    }

    fn dictionary(words: &[(&str, usize)]) -> Dictionary {
        Dictionary {
            mode: "extend_default".into(),
            entries: words
                .iter()
                .map(|(word, frequency)| crate::config::DictionaryEntry {
                    word: (*word).into(),
                    frequency: *frequency,
                })
                .collect(),
        }
    }
    fn tokens(instance: Arc<ConfiguredJieba>, text: &str) -> Vec<String> {
        let mut tokenizer = ConfiguredTokenizer(instance);
        let mut stream = tokenizer.token_stream(text);
        let mut words = Vec::new();
        while stream.advance() {
            words.push(stream.token().text.clone());
        }
        words
    }

    #[test]
    fn configured_instances_share_only_complete_equal_dictionaries() {
        fn send_sync<T: Send + Sync>() {}
        send_sync::<ConfiguredJieba>();
        let default = configured_jieba(&dictionary(&[]));
        let a = dictionary(&[("鸿蒙星河检索", 1_000_000_000)]);
        let b = dictionary(&[("端侧向量检索", 1_000_000_000)]);
        let first = configured_jieba(&a);
        let second = configured_jieba(&b);
        assert!(Arc::ptr_eq(&default, &configured_jieba(&dictionary(&[]))));
        assert!(Arc::ptr_eq(&first, &configured_jieba(&a.clone())));
        assert!(!Arc::ptr_eq(&first, &second));
        assert!(!Arc::ptr_eq(&default, &first));
        let input = "鸿蒙星河检索 端侧向量检索";
        let original = tokens(default.clone(), input);
        assert!(tokens(first.clone(), input).contains(&"鸿蒙星河检索".into()));
        assert!(tokens(second.clone(), input).contains(&"端侧向量检索".into()));
        assert!(!original.contains(&"鸿蒙星河检索".into()));
        assert!(!original.contains(&"端侧向量检索".into()));
        assert_eq!(tokens(default, input), original);
        let different_frequency = configured_jieba(&dictionary(&[("鸿蒙星河检索", 1)]));
        assert!(!Arc::ptr_eq(&first, &different_frequency));
        let ordered = configured_jieba(&dictionary(&[("鸿蒙星河检索", 1), ("端侧向量检索", 2)]));
        let reversed = configured_jieba(&dictionary(&[("端侧向量检索", 2), ("鸿蒙星河检索", 1)]));
        assert!(!Arc::ptr_eq(&ordered, &reversed));
    }

    #[test]
    fn configured_weak_pool_reclaims_and_prunes_dead_entries() {
        let d = dictionary(&[("弱引用回收独立验证词条", 123_456)]);
        let a = configured_jieba(&d);
        let weak = Arc::downgrade(&a);
        let b = configured_jieba(&d);
        assert!(Arc::ptr_eq(&a, &b));
        drop(a);
        assert!(weak.upgrade().is_some());
        drop(b);
        assert!(weak.upgrade().is_none());
        // An expired Weak cannot retain the dropped instance's Dictionary Vec
        // or Jieba heap; the next access also removes the weak allocation entry.
        let reopened = configured_jieba(&d);
        assert!(!Weak::ptr_eq(&weak, &Arc::downgrade(&reopened)));
        assert!(!CONFIGURED
            .lock()
            .unwrap()
            .iter()
            .any(|entry| Weak::ptr_eq(entry, &weak)));
        let second_weak = Arc::downgrade(&reopened);
        drop(reopened);
        assert!(second_weak.upgrade().is_none());
        let _live = configured_jieba(&dictionary(&[("回收清理触发词条", 123_457)]));
        assert!(!CONFIGURED
            .lock()
            .unwrap()
            .iter()
            .any(|entry| Weak::ptr_eq(entry, &second_weak)));
    }

    #[test]
    fn configured_concurrent_first_lookup_shares_one_immutable_instance() {
        let d = dictionary(&[("并发首次构造独立验证词条", 123_458)]);
        let barrier = Arc::new(std::sync::Barrier::new(6));
        let owners: Vec<_> = std::thread::scope(|scope| {
            let tasks: Vec<_> = (0..6)
                .map(|_| {
                    let barrier = barrier.clone();
                    let d = d.clone();
                    scope.spawn(move || {
                        barrier.wait();
                        let instance = configured_jieba(&d);
                        assert!(tokens(instance.clone(), "并发首次构造独立验证词条")
                            .contains(&"并发首次构造独立验证词条".into()));
                        instance
                    })
                })
                .collect();
            tasks.into_iter().map(|task| task.join().unwrap()).collect()
        });
        assert!(owners.iter().all(|owner| Arc::ptr_eq(&owners[0], owner)));
        let weak = Arc::downgrade(&owners[0]);
        drop(owners);
        assert!(weak.upgrade().is_none());
    }

    #[test]
    fn configured_handles_concurrently_open_release_and_reopen() {
        use crate::{config, SearchHandle};
        let path = std::env::temp_dir().join(format!(
            "tantivy-pool-handles-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        let path = path.to_str().unwrap().to_owned();
        let d = dictionary(&[("并发只读句柄独立验证词条", 123_459)]);
        let mut config = config::Config::legacy();
        config.version = 3;
        config.tokenizer = "zh_jieba_v3".into();
        config.dictionary = Some(d.clone());
        config.dictionary_fingerprint = Some(config.fingerprint().unwrap());
        let mut writer = SearchHandle::open_config_with_writer(
            &path,
            Some(config),
            Some(crate::writer::WriterConfig::default()),
        )
        .unwrap();
        writer.commit().unwrap();
        let instance = configured_jieba(&d);
        let weak = Arc::downgrade(&instance);
        drop(instance);
        drop(writer);
        assert!(
            weak.upgrade().is_none(),
            "closed writer retained dictionary"
        );
        let barrier = Arc::new(std::sync::Barrier::new(6));
        let owners: Vec<_> = std::thread::scope(|scope| {
            let tasks: Vec<_> = (0..6)
                .map(|_| {
                    let barrier = barrier.clone();
                    let path = path.clone();
                    scope.spawn(move || {
                        let opened = SearchHandle::open_config_with_writer(&path, None, None);
                        barrier.wait();
                        let h = opened.unwrap();
                        let instance = configured_jieba(h.config.dictionary.as_ref().unwrap());
                        // The registered analyzer must itself retain an owner.
                        assert!(Arc::strong_count(&instance) >= 2);
                        let mut analyzer = h.index.tokenizers().get(&h.config.tokenizer).unwrap();
                        let mut stream = analyzer.token_stream("并发只读句柄独立验证词条");
                        assert!(stream.advance());
                        assert_eq!(stream.token().text, "并发只读句柄独立验证词条");
                        drop(stream);
                        drop(analyzer);
                        drop(h);
                        instance
                    })
                })
                .collect();
            tasks.into_iter().map(|task| task.join().unwrap()).collect()
        });
        assert!(owners.iter().all(|owner| Arc::ptr_eq(&owners[0], owner)));
        let weak = Arc::downgrade(&owners[0]);
        drop(owners);
        assert!(
            weak.upgrade().is_none(),
            "closed readers retained dictionary"
        );
        let reopened = SearchHandle::open_config_with_writer(&path, None, None).unwrap();
        let instance = configured_jieba(reopened.config.dictionary.as_ref().unwrap());
        assert!(!Weak::ptr_eq(&weak, &Arc::downgrade(&instance)));
        let weak = Arc::downgrade(&instance);
        drop(instance);
        drop(reopened);
        assert!(weak.upgrade().is_none());
        std::fs::remove_dir_all(path).unwrap();
    }
}
