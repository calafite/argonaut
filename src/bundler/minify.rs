use std::{
    cmp::Reverse,
    collections::{HashMap, HashSet},
};

#[derive(Debug, Clone)]
pub enum Token {
    Preproc(String),
    StringLiteral(String),
    CharLiteral(String),
    Word(String),
    Punct(String),
}

const LINE_BREAK: char = '\n';
const CARRIAGE_RETURN: char = '\r';
const BACKSLASH: char = '\\';
const UNDERSCORE: char = '_';
const DOUBLE_UNDERSCORE: &str = "__";
const SLASH: char = '/';
const STAR: char = '*';
const HASH: char = '#';
const QUOTE: char = '"';
const SINGLE_QUOTE: char = '\'';

const RESERVED_KEYWORDS: [&str; 9] = [
    "defined", //
    "include", //
    "pragma",  //
    "define",  //
    "undef",   //
    "ifdef",   //
    "ifndef",  //
    "elif",    //
    "endif",   //
];

pub struct Minifier;

impl Minifier {
    fn analyse(tokens: &[Token]) -> (HashMap<String, usize>, HashSet<String>) {
        let mut frequency = HashMap::new();
        let mut existing = HashSet::new();
        for token in tokens {
            if let Token::Word(word) = token {
                existing.insert(word.clone());
                if Self::safe_macro(word) && word.len() >= 4 {
                    *frequency.entry(word.clone()).or_insert(0) += 1;
                }
            }
        }
        (frequency, existing)
    }

    fn optimise(
        frequency: HashMap<String, usize>,
        existing: &HashSet<String>,
    ) -> (HashMap<String, String>, Vec<String>) {
        let mut ranked: Vec<_> = frequency.into_iter().collect();
        ranked.sort_by_key(Self::rank_tokens);

        let mut dictionary = HashMap::new();
        let mut macro_definitions = Vec::new();
        let mut macro_counter = 0;

        for (word, count) in ranked {
            let mut macro_name = format!("Z{}", macro_counter);
            while existing.contains(&macro_name) {
                macro_counter += 1;
                macro_name = format!("Z{}", macro_counter);
            }

            let macro_len = macro_name.len();
            let cost = 9 + macro_len + word.len();
            let old_size = word.len() * count;
            let new_size = macro_len * count;

            let savings = old_size as isize - new_size as isize - cost as isize;

            if savings > 10 {
                dictionary.insert(word.clone(), macro_name.clone());
                macro_definitions.push(format!("#define {} {}", macro_name, word));
                macro_counter += 1;
            }
        }

        (dictionary, macro_definitions)
    }

    fn assemble(
        tokens: Vec<Token>,
        dictionary: &HashMap<String, String>,
        macro_defs: &[String],
    ) -> String {
        let mut output = String::new();
        output.push_str("// COMPRESSED BY ARGONAUT");

        for definition in macro_defs {
            output.push_str(definition);
            output.push(LINE_BREAK);
        }

        let mut last_alnum = false;

        for token in tokens {
            match token {
                Token::Preproc(preprocessor_string) => {
                    if !output.ends_with(LINE_BREAK) && !output.is_empty() {
                        output.push(LINE_BREAK);
                    }
                    output.push_str(&preprocessor_string);
                    if !preprocessor_string.ends_with(LINE_BREAK) {
                        output.push(LINE_BREAK);
                    }
                    last_alnum = false;
                }
                Token::StringLiteral(str) | Token::CharLiteral(str) => {
                    output.push_str(&str);
                    last_alnum = false;
                }
                Token::Punct(punctuation) => {
                    output.push_str(&punctuation);
                    last_alnum = false;
                }
                Token::Word(word) => {
                    let closure = |character: char| {
                        character.is_ascii_alphanumeric() || character == UNDERSCORE
                    };
                    let mapped = dictionary.get(&word).unwrap_or(&word);
                    let starts_alnum = mapped.chars().next().is_some_and(closure);

                    if last_alnum && starts_alnum {
                        output.push(' ');
                    }

                    output.push_str(mapped);
                    last_alnum = mapped.chars().last().is_some_and(closure);
                }
            }
        }
        output
    }

    fn safe_macro(word: &str) -> bool {
        let mut characters = word.chars();
        let first = characters.next();
        if first.is_none() {
            return false;
        }
        let first = first.unwrap();
        if first.is_ascii_digit() || word.starts_with(DOUBLE_UNDERSCORE) {
            return false;
        }
        !RESERVED_KEYWORDS.contains(&word)
    }

    fn lex_cpp(input: &str) -> Vec<Token> {
        let mut tokens = Vec::new();
        let mut chars = input.chars().peekable();
        let mut line_start = true;

        while let Some(&character) = chars.peek() {
            if character == LINE_BREAK {
                line_start = true;
                chars.next();
                continue;
            }

            if character.is_whitespace() {
                chars.next();
                continue;
            }

            if character == HASH && line_start {
                let macro_string = Self::consume_macro(&mut chars);
                tokens.push(Token::Preproc(macro_string));
                line_start = true;
                continue;
            }

            line_start = false;

            if character == SLASH {
                chars.next();
                if let Some(&nc) = chars.peek() {
                    if nc == SLASH {
                        chars.next();
                        Self::skip_comment(&mut chars, &mut line_start);
                        continue;
                    } else if nc == STAR {
                        chars.next();
                        Self::skip_block(&mut chars);
                        continue;
                    }
                }
                tokens.push(Token::Punct(String::from("/")));
                continue;
            }

            if character == QUOTE {
                let string = Self::consume_literal(&mut chars, QUOTE);
                tokens.push(Token::StringLiteral(string));
                continue;
            }

            if character == SINGLE_QUOTE {
                let string = Self::consume_literal(&mut chars, SINGLE_QUOTE);
                tokens.push(Token::CharLiteral(string));
                continue;
            }

            if character.is_alphanumeric() || character == UNDERSCORE {
                let word = Self::consume_word(&mut chars);
                tokens.push(Token::Word(word));
                continue;
            }

            if let Some(unmatched) = chars.next() {
                tokens.push(Token::Punct(unmatched.to_string()));
            }
        }

        tokens
    }

    fn skip_comment(chars: &mut std::iter::Peekable<std::str::Chars>, line_start: &mut bool) {
        for character in chars.by_ref() {
            if character == '\n' {
                *line_start = true;
                break;
            }
        }
    }

    fn skip_block(chars: &mut std::iter::Peekable<std::str::Chars>) {
        let mut was_star = false;
        for character in chars.by_ref() {
            if was_star && character == SLASH {
                break;
            }
            was_star = character == STAR;
        }
    }

    fn consume_macro(chars: &mut std::iter::Peekable<std::str::Chars>) -> String {
        let mut preprocessor_string = String::new();

        loop {
            let next = chars.next();
            let macro_character = match next {
                Some(character) => character,
                None => break,
            };
            preprocessor_string.push(macro_character);
            if macro_character == LINE_BREAK {
                break;
            }
            if macro_character != BACKSLASH {
                continue;
            }
            let peeked = chars.peek();
            let next_character = match peeked {
                Some(character) => *character,
                None => continue,
            };
            let lb = next_character == LINE_BREAK;
            let cr = next_character == CARRIAGE_RETURN;
            let continuation = lb || cr;
            if !continuation {
                continue;
            }
            let continued = match chars.next() {
                Some(character) => character,
                None => break,
            };
            preprocessor_string.push(continued);
            let cr_end = preprocessor_string.ends_with(CARRIAGE_RETURN);
            if !cr_end {
                continue;
            }
            let next_lf = chars.peek() == Some(&LINE_BREAK);
            if !next_lf {
                continue;
            }
            let lf = match chars.next() {
                Some(character) => character,
                None => break,
            };
            preprocessor_string.push(lf);
        }
        preprocessor_string
    }

    fn consume_literal(
        chars: &mut std::iter::Peekable<std::str::Chars>,
        delimiter: char,
    ) -> String {
        let mut string = String::new();
        let next_character = chars.next();
        if next_character.is_some() {
            let next_character = next_character.unwrap();
            string.push(next_character);
            let mut escape = false;
            for character in chars.by_ref() {
                string.push(character);
                if escape {
                    escape = false;
                } else if character == BACKSLASH {
                    escape = true;
                } else if character == delimiter {
                    break;
                }
            }
        }
        string
    }

    fn consume_word(chars: &mut std::iter::Peekable<std::str::Chars>) -> String {
        let mut word = String::new();
        while let Some(&word_character) = chars.peek() {
            if word_character.is_alphanumeric() || word_character == UNDERSCORE {
                let next_char = chars.next();
                if next_char.is_some() {
                    let next_char = next_char.unwrap();
                    word.push(next_char);
                }
            } else {
                break;
            }
        }
        word
    }

    fn rank_tokens((word, count): &(String, usize)) -> Reverse<usize> {
        Reverse(word.len() * *count)
    }
}
