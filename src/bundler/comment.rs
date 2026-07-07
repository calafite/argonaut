use super::*;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum State {
    Normal,
    String(char),
    SingleLineComment,
    MultiLineComment,
}

pub fn strip_comments(input: &str) -> String {
    let mut output = String::with_capacity(input.len());
    let mut chars = input.chars().peekable();
    let mut state = State::Normal;

    while let Some(character) = chars.next() {
        match state {
            State::SingleLineComment => {
                if character == LINE_BREAK {
                    state = State::Normal;
                    output.push(LINE_BREAK);
                }
            }
            State::MultiLineComment => {
                if character == STAR && chars.peek() == Some(&SLASH) {
                    chars.next();
                    state = State::Normal;
                    output.push(' ');
                } else if character == LINE_BREAK {
                    output.push(LINE_BREAK);
                }
            }
            State::String(quote) => {
                output.push(character);
                if character == BACKSLASH {
                    if let Some(next) = chars.next() {
                        output.push(next);
                    }
                } else if character == quote {
                    state = State::Normal;
                }
            }
            State::Normal => {
                if character == QUOTE || character == SINGLE_QUOTE {
                    state = State::String(character);
                    output.push(character);
                } else if character == SLASH {
                    if chars.peek() == Some(&SLASH) {
                        chars.next();
                        state = State::SingleLineComment;
                    } else if chars.peek() == Some(&STAR) {
                        chars.next();
                        state = State::MultiLineComment;
                    } else {
                        output.push(character);
                    }
                } else {
                    output.push(character);
                }
            }
        }
    }
    output
}
