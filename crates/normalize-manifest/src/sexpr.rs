//! Minimal S-expression parser shared by the Dune, Common Lisp, and Racket
//! manifest parsers.
//!
//! Supports:
//! - `;` line comments
//! - `#| … |#` block comments (Common Lisp / Racket)
//! - `'expr` → `(quote expr)` reader macro
//! - `` `expr `` and `,expr` — quasiquote / unquote (inner form passed through)
//! - `[…]` bracket lists (Racket — treated as `(…)`)
//! - `#:sym` and `:kw` atoms stored as-is

/// An S-expression value.
#[derive(Debug, Clone, PartialEq)]
pub enum Sexp {
    /// A symbol, keyword (`:foo`, `#:bar`), number, or other bare token.
    Atom(String),
    /// A string literal — content only, **without** surrounding `"` characters.
    Str(String),
    /// A parenthesised or bracketed list.
    List(Vec<Sexp>),
}

impl Sexp {
    /// Parse all top-level S-expressions from `input`.
    pub fn parse(input: &str) -> Vec<Self> {
        let chars: Vec<char> = input.chars().collect();
        let mut pos = 0;
        let mut out = Vec::new();
        while pos < chars.len() {
            let before = pos;
            skip_ws(&chars, &mut pos);
            if pos >= chars.len() {
                break;
            }
            if let Some(tok) = read_sexp(&chars, &mut pos) {
                out.push(tok);
            } else if pos == before {
                // Nothing consumed and no token — skip one char to avoid spinning.
                pos += 1;
            }
        }
        out
    }

    /// Return the atom string if this is `Sexp::Atom`.
    pub fn as_atom(&self) -> Option<&str> {
        match self {
            Self::Atom(s) => Some(s),
            _ => None,
        }
    }

    /// Return the string content if this is `Sexp::Str`.
    pub fn as_str(&self) -> Option<&str> {
        match self {
            Self::Str(s) => Some(s),
            _ => None,
        }
    }

    /// Return the text content whether this is `Sexp::Atom` or `Sexp::Str`.
    pub fn as_text(&self) -> Option<&str> {
        match self {
            Self::Atom(s) | Self::Str(s) => Some(s),
            _ => None,
        }
    }

    /// Return the children if this is `Sexp::List`.
    pub fn as_list(&self) -> Option<&[Sexp]> {
        match self {
            Self::List(v) => Some(v),
            _ => None,
        }
    }

    /// If this is a `List` whose first element is `Atom(head)`, return the
    /// remaining items (everything after the head).
    pub fn tagged_list(&self, head: &str) -> Option<&[Sexp]> {
        let items = self.as_list()?;
        match items.first() {
            Some(Sexp::Atom(h)) if h == head => Some(&items[1..]),
            _ => None,
        }
    }
}

/// Find the value after a keyword atom in a flat token slice (case-sensitive).
///
/// Searches for `Atom(key)` and returns the next item.
pub fn kw_arg<'a>(items: &'a [Sexp], key: &str) -> Option<&'a Sexp> {
    let mut iter = items.iter();
    while let Some(tok) = iter.next() {
        if let Sexp::Atom(k) = tok
            && k == key
        {
            return iter.next();
        }
    }
    None
}

// ============================================================================
// Internal parser
// ============================================================================

fn skip_ws(chars: &[char], pos: &mut usize) {
    while *pos < chars.len() {
        match chars[*pos] {
            ' ' | '\t' | '\n' | '\r' => *pos += 1,
            ';' => {
                while *pos < chars.len() && chars[*pos] != '\n' {
                    *pos += 1;
                }
            }
            '#' if *pos + 1 < chars.len() && chars[*pos + 1] == '|' => {
                *pos += 2;
                while *pos + 1 < chars.len() {
                    if chars[*pos] == '|' && chars[*pos + 1] == '#' {
                        *pos += 2;
                        break;
                    }
                    *pos += 1;
                }
            }
            _ => break,
        }
    }
}

fn read_sexp(chars: &[char], pos: &mut usize) -> Option<Sexp> {
    skip_ws(chars, pos);
    if *pos >= chars.len() {
        return None;
    }

    match chars[*pos] {
        '(' | '[' => {
            let close = if chars[*pos] == '(' { ')' } else { ']' };
            *pos += 1;
            let mut children = Vec::new();
            loop {
                skip_ws(chars, pos);
                if *pos >= chars.len() {
                    break;
                }
                if chars[*pos] == close {
                    *pos += 1;
                    break;
                }
                let before = *pos;
                if let Some(tok) = read_sexp(chars, pos) {
                    children.push(tok);
                } else if *pos == before {
                    *pos += 1; // skip unexpected char
                }
            }
            Some(Sexp::List(children))
        }
        ')' | ']' => {
            *pos += 1;
            None
        }
        '"' => {
            *pos += 1;
            let mut s = String::new();
            while *pos < chars.len() && chars[*pos] != '"' {
                if chars[*pos] == '\\' {
                    *pos += 1;
                    if *pos < chars.len() {
                        s.push(unescape_char(chars[*pos]));
                        *pos += 1;
                    }
                } else {
                    s.push(chars[*pos]);
                    *pos += 1;
                }
            }
            if *pos < chars.len() {
                *pos += 1; // closing "
            }
            Some(Sexp::Str(s))
        }
        '\'' => {
            *pos += 1;
            let inner = read_sexp(chars, pos)?;
            Some(Sexp::List(vec![Sexp::Atom("quote".to_string()), inner]))
        }
        '`' => {
            *pos += 1;
            read_sexp(chars, pos)
        }
        ',' => {
            *pos += 1;
            // ,@ splice — consume '@' too
            if *pos < chars.len() && chars[*pos] == '@' {
                *pos += 1;
            }
            read_sexp(chars, pos)
        }
        _ => {
            let mut s = String::new();
            while *pos < chars.len()
                && !matches!(
                    chars[*pos],
                    ' ' | '\t' | '\n' | '\r' | '(' | ')' | '[' | ']' | '"' | ';' | '\'' | '`' | ','
                )
            {
                s.push(chars[*pos]);
                *pos += 1;
            }
            if s.is_empty() {
                None
            } else {
                Some(Sexp::Atom(s))
            }
        }
    }
}

fn unescape_char(c: char) -> char {
    match c {
        'n' => '\n',
        't' => '\t',
        'r' => '\r',
        _ => c,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_atoms_and_strings() {
        let forms = Sexp::parse(r#"hello "world" :key #:sym"#);
        assert_eq!(forms.len(), 4);
        assert_eq!(forms[0].as_atom(), Some("hello"));
        assert_eq!(forms[1].as_str(), Some("world"));
        assert_eq!(forms[2].as_atom(), Some(":key"));
        assert_eq!(forms[3].as_atom(), Some("#:sym"));
    }

    #[test]
    fn test_list() {
        let forms = Sexp::parse("(define x 42)");
        assert_eq!(forms.len(), 1);
        let items = forms[0].as_list().unwrap();
        assert_eq!(items[0].as_atom(), Some("define"));
        assert_eq!(items[1].as_atom(), Some("x"));
        assert_eq!(items[2].as_atom(), Some("42"));
    }

    #[test]
    fn test_quote_sugar() {
        let forms = Sexp::parse("'(a b c)");
        assert_eq!(forms.len(), 1);
        let items = forms[0].as_list().unwrap();
        assert_eq!(items[0].as_atom(), Some("quote"));
        let inner = items[1].as_list().unwrap();
        assert_eq!(inner.len(), 3);
    }

    #[test]
    fn test_bracket_list() {
        let forms = Sexp::parse("[a b c]");
        assert_eq!(forms.len(), 1);
        let items = forms[0].as_list().unwrap();
        assert_eq!(items.len(), 3);
    }

    #[test]
    fn test_line_comment() {
        let forms = Sexp::parse("; comment\n(foo)");
        assert_eq!(forms.len(), 1);
        assert!(forms[0].as_list().is_some());
    }

    #[test]
    fn test_block_comment() {
        let forms = Sexp::parse("#| block comment |# (foo)");
        assert_eq!(forms.len(), 1);
    }

    #[test]
    fn test_tagged_list() {
        let forms = Sexp::parse("(define x 1)");
        assert_eq!(
            forms[0].tagged_list("define"),
            Some(&forms[0].as_list().unwrap()[1..])
        );
    }

    #[test]
    fn test_kw_arg() {
        let forms = Sexp::parse("(:version \"1.0\" :name \"pkg\")");
        let items = forms[0].as_list().unwrap();
        let ver = kw_arg(items, ":version").unwrap();
        assert_eq!(ver.as_str(), Some("1.0"));
    }

    #[test]
    fn test_as_text() {
        let forms = Sexp::parse(r#"atom "str""#);
        assert_eq!(forms[0].as_text(), Some("atom"));
        assert_eq!(forms[1].as_text(), Some("str"));
    }
}
