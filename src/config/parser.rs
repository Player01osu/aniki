use std::{str::Chars, path::{PathBuf, Path}};
use anyhow::Result;

use super::Config;

#[derive(Debug, Eq, PartialEq)]
enum TokenKind {
    Ident(String),
    StringLiteral(String),

    // Keywords
    ThumbnailPath,
    DatabasePath,
    VideoPaths,

    Newline,
    OpenBracket,
    CloseBracket,
    Comma,
    Assignment,

    EOF,
    Illegal,
}

#[derive(Debug)]
struct Token {
    kind: TokenKind,
}

#[derive(Debug)]
pub struct ConfigLexer<'a> {
    src: &'a str,
    cursor: Chars<'a>,
}

#[derive(Debug)]
pub enum Node {
    ThumbnailPath(PathBuf),
    DatabasePath(PathBuf),
    VideoPaths(Vec<PathBuf>),
}

fn expect_token(Token { kind }: &Token, expected: TokenKind) -> Result<()> {
    if kind != &expected {
        return Err(anyhow::anyhow!(
            "Unexpected token: \"{kind:?}\" Expected: \"{expected:?}\""
        ));
    }
    Ok(())
}

fn parse_path_array<'a>(lexer: &mut ConfigLexer<'a>) -> Result<Vec<PathBuf>> {
    let mut vec = match lexer.next_token().kind {
        TokenKind::StringLiteral(s) => vec![s.into()],
        kind => return Err(anyhow::anyhow!("Unexpected token: {kind:?}")),
    };

    loop {
        match lexer.next_token().kind {
            TokenKind::Comma => {
                vec.push(match lexer.next_token().kind {
                    TokenKind::StringLiteral(s) => s.into(),
                    kind => return Err(anyhow::anyhow!("Unexpected token: {kind:?}")),
                });
            }
            TokenKind::CloseBracket => break,
            kind => return Err(anyhow::anyhow!("Unexpected token: {kind:?}")),
        }
    }
    Ok(vec)
}

fn cook_string(s: String) -> PathBuf {
    let p = Path::new(&s);
    match p.strip_prefix("~").ok() {
        Some(v) => Path::new(&std::env::var("HOME").unwrap_or(String::from("/"))).join(v),
        None => p.to_path_buf(),
    }
}

fn next_path<'a>(lexer: &mut ConfigLexer<'a>) -> Result<PathBuf> {
    match lexer.next_token().kind {
        TokenKind::StringLiteral(s) => Ok(cook_string(s)),
        kind => Err(anyhow::anyhow!("Unexpected token: {kind:?}")),
    }
}

pub(super) fn next_node<'a>(lexer: &mut ConfigLexer<'a>) -> Result<Option<Node>> {
    match lexer.next_token().kind {
        TokenKind::ThumbnailPath => {
            expect_token(&lexer.next_token(), TokenKind::Assignment)?;
            let path = next_path(lexer)?;

            let next_token = lexer.next_token();
            expect_token(&next_token, TokenKind::Newline).or(expect_token(&next_token, TokenKind::EOF))?;
            Ok(Some(Node::ThumbnailPath(path)))
        }
        TokenKind::DatabasePath => {
            expect_token(&lexer.next_token(), TokenKind::Assignment)?;
            let path = next_path(lexer)?;

            let next_token = lexer.next_token();
            expect_token(&next_token, TokenKind::Newline).or(expect_token(&next_token, TokenKind::EOF))?;
            Ok(Some(Node::DatabasePath(path)))
        }
        TokenKind::VideoPaths => {
            expect_token(&lexer.next_token(), TokenKind::Assignment)?;

            let paths = match lexer.next_token().kind {
                TokenKind::StringLiteral(s) => vec![cook_string(s)],
                TokenKind::OpenBracket => parse_path_array(lexer)?,
                kind => return Err(anyhow::anyhow!("Unexpected token: {kind:?}")),
            };

            let next_token = lexer.next_token();
            expect_token(&next_token, TokenKind::Newline).or(expect_token(&next_token, TokenKind::EOF))?;
            Ok(Some(Node::VideoPaths(paths)))
        }
        TokenKind::EOF => Ok(None),
        kind => Err(anyhow::anyhow!("Unexpected token: {kind:?}")),
    }
}

impl Config {
    pub fn parse(
        config_path: PathBuf,
        thumbnail_path: PathBuf,
        database_path: PathBuf,
        video_paths: Vec<PathBuf>,
    ) -> Self {
        let src = std::fs::read_to_string(config_path).unwrap();
        Self::parse_str(&src, thumbnail_path, database_path, video_paths)
    }

    pub fn parse_str(
        src: &str,
        mut thumbnail_path: PathBuf,
        mut database_path: PathBuf,
        mut video_paths: Vec<PathBuf>,
    ) -> Self {
        let mut lexer = ConfigLexer::new(src);
        while let Some(node) = next_node(&mut lexer).unwrap() {
            match node {
                Node::ThumbnailPath(path) => thumbnail_path = path,
                Node::DatabasePath(path) => database_path = path,
                Node::VideoPaths(paths) => video_paths = paths,
            }
        }

        Self {
            thumbnail_path,
            database_path,
            video_paths,
        }
    }
}

fn is_newline(c: char) -> bool {
    matches!(c, '\n' | '\r' | '\0')
}

fn is_ident(c: char) -> bool {
    c.is_ascii_alphanumeric() | matches!(c, '_')
}

impl<'a> ConfigLexer<'a> {
    pub fn new(src: &'a str) -> Self {
        Self {
            src,
            cursor: src.chars(),
        }
    }

    fn peak(&self) -> char {
        self.cursor.clone().next().unwrap_or('\0')
    }

    fn bump(&mut self) -> char {
        self.cursor.next().unwrap_or('\0')
    }

    fn consume_comment(&mut self) {
        loop {
            if is_newline(self.bump()) {
                break;
            }
        }
    }

    fn consume_ident(&mut self, c: char) -> TokenKind {
        let mut buf = String::from(c);
        while is_ident(self.peak()) {
            buf.push(self.bump());
        }
        match buf.as_str() {
            "thumbnail_path" => TokenKind::ThumbnailPath,
            "database_path" => TokenKind::DatabasePath,
            "video_paths" => TokenKind::VideoPaths,
            _ => TokenKind::Ident(buf),
        }
    }

    fn consume_string_literal(&mut self, quote: char) -> TokenKind {
        let mut buf = String::new();
        loop {
            let c = self.bump();
            if c == quote {
                break;
            }
            buf.push(c);
        }

        TokenKind::StringLiteral(buf)
    }

    fn consume_space(&mut self) {
        while self.peak() == ' ' {
            self.bump();
        }
    }

    fn next_token(&mut self) -> Token {
        let c = match self.cursor.next() {
            Some(c) => c,
            None => {
                return Token {
                    kind: TokenKind::EOF,
                };
            }
        };

        let kind = match c {
            ' ' => {
                self.consume_space();
                return self.next_token();
            }
            '#' => {
                self.consume_comment();
                return self.next_token();
            }
            '[' => TokenKind::OpenBracket,
            ']' => TokenKind::CloseBracket,
            '=' => TokenKind::Assignment,
            ',' => TokenKind::Comma,
            '\n' | '\r' => TokenKind::Newline,
            c @ '"' | c @ '\'' => self.consume_string_literal(c),
            'a'..='z' | 'A'..='Z' => self.consume_ident(c),
            k => {
                eprintln!("Illegal Token: {k}");
                TokenKind::Illegal
            }
        };
        Token { kind }
    }
}

#[test]
fn parser_test_0() {
    let path = "/path";
    let src = format!(r#"thumbnail_path = "{path}""#);
    let base_dir_path = Path::new("/");
    let cfg = {
        let database_path = base_dir_path.join("aniki.db");
        let thumbnail_path = base_dir_path.join("thumbnails");
        let video_paths = vec![];
        Config::parse_str(&src, thumbnail_path, database_path, video_paths)
    };

    let database_path = base_dir_path.join("aniki.db");
    let _thumbnail_path = base_dir_path.join("thumbnails");
    let video_paths = vec![];

    assert_eq!(cfg, Config { thumbnail_path: PathBuf::from(path), database_path, video_paths });
}

#[test]
fn parser_test_1() {
    let path = "/path";
    let src = format!(r#"database_path = '{path}'"#);
    let base_dir_path = Path::new("/");
    let cfg = {
        let database_path = base_dir_path.join("aniki.db");
        let thumbnail_path = base_dir_path.join("thumbnails");
        let video_paths = vec![];
        Config::parse_str(&src, thumbnail_path, database_path, video_paths)
    };

    let _database_path = base_dir_path.join("aniki.db");
    let thumbnail_path = base_dir_path.join("thumbnails");
    let video_paths = vec![];

    assert_eq!(cfg, Config { thumbnail_path, database_path: PathBuf::from(path), video_paths });
}

#[test]
fn parser_test_2() {
    let path = "/path";
    let path_1 = "/path-1";
    let src = format!(r#"video_paths = ["{path}", "{path_1}"]"#);
    let base_dir_path = Path::new("/");
    let cfg = {
        let database_path = base_dir_path.join("aniki.db");
        let thumbnail_path = base_dir_path.join("thumbnails");
        let video_paths = vec![];
        Config::parse_str(&src, thumbnail_path, database_path, video_paths)
    };

    let database_path = base_dir_path.join("aniki.db");
    let thumbnail_path = base_dir_path.join("thumbnails");
    let video_paths = vec![PathBuf::from(path), PathBuf::from(path_1)];

    assert_eq!(cfg, Config { thumbnail_path, database_path, video_paths });
}
