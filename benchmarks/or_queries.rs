//! Benchmark-only derivation from original queries. Never tokenizes the corpus.
use jieba_rs::{Jieba, TokenizeMode};
use std::collections::HashSet;
use std::fs::{File, OpenOptions};
use std::io::{BufRead, BufReader, BufWriter, Write};
use tantivy_query_grammar::{parse_query, Occur, UserInputAst, UserInputLeaf};

fn quote(word: &str) -> String {
    format!("\"{}\"", word.replace('\\', "\\\\").replace('"', "\\\""))
}
fn literals(ast: UserInputAst, out: &mut Vec<String>) {
    match ast {
        UserInputAst::Clause(children) => for (occur, child) in children {
            assert!(occur.is_none() || occur == Some(Occur::Should));
            literals(child, out);
        },
        UserInputAst::Leaf(leaf) => match *leaf {
            UserInputLeaf::Literal(l) => {
                assert!(l.field_name.is_none() && l.slop == 0 && !l.prefix);
                out.push(l.phrase);
            }
            _ => panic!("derived query was not a literal"),
        },
        _ => panic!("derived query contained a boost"),
    }
}
fn analyze(jieba: &Jieba, text: &str) -> Vec<String> {
    jieba.tokenize(text, TokenizeMode::Default, true).into_iter()
        .filter(|t| t.word.chars().any(char::is_alphanumeric))
        .map(|t| t.word.to_lowercase()).collect()
}
fn main() {
    // Exercise all parser metacharacters, reserved words, quoting and backslashes.
    for word in [r#"a"\b:c+(x) AND OR NOT IN~^*?[v]{k}/"#, "中文🙂", "a b", "OR", "a\\", "a\"b"] {
        let mut actual=Vec::new(); literals(parse_query(&quote(word)).unwrap(),&mut actual);
        assert_eq!(actual,vec![word]);
    }
    let args:Vec<_>=std::env::args().collect(); assert_eq!(args.len(),4,"ORIGINAL_QUERIES TOKENS_OUT OR_OUT");
    let input=BufReader::new(File::open(&args[1]).unwrap());
    let create=|p:&str| BufWriter::new(OpenOptions::new().write(true).create_new(true).open(p).unwrap());
    let mut tokens=create(&args[2]); let mut queries=create(&args[3]);
    let jieba=Jieba::new(); let mut rows=0; let mut total=0; let mut unique_total=0;
    let mut reanalyzed=0;
    for line in input.lines() {
        let line=line.unwrap(); let (id,text)=line.split_once('\t').unwrap();
        let words=analyze(&jieba,text); assert!(!words.is_empty(),"empty query {id}");
        assert!(words.iter().all(|w| !w.chars().any(char::is_whitespace)));
        let mut seen=HashSet::new();
        let unique:Vec<_>=words.iter().filter(|w| seen.insert((*w).clone())).cloned().collect();
        // QueryParser re-analyzes quoted literals through native Jieba. Contextual
        // HMM terms need not remain a single token; retain them, record the audit.
        for word in &unique {
            let again=analyze(&jieba,word);
            if again!=vec![word.clone()] { reanalyzed+=1; eprintln!("{id}\t{word}\t{}",again.join(" ")); }
        }
        let expression=unique.iter().map(|w|quote(w)).collect::<Vec<_>>().join(" OR ");
        let mut actual=Vec::new(); literals(parse_query(&expression).unwrap(),&mut actual); assert_eq!(actual,unique);
        writeln!(tokens,"{id}\t{}",words.join(" ")).unwrap();
        writeln!(queries,"{id}\t{expression}").unwrap();
        rows+=1;total+=words.len();unique_total+=unique.len();
    }
    println!("{{\"rows\":{rows},\"tokens\":{total},\"unique_terms\":{unique_total},\"jieba_rs\":\"0.8.1\",\"hmm\":true,\"lowercase\":true,\"literal_ast_roundtrip\":true,\"native_reanalysis_changed_literals\":{reanalyzed},\"metacharacter_tests\":6}}");
}
