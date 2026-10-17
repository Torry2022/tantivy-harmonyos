use jieba_rs::{Jieba, TokenizeMode};
use std::sync::OnceLock;
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
        // Jieba offsets count Unicode characters; Tantivy requires UTF-8 byte offsets.
        let offsets: Vec<usize> = text
            .char_indices()
            .map(|(i, _)| i)
            .chain([text.len()])
            .collect();
        let tokens = JIEBA
            .get_or_init(Jieba::new)
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
}
