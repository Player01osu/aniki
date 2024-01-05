use std::str::Chars;

fn matches_string(cursor: Chars, matches: &str) -> bool {
    if cursor.as_str().is_empty() {
        return false;
    }
    for (src, other) in cursor.zip(matches.chars()) {
        if src.to_ascii_uppercase() != (other.to_ascii_uppercase()) {
            return false;
        }
    }
    true
}

fn consume_bracket(s: &mut Chars, buf: &mut String, terminating_bracket: char) {
    for c in s.by_ref() {
        if c == terminating_bracket {
            break;
        }
    }
    sanitize_name(s, buf);
}

fn consume_n(s: &mut Chars, n: usize) {
    for _ in 0..n {
        s.next();
    }
}

fn consume_str_len(s: &mut Chars, other: &str) {
    consume_n(s, other.len());
}

fn consume_match_str(s: &mut Chars, match_str: &str) -> bool {
    if matches_string(s.clone(), match_str) {
        consume_str_len(s, match_str);
        return true;
    }
    false
}

fn consume_resolution(s: &mut Chars) -> bool {
    [
        consume_match_str(s, "720p"),
        consume_match_str(s, "720"),
        consume_match_str(s, "1080p"),
        consume_match_str(s, "1080"),
        consume_match_str(s, "Hi10p"),
    ]
    .iter()
    .any(|v| *v)
}

fn consume_codec(s: &mut Chars) -> bool {
    [
        consume_match_str(s, "x264"),
        consume_match_str(s, "x265"),
        consume_match_str(s, "H 264"),
    ]
    .iter()
    .any(|v| *v)
}

fn consume_format(s: &mut Chars) -> bool {
    [
        consume_match_str(s, "FLAC"),
        consume_match_str(s, "AAC"),
        consume_match_str(s, "AAC2.0"),
        consume_match_str(s, "2.1"),
        consume_match_str(s, "2.0"),
        consume_match_str(s, "5.1"),
        consume_match_str(s, "NF"),
    ]
    .iter()
    .any(|v| *v)
}

fn consume_rip(s: &mut Chars) -> bool {
    [
        consume_match_str(s, "BrRip"),
        consume_match_str(s, "BluRay"),
        consume_match_str(s, "WEB-DL"),
        consume_match_str(s, "WEB"),
    ]
    .iter()
    .any(|v| *v)
}

fn consume_space(s: &mut Chars, buf: &mut String) {
    loop {
        let peak = match s.clone().next() {
            Some(c) => c,
            None => return,
        };
        if peak == ' ' {
            s.next();
        } else {
            buf.push(' ');
            return sanitize_name(s, buf);
        }
    }
}

pub fn sanitize_name(s: &mut Chars, buf: &mut String) {
    let c = match s.next() {
        Some(c) => c,
        None => return,
    };

    match c {
        '[' => consume_bracket(s, buf, ']'),
        '(' => consume_bracket(s, buf, ')'),
        ' ' => consume_space(s, buf),
        '.' => {
            buf.push(' ');
            sanitize_name(s, buf);
        }
        _ => {
            if ![
                consume_rip(s),
                consume_format(s),
                consume_codec(s),
                consume_resolution(s),
            ]
            .iter()
            .any(|v| *v)
            {
                buf.push(c);
            }

            sanitize_name(s, buf);
        }
    }
}
