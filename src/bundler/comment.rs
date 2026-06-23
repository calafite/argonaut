pub fn strip_comments(input: &str) -> String {
    let mut out = String::with_capacity(input.len());
    let mut chars = input.chars().peekable();
    let mut in_string = false;
    let mut string_char = '"';
    let mut in_s_line = false;
    let mut in_m_line = false;

    while let Some(c) = chars.next() {
        if in_s_line {
            if c == '\n' {
                in_s_line = false;
                out.push('\n');
            }
            continue;
        }
        if in_m_line {
            if c == '*' && chars.peek() == Some(&'/') {
                chars.next();
                in_m_line = false;
            } else if c == '\n' {
                out.push('\n'); 
            }
            continue;
        }
        if in_string {
            out.push(c);
            if c == '\\' {
                if let Some(next) = chars.next() {
                    out.push(next);
                }
            } else if c == string_char {
                in_string = false;
            }
            continue;
        }

        if c == '"' || c == '\'' {
            in_string = true;
            string_char = c;
            out.push(c);
        } else if c == '/' {
            if chars.peek() == Some(&'/') {
                chars.next();
                in_s_line = true;
            } else if chars.peek() == Some(&'*') {
                chars.next();
                in_m_line = true;
            } else {
                out.push(c);
            }
        } else {
            out.push(c);
        }
    }
    out
}
