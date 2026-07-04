use std::collections::{HashMap, HashSet};

#[derive(Debug, Clone)]
pub enum Token {
    Preproc(String),
    StringLiteral(String),
    CharLiteral(String),
    Word(String),
    Punct(String),
}

pub fn minify_bundle(bundle: &str) -> String {
    let tokens = lex_cpp(bundle);

    let mut freq = HashMap::new();
    let mut existing_words = HashSet::new();

    for t in &tokens {
        if let Token::Word(w) = t {
            existing_words.insert(w.clone());
            if is_safe_to_macro(w) && w.len() >= 4 {
                *freq.entry(w.clone()).or_insert(0) += 1;
            }
        }
    }

    let mut ranked: Vec<_> = freq.into_iter().collect();
    ranked.sort_by_key(|(w, c)| std::cmp::Reverse(w.len() * c));

    let mut dictionary = HashMap::new();
    let mut macro_defs = Vec::new();
    let mut macro_counter = 0;

    for (word, count) in ranked {
        let mut macro_name = format!("Z{}", macro_counter);
        while existing_words.contains(&macro_name) {
            macro_counter += 1;
            macro_name = format!("Z{}", macro_counter);
        }

        let m_len = macro_name.len();
        let cost = 9 + m_len + word.len();
        let old_size = word.len() * count;
        let new_size = m_len * count;

        let savings = old_size as isize - new_size as isize - cost as isize;

        if savings > 10 {
            dictionary.insert(word.clone(), macro_name.clone());
            macro_defs.push(format!("#define {} {}", macro_name, word));
            macro_counter += 1;
        }
    }

    let mut out = String::new();
    out.push_str("// Cursed Minification Level: Maximum\n");

    for def in macro_defs {
        out.push_str(&def);
        out.push('\n');
    }

    let mut last_char_alnum = false;

    for token in tokens {
        match token {
            Token::Preproc(p) => {
                if !out.ends_with('\n') && !out.is_empty() {
                    out.push('\n');
                }
                out.push_str(&p);
                if !p.ends_with('\n') {
                    out.push('\n');
                }
                last_char_alnum = false;
            }
            Token::StringLiteral(s) | Token::CharLiteral(s) => {
                out.push_str(&s);
                last_char_alnum = false;
            }
            Token::Punct(p) => {
                out.push_str(&p);
                last_char_alnum = false;
            }
            Token::Word(w) => {
                let mapped = dictionary.get(&w).unwrap_or(&w);
                let starts_alnum = mapped
                    .chars()
                    .next()
                    .is_some_and(|c| c.is_ascii_alphanumeric() || c == '_');

                if last_char_alnum && starts_alnum {
                    out.push(' ');
                }

                out.push_str(mapped);
                last_char_alnum = mapped
                    .chars()
                    .last()
                    .is_some_and(|c| c.is_ascii_alphanumeric() || c == '_');
            }
        }
    }

    out
}

fn is_safe_to_macro(w: &str) -> bool {
    let first = w.chars().next().unwrap();
    if first.is_ascii_digit() || w.starts_with("__") {
        return false;
    }
    let reserved = [
        "defined", "include", "pragma", "define", "undef", "ifdef", "ifndef", "elif", "endif",
    ];
    !reserved.contains(&w)
}

fn lex_cpp(input: &str) -> Vec<Token> {
    let mut tokens = Vec::new();
    let mut chars = input.chars().peekable();
    let mut is_start_of_line = true;

    while let Some(&c) = chars.peek() {
        if c == '\n' {
            is_start_of_line = true;
            chars.next();
            continue;
        }
        if c.is_whitespace() {
            chars.next();
            continue;
        }

        if c == '#' && is_start_of_line {
            let mut preproc = String::new();
            while let Some(pc) = chars.next() {
                preproc.push(pc);
                if pc == '\n' {
                    break;
                }
                if pc == '\\'
                    && let Some(&nc) = chars.peek()
                    && (nc == '\n' || nc == '\r')
                {
                    preproc.push(chars.next().unwrap());
                    if preproc.ends_with('\r') && chars.peek() == Some(&'\n') {
                        preproc.push(chars.next().unwrap());
                    }
                }
            }
            tokens.push(Token::Preproc(preproc));
            is_start_of_line = true;
            continue;
        }

        is_start_of_line = false;

        if c == '/' {
            chars.next();
            if let Some(&nc) = chars.peek() {
                if nc == '/' {
                    chars.next();
                    for cc in chars.by_ref() {
                        if cc == '\n' {
                            is_start_of_line = true;
                            break;
                        }
                    }
                    continue;
                } else if nc == '*' {
                    chars.next();
                    let mut last_was_star = false;
                    for cc in chars.by_ref() {
                        if last_was_star && cc == '/' {
                            break;
                        }
                        last_was_star = cc == '*';
                    }
                    continue;
                }
            }
            tokens.push(Token::Punct("/".to_string()));
            continue;
        }

        if c == '"' {
            let mut s = String::new();
            s.push(chars.next().unwrap());
            let mut escape = false;
            for sc in chars.by_ref() {
                s.push(sc);
                if escape {
                    escape = false;
                } else if sc == '\\' {
                    escape = true;
                } else if sc == '"' {
                    break;
                }
            }
            tokens.push(Token::StringLiteral(s));
            continue;
        }

        if c == '\'' {
            let mut s = String::new();
            s.push(chars.next().unwrap());
            let mut escape = false;
            for sc in chars.by_ref() {
                s.push(sc);
                if escape {
                    escape = false;
                } else if sc == '\\' {
                    escape = true;
                } else if sc == '\'' {
                    break;
                }
            }
            tokens.push(Token::CharLiteral(s));
            continue;
        }

        if c.is_ascii_alphanumeric() || c == '_' {
            let mut w = String::new();
            while let Some(&wc) = chars.peek() {
                if wc.is_ascii_alphanumeric() || wc == '_' {
                    w.push(chars.next().unwrap());
                } else {
                    break;
                }
            }
            tokens.push(Token::Word(w));
            continue;
        }

        tokens.push(Token::Punct(chars.next().unwrap().to_string()));
    }

    tokens
}
