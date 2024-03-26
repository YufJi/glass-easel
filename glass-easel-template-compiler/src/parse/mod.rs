use std::ops::Range;

use compact_str::CompactString;

use super::binding_map;
pub use tag::Template;

#[cfg(test)]
macro_rules! case {
    ($src:expr, $expect:expr $(, $msg:expr, $range:expr)*) => {
        {
            use crate::stringify::Stringify;
            let src: &str = $src;
            let expect: &str = $expect;

            // parse and check warnings
            let (template, ps) = $crate::parse::parse("TEST", src);
            let mut warnings = ps.warnings();
            $(
                let next = warnings.next();
                assert!(next.is_some());
                let err = next.unwrap();
                assert_eq!(err.kind, $msg);
                assert_eq!(err.location.start.utf16_col..err.location.end.utf16_col, $range);
            )*
            assert_eq!(warnings.next(), None);

            // check stringify result
            let mut stringifier = crate::stringify::Stringifier::new(String::new(), "test", src);
            template.stringify_write(&mut stringifier).unwrap();
            let (stringify_result, _sourcemap) = stringifier.finish();
            assert_eq!(stringify_result.as_str(), expect);

            // re-parse and then stringify
            let (template, ps) = $crate::parse::parse("TEST", expect);
            assert_eq!(ps.warnings().filter(|x| x.kind.level() > crate::parse::ParseErrorLevel::Note).next(), None);
            let mut stringifier = crate::stringify::Stringifier::new(String::new(), "test", src);
            template.stringify_write(&mut stringifier).unwrap();
            assert_eq!(stringifier.finish().0.as_str(), expect);
        }
    };
}

pub mod tag;
pub mod expr;

pub trait TemplateStructure {
    fn location(&self) -> Range<Position>;

    fn location_start(&self) -> Position {
        self.location().start
    }

    fn location_end(&self) -> Position {
        self.location().end
    }
}

/// Some meta information of the parsing.
pub struct ParseState<'s> {
    path: String,
    whole_str: &'s str,
    cur_index: usize,
    line: u32,
    utf16_col: u32,
    scopes: Vec<(CompactString, Range<Position>)>,
    inside_dynamic_tree: usize,
    auto_skip_whitespace: bool,
    warnings: Vec<ParseError>,
}

impl<'s> ParseState<'s> {
    fn new(path: &str, content: &'s str) -> Self {
        let s = content;
        let s = if s.len() >= u32::MAX as usize {
            log::error!("Source code too long. Truncated to `u32::MAX - 1` .");
            &s[..(u32::MAX as usize - 1)]
        } else {
            s
        };
        Self {
            path: path.to_string(),
            whole_str: s,
            cur_index: 0,
            line: 1,
            utf16_col: 0,
            scopes: vec![],
            inside_dynamic_tree: 0,
            auto_skip_whitespace: false,
            warnings: vec![],
        }
    }

    fn add_warning(&mut self, kind: ParseErrorKind, location: Range<Position>) {
        self.warnings.push(ParseError { path: self.path.to_string(), kind, location })
    }

    fn add_warning_at_current_position(&mut self, kind: ParseErrorKind) {
        let pos = self.position();
        self.add_warning(kind, pos..pos)
    }

    /// List warnings.
    pub fn warnings(&self) -> impl Iterator<Item = &ParseError> {
        self.warnings.iter()
    }

    /// Extract and then clear all warnings.
    pub fn take_warnings(&mut self) -> Vec<ParseError> {
        std::mem::replace(&mut self.warnings, vec![])
    }

    fn cur_str(&self) -> &'s str {
        &self.whole_str[self.cur_index..]
    }

    fn ended(&self) -> bool {
        self.cur_str().len() == 0
    }

    fn parse_on_auto_whitespace<T>(&mut self, f: impl FnOnce(&mut Self) -> T) -> T {
        let prev = self.auto_skip_whitespace;
        self.auto_skip_whitespace = true;
        let ret = f(self);
        self.auto_skip_whitespace = prev;
        ret
    }

    fn parse_off_auto_whitespace<T>(&mut self, f: impl FnOnce(&mut Self) -> T) -> T {
        let prev = self.auto_skip_whitespace;
        self.auto_skip_whitespace = false;
        let ret = f(self);
        self.auto_skip_whitespace = prev;
        ret
    }

    fn try_parse<T>(&mut self, f: impl FnOnce(&mut Self) -> Option<T>) -> Option<T> {
        let prev = self.cur_index;
        let prev_line = self.line;
        let prev_utf16_col = self.utf16_col;
        let ret = f(self);
        if ret.is_none() {
            self.cur_index = prev;
            self.line = prev_line;
            self.utf16_col = prev_utf16_col;
        }
        ret
    }

    fn skip_bytes(&mut self, count: usize) {
        let skipped = &self.cur_str()[..count];
        self.cur_index += count;
        let line_wrap_count = skipped.as_bytes().into_iter().filter(|x| **x == b'\n').count();
        self.line += line_wrap_count as u32;
        if line_wrap_count > 0 {
            let last_line_start = skipped.rfind('\n').unwrap() + 1;
            self.utf16_col = skipped[last_line_start..].encode_utf16().count() as u32;
        } else {
            self.utf16_col += skipped.encode_utf16().count() as u32;
        }
    }

    fn skip_until_before(&mut self, until: &str) -> Option<&'s str> {
        let s = self.cur_str();
        if let Some(index) = s.find(until) {
            let ret = &s[..index];
            self.skip_bytes(index);
            Some(ret)
        } else {
            self.skip_bytes(s.len());
            None
        }
    }

    fn skip_until_after(&mut self, until: &str) -> Option<&'s str> {
        let ret = self.skip_until_before(until);
        if ret.is_some() {
            self.skip_bytes(until.len());
        }
        ret
    }

    fn peek_chars(&mut self) -> impl 's + Iterator<Item = char> {
        if self.auto_skip_whitespace { self.skip_whitespace(); }
        self.cur_str().chars()
    }

    fn peek_n<const N: usize>(&mut self) -> Option<[char; N]> {
        let mut ret: [char; N] = ['\x00'; N];
        let mut iter = self.peek_chars();
        for i in 0..N {
            ret[i] = iter.next()?;
        }
        Some(ret)
    }

    fn peek<const I: usize>(&mut self) -> Option<char> {
        let mut iter = self.peek_chars();
        for _ in 0..I {
            iter.next()?;
        }
        iter.next()
    }

    fn peek_str(&mut self, s: &str) -> bool {
        if self.auto_skip_whitespace { self.skip_whitespace(); }
        self.cur_str().starts_with(s)
    }

    fn consume_str_except_followed<const N: usize>(&mut self, s: &str, excepts: [&str; N]) -> Option<Range<Position>> {
        if !self.peek_str(s) {
            return None;
        }
        let s_followed = &self.cur_str()[s.len()..];
        for except in excepts {
            if s_followed.starts_with(except) {
                return None;
            }
        }
        let start = self.position();
        self.skip_bytes(s.len());
        let end = self.position();
        Some(start..end)
    }

    fn consume_str_except_followed_char(&mut self, s: &str, reject_followed: impl FnOnce(char) -> bool) -> Option<Range<Position>> {
        if !self.peek_str(s) {
            return None;
        }
        let s_followed = &self.cur_str()[s.len()..];
        match s_followed.chars().next() {
            None => {}
            Some(ch) => {
                if reject_followed(ch) {
                    return None;
                }
            }
        }
        let start = self.position();
        self.skip_bytes(s.len());
        let end = self.position();
        Some(start..end)
    }

    fn consume_str(&mut self, s: &str) -> Option<Range<Position>> {
        self.consume_str_except_followed(s, [])
    }

    fn next_char_as_str(&mut self) -> &'s str {
        let s = self.cur_str();
        if s.len() > 0 {
            let mut i = 0;
            loop {
                i += 1;
                if s.is_char_boundary(i) {
                    break;
                }
            }
            let ret = &s[..i];
            self.skip_bytes(i);
            ret
        } else {
            ""
        }
    }

    fn next(&mut self) -> Option<char> {
        if self.auto_skip_whitespace { self.skip_whitespace(); }
        let mut i = self.cur_str().char_indices();
        let (_, ret) = i.next()?;
        self.cur_index += match i.next() {
            Some((p, _)) => p,
            None => self.cur_str().len(),
        };
        if ret == '\n' {
            self.line += 1;
            self.utf16_col = 0;
        } else {
            self.utf16_col += ret.encode_utf16(&mut [0; 2]).len() as u32;
        }
        Some(ret)
    }

    fn skip_whitespace(&mut self) -> Option<Range<Position>> {
        let mut start_pos = None;
        let s = self.cur_str();
        let mut i = s.char_indices();
        self.cur_index += loop {
            let Some((index, c)) = i.next() else {
                break s.len();
            };
            if !char::is_whitespace(c) {
                break index;
            }
            if start_pos.is_none() {
                start_pos = Some(self.position());
            }
            if c == '\n' {
                self.line += 1;
                self.utf16_col = 0;
            } else {
                self.utf16_col += c.encode_utf16(&mut [0; 2]).len() as u32;
            }
        };
        start_pos.map(|x| x..self.position())
    }

    fn code_slice(&self, range: Range<usize>) -> &'s str {
        &self.whole_str[range]
    }

    fn cur_index(&self) -> usize {
        self.cur_index as usize
    }

    fn position(&self) -> Position {
        Position {
            line: self.line,
            utf16_col: self.utf16_col,
        }
    }
}

pub fn parse<'s>(path: &str, source: &'s str) -> (tag::Template, ParseState<'s>) {
    let mut state = ParseState::new(path, source);
    let template = tag::Template::parse(&mut state);
    (template, state)
}

/// A location in source code.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Position {
    pub line: u32,
    pub utf16_col: u32,
}

impl Position {
    /// Get the line-column offsets (in UTF-16) in the source code.
    pub fn line_col_utf16<'s>(&self) -> (usize, usize) {
        (self.line as usize, self.utf16_col as usize)
    }
}

/// Template parsing error object.
#[derive(Debug, Clone, PartialEq)]
pub struct ParseError {
    pub path: String,
    pub kind: ParseErrorKind,
    pub location: Range<Position>,
}

impl std::fmt::Display for ParseError {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        write!(
            f,
            "template parsing error at {}:{}:{}-{}:{}: {}",
            self.path,
            self.location.start.line + 1,
            self.location.start.utf16_col + 1,
            self.location.end.line + 1,
            self.location.end.utf16_col + 1,
            self.kind,
        )
    }
}

impl std::error::Error for ParseError {}

impl ParseError {
    /// The level of the error.
    pub fn level(&self) -> ParseErrorLevel {
        self.kind.level()
    }

    /// An error code.
    pub fn code(&self) -> u32 {
        self.kind.clone() as u32
    }

    /// Whether the error prevent a success compilation.
    pub fn prevent_success(&self) -> bool {
        self.level() >= ParseErrorLevel::Error
    }
}

#[derive(Clone, PartialEq, Eq)]
pub enum ParseErrorKind {
    UnexpectedCharacter = 0x10001,
    UnexpectedExpressionCharacter,
    UnrecognizedTag,
    MissingExpressionEnd,
    IllegalEntity,
    IncompleteTag,
    MissingEndTag,
    IllegalNamePrefix,
    InvalidAttributePrefix,
    InvalidAttributeName,
    InvalidAttributeValue,
    InvalidAttribute,
    DuplicatedAttribute,
    DuplicatedName,
    AvoidUppercaseLetters,
    UnexpectedWhitespace,
    MissingAttributeValue,
    DataBindingNotAllowed,
    InvalidIdentifier,
    InvalidScopeName,
    ChildNodesNotAllowed,
    IllegalEscapeSequence,
    IncompleteConditionExpression,
    UnmatchedBracket,
    UnmatchedParenthesis,
    MissingModuleName,
    MissingSourcePath,
    UnsupportedSyntax,
}

impl ParseErrorKind {
    fn static_message(&self) -> &'static str {
        match self {
            Self::UnexpectedCharacter => "unexpected character",
            Self::UnexpectedExpressionCharacter => "unexpected character inside expression",
            Self::UnrecognizedTag => "unrecognized tag",
            Self::MissingExpressionEnd => "missing expression end",
            Self::IllegalEntity => "illegal entity",
            Self::IncompleteTag => "incomplete tag",
            Self::MissingEndTag => "missing end tag",
            Self::IllegalNamePrefix => "illegal name prefix",
            Self::InvalidAttributePrefix => "invalid attribute prefix",
            Self::InvalidAttributeName => "invalid attribute name",
            Self::InvalidAttributeValue => "invalid attribute value",
            Self::InvalidAttribute => "invalid attribute",
            Self::DuplicatedAttribute => "duplicated attribute",
            Self::DuplicatedName => "duplicated name",
            Self::AvoidUppercaseLetters => "avoid uppercase letters",
            Self::UnexpectedWhitespace => "unexpected whitespace",
            Self::MissingAttributeValue => "missing attribute value",
            Self::DataBindingNotAllowed => "data bindings are not allowed for this attribute",
            Self::InvalidIdentifier => "not a valid identifier",
            Self::InvalidScopeName => "not a valid identifier as scope name",
            Self::ChildNodesNotAllowed => "child nodes are not allowed for this element",
            Self::IllegalEscapeSequence => "illegal escape sequence",
            Self::IncompleteConditionExpression => "incomplete condition expression",
            Self::UnmatchedBracket => "unmatched bracket",
            Self::UnmatchedParenthesis => "unmatched parenthesis",
            Self::MissingModuleName => "missing module name",
            Self::MissingSourcePath => "missing source path",
            Self::UnsupportedSyntax => "this syntax has not been supported yet",
        }
    }

    pub fn level(&self) -> ParseErrorLevel {
        match self {
            Self::UnexpectedCharacter => ParseErrorLevel::Fatal,
            Self::UnexpectedExpressionCharacter => ParseErrorLevel::Fatal,
            Self::UnrecognizedTag => ParseErrorLevel::Warn,
            Self::MissingExpressionEnd => ParseErrorLevel::Fatal,
            Self::IllegalEntity => ParseErrorLevel::Error,
            Self::IncompleteTag => ParseErrorLevel::Fatal,
            Self::MissingEndTag => ParseErrorLevel::Error,
            Self::IllegalNamePrefix => ParseErrorLevel::Warn,
            Self::InvalidAttributePrefix => ParseErrorLevel::Warn,
            Self::InvalidAttributeName => ParseErrorLevel::Warn,
            Self::InvalidAttributeValue => ParseErrorLevel::Note,
            Self::InvalidAttribute => ParseErrorLevel::Warn,
            Self::DuplicatedAttribute => ParseErrorLevel::Warn,
            Self::DuplicatedName => ParseErrorLevel::Note,
            Self::AvoidUppercaseLetters => ParseErrorLevel::Warn,
            Self::UnexpectedWhitespace => ParseErrorLevel::Note,
            Self::MissingAttributeValue => ParseErrorLevel::Error,
            Self::DataBindingNotAllowed => ParseErrorLevel::Note,
            Self::InvalidIdentifier => ParseErrorLevel::Fatal,
            Self::InvalidScopeName => ParseErrorLevel::Note,
            Self::ChildNodesNotAllowed => ParseErrorLevel::Error,
            Self::IllegalEscapeSequence => ParseErrorLevel::Error,
            Self::IncompleteConditionExpression => ParseErrorLevel::Fatal,
            Self::UnmatchedBracket => ParseErrorLevel::Fatal,
            Self::UnmatchedParenthesis => ParseErrorLevel::Fatal,
            Self::MissingModuleName => ParseErrorLevel::Error,
            Self::MissingSourcePath => ParseErrorLevel::Error,
            Self::UnsupportedSyntax => ParseErrorLevel::Error,
        }
    }
}

impl std::fmt::Debug for ParseErrorKind {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        write!(f, "{:?}", self.static_message())
    }
}

impl std::fmt::Display for ParseErrorKind {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        write!(f, "{}", self.static_message())
    }
}

#[derive(Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum ParseErrorLevel {
    /// Likely to be an mistake and should be noticed.
    ///
    /// The generator may generate code that contains this kind of mistakes.
    Note = 1,
    /// Should be a mistake but the compiler can guess a good way to generate proper code.
    Warn,
    /// An error that prevents a successful compilation, but can still continue to find more errors.
    Error,
    /// A very serious error that can cause continuous compiling issues, such as miss matched braces.
    Fatal,
}
