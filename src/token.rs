use std::borrow::Cow;
use std::fmt;
use std::str::FromStr;
use thiserror::Error;
use unic_ucd_ident::{is_xid_continue, is_xid_start};

/// An error encountered during lexing of a Rado source file.
#[derive(Copy, Clone, Debug, Error)]
pub enum LexerError {
  #[error("Unterminated /* block comment */")]
  UnterminatedBlockComment,
  #[error("Numeric literal suffixes are not supported")]
  NumericLiteralSuffix,
  #[error("Unterminated \"string literal\"")]
  UnterminatedStringLiteral,
  #[error("Unrecognized escape sequence character: {0:?}")]
  UnrecognizedEscapeSequence(char),
  #[error("! must be followed by = to make !=")]
  LoneExclamationPoint,
  #[error("Negative zero literal")]
  NegativeZero,
  #[error("Unrecognized character: {0:?}")]
  UnrecognizedCharacter(char),
}

#[derive(Clone, Debug, Error)]
#[error("{:?} is not a keyword", s)]
pub struct LexKwError {
  s: String,
}

/// An error from failing to lex a [Sym].
#[derive(Clone, Debug, Error)]
#[error("{:?} is not a symbol", s)]
pub struct LexSymError {
  s: String,
}

macro_rules! toks {
  {
    $(#[$outer:meta])*
    $v:vis enum $name:ident {
      err $err:ident;
      $($kw:ident <- $spl:expr),*,
    }
  } => {
    $(#[$outer])*
    #[derive(Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Debug)]
    $v enum $name {
      $($kw),*,
    }

    impl FromStr for $name {
      type Err = $err;

      fn from_str(s: &str) -> Result<$name, $err> {
        match s {
          $($spl => Ok($name::$kw)),*,
          _ => Err($err{s: s.into()}),
        }
      }
    }

    impl fmt::Display for $name {
      fn fmt(&self, f : &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
          $($name::$kw => $spl),*,
        })
      }
    }
  };
}

toks! {
  /// "Rado keywords."
  pub enum Kw {
    err LexKwError;
    // Declarations
    Region <- "region",
    Link <- "link",
    Item <- "item",
    Items <- "items",
    Location <- "location",
    Locations <- "locations",
    Fn <- "fn",
    Enum <- "enum",
    Config <- "config",
    Configs <- "configs",
    Configset <- "configset",
    Random <- "random",
    If <- "if",
    Else <- "else",
    Modify <- "modify",
    Override <- "override",

    // Properties
    Requires <- "requires",
    Visible <- "visible",
    Unlock <- "unlock",
    Tag <- "tag",
    Alias <- "alias",
    Provides <- "provides",
    Progressive <- "progressive",
    Val <- "val",
    Max <- "max",
    Consumable <- "consumable",
    Avail <- "avail",
    Infinity <- "infinity",
    Grants <- "grants",
    Count <- "count",
    Start <- "start",

    // Expressions & types not covered above
    Num <- "num",
    Bool <- "bool",
    Then <- "then",
    Match <- "match",
    True <- "true",
    False <- "false",
    Not <- "not",
    And <- "and",
    Or <- "or",
    Min <- "min",
    Sum <- "sum",

    // Miscellaneous
    With <- "with",
    To <- "to",
    From <- "from",
    In <- "in",
    Default <- "default",
  }
}

toks! {
  /// Rado symbol tokens. Each operator is a distinct token, so some tokens are
  /// multiple characters long.
  pub enum Sym {
    err LexSymError;
    // Delimeters
    LParen <- "(",
    RParen <- ")",
    LBrack <- "[",
    RBrack <- "]",
    LBrace <- "{",
    RBrace <- "}",

    // Punctuation
    Semi <- ";",
    Comma <- ",",
    Colon <- ":",
    Dot <- ".",
    Assign <- "=",
    Arrow <- "->",
    DoubleArrow <- "=>",

    // Operators
    Plus <- "+",
    Minus <- "-",
    Star <- "*",
    Slash <- "/",
    Percent <-"%",
    Eq <- "==",
    NEq <- "!=",
    LT <- "<",
    LE <- "<=",
    GT <- ">=",
    GE <- ">",
  }
}

/// The sign of a numeric literal. Zero is considered positive, since the minus
/// sign is not used for zero literals; negative 0 is an error.
#[derive(Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Debug)]
pub enum Sign {
  Positive,
  Negative,
}

impl fmt::Display for Sign {
  /// Display the sign as it renders before a number: nothing if it is
  /// positive, and a minus sign for negative.
  fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
    f.write_str(match self {
      Sign::Positive => "",
      Sign::Negative => "-",
    })
  }
}

/// A Rado token.
#[derive(Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Debug)]
pub enum Tok<'a> {
  /// A keyword.
  Kw(Kw),
  /// A symbol or operator.
  Sym(Sym),
  /// An identifier other than a keyword.
  Ident(Cow<'a, str>),
  /// A numeric literal. Numbers are unparsed
  Num(
    /// The sign of the number.
    Sign,
    /// The whole-number portion of the number (before the '.', if any).
    Cow<'a, str>,
    /// The decimal portion of the number (after the '.').
    Option<Cow<'a, str>>,
  ),
  /// A string literal. The field contains the string with escapes already
  /// procesed.
  String(Cow<'a, str>),
}

impl<'a> Tok<'a> {
  /// Convert any `Cow` strings owned by this token to owned versions, making
  /// copies if needed. After this, calling `to_owned` on them will be a
  /// no-op.
  pub fn into_owned(self) -> Tok<'static> {
    use Tok::*;
    match self {
      Kw(k) => Kw(k),
      Sym(s) => Sym(s),
      Ident(i) => Ident(Cow::Owned(i.into_owned())),
      Num(s, w, d) => Num(
        s,
        Cow::Owned(w.into_owned()),
        d.map(|d| Cow::Owned(d.into_owned())),
      ),
      String(s) => String(Cow::Owned(s.into_owned())),
    }
  }
}

impl<'a> fmt::Display for Tok<'a> {
  fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
    match self {
      Tok::Kw(k) => write!(f, "{}", k),
      Tok::Sym(s) => write!(f, "{}", s),
      Tok::Ident(i) => write!(f, "{}", i),
      Tok::Num(s, w, d) => write!(
        f,
        "{}{}{}{}",
        s,
        w,
        if d.is_some() { "." } else { "" },
        d.as_ref().map_or("", |d| &d),
      ),
      Tok::String(s) => write!(f, "{:?}", s),
    }
  }
}

/// For a string starting on a block comment marker, advance up to the last
/// character of the block comment. It will recurse in order to handle nested
/// comments.
fn skip_block_comment(mut s: &str) -> Result<&str, LexerError> {
  assert!(s.len() >= 2);
  assert!(s.starts_with("/*"));
  s = &s[2..];

  // TODO: This feels kind of bad to search twice, but lazy_static/regex is a
  // lot of work for two two-character search patterns. Also this means every
  // recursion is checking for an EOF, which is pointless.
  loop {
    let end = s
      .find("*/")
      .ok_or_else(|| LexerError::UnterminatedBlockComment)?;
    match s.find("/*") {
      Some(inner) if inner < end => s = &skip_block_comment(&s[inner..])?,
      _ => break Ok(&s[end + 2..]),
    }
  }
}

/// Lex a numeric literal.
#[allow(clippy::type_complexity, clippy::many_single_char_names)]
fn lex_num_lit(mut s: &str) -> Result<(Cow<'_, str>, Option<Cow<'_, str>>, &str), LexerError> {
  let i = s
    .find(|c: char| !c.is_ascii_digit())
    .unwrap_or_else(|| s.len());
  let (w, mut f) = (s[0..i].into(), None);
  s = &s[i..];

  let mut r = s.chars();
  if r.next() == Some('.') && r.next().map_or(false, |c| c.is_ascii_digit()) {
    s = &s[1..];
    let i = s
      .find(|c: char| !c.is_ascii_digit())
      .unwrap_or_else(|| s.len());
    f = Some(s[0..i].into());
    s = &s[i..];
  }
  if s
    .chars()
    .next()
    .map_or(false, |c| c == '_' || is_xid_start(c) || is_xid_continue(c))
  {
    return Err(LexerError::NumericLiteralSuffix);
  }
  Ok((w, f, s))
}

/// Lex a string literal, and return the contents (with escapes processed) in the first position,
/// and the remainder of the source in the second. s is expected to already have had the opening quote
/// removed.
fn lex_string_lit(mut s: &str) -> Result<(Cow<'_, str>, &str), LexerError> {
  // Easy case: there is no escape sequence, so we can just borrow the
  // contents directly.
  let escape = s.find('\\').unwrap_or_else(|| s.len());
  let quote = s
    .find('\"')
    .ok_or_else(|| LexerError::UnterminatedStringLiteral)?;
  if quote < escape {
    return Ok((s[0..quote].into(), &s[quote + 1..]));
  }

  let mut l = String::new();
  while let Some(escape) = s.find('\\') {
    l += &s[0..escape];
    s = &s[escape + 1..];
    match s.chars().next() {
      None => return Err(LexerError::UnterminatedStringLiteral),
      Some('"') => l += "\"",
      Some('\\') => l += "\\",
      Some('n') => l += "\n",
      Some('r') => l += "\r",
      Some('t') => l += "\t",
      Some(e) => return Err(LexerError::UnrecognizedEscapeSequence(e)),
    }
    // Any escape sequence we actually accept is 1 ASCII character long.
    s = &s[1..];
  }
  let quote = s
    .find('\"')
    .ok_or_else(|| LexerError::UnterminatedStringLiteral)?;
  l += &s[0..quote];
  Ok((l.into(), &s[quote + 1..]))
}

/// Lex a string into a token vector. An error occurs if the string is not made of legal tokens.
pub fn lex<'a>(mut s: &'a str) -> Result<Vec<Tok<'a>>, LexerError> {
  let mut toks = Vec::new();
  while let Some(c) = s.chars().next() {
    let rest = &s[c.len_utf8()..];
    match c {
      '(' => {
        toks.push(Tok::Sym(Sym::LParen));
        s = rest;
      }
      ')' => {
        toks.push(Tok::Sym(Sym::RParen));
        s = rest;
      }
      '[' => {
        toks.push(Tok::Sym(Sym::LBrack));
        s = rest;
      }
      ']' => {
        toks.push(Tok::Sym(Sym::RBrack));
        s = rest;
      }
      '{' => {
        toks.push(Tok::Sym(Sym::LBrace));
        s = rest;
      }
      '}' => {
        toks.push(Tok::Sym(Sym::RBrace));
        s = rest;
      }
      ';' => {
        toks.push(Tok::Sym(Sym::Semi));
        s = rest;
      }
      ',' => {
        toks.push(Tok::Sym(Sym::Comma));
        s = rest;
      }
      ':' => {
        toks.push(Tok::Sym(Sym::Colon));
        s = rest;
      }
      '.' => {
        toks.push(Tok::Sym(Sym::Dot));
        s = rest;
      }
      '+' => {
        toks.push(Tok::Sym(Sym::Plus));
        s = rest;
      }
      '*' => {
        toks.push(Tok::Sym(Sym::Star));
        s = rest;
      }
      '%' => {
        toks.push(Tok::Sym(Sym::Percent));
        s = rest;
      }
      '/' => match rest.chars().next() {
        Some('/') => {
          // If we don't find \n, we set i to s.len()-1 so that when we add 1 on the next
          // line, we end up right at the end of the string.
          let i = s.find('\n').unwrap_or(s.len() - 1);
          s = &s[i + 1..];
        }
        Some('*') => s = skip_block_comment(s)?,
        _ => {
          toks.push(Tok::Sym(Sym::Slash));
          s = rest;
        }
      },
      '!' => {
        if rest.starts_with('=') {
          toks.push(Tok::Sym(Sym::NEq));
          s = &s[2..];
        } else {
          return Err(LexerError::LoneExclamationPoint);
        }
      }
      '=' => match rest.chars().next() {
        Some('=') => {
          toks.push(Tok::Sym(Sym::Eq));
          s = &s[2..];
        }
        Some('>') => {
          toks.push(Tok::Sym(Sym::DoubleArrow));
          s = &s[2..];
        }
        _ => {
          toks.push(Tok::Sym(Sym::Assign));
          s = rest;
        }
      },
      '>' => {
        if rest.starts_with('=') {
          toks.push(Tok::Sym(Sym::GE));
          s = &s[2..];
        } else {
          toks.push(Tok::Sym(Sym::GT));
          s = rest;
        }
      }
      '<' => {
        if rest.starts_with('=') {
          toks.push(Tok::Sym(Sym::LE));
          s = &s[2..];
        } else {
          toks.push(Tok::Sym(Sym::LT));
          s = rest;
        }
      }
      '-' => match rest.chars().next() {
        Some('>') => {
          toks.push(Tok::Sym(Sym::Arrow));
          s = &s[2..];
        }
        Some(c) if c.is_ascii_digit() => {
          let (w, f, s_) = lex_num_lit(rest)?;
          if w.chars().all(|c| c == '0')
            && f.as_ref().unwrap_or(&"".into()).chars().all(|c| c == '0')
          {
            return Err(LexerError::NegativeZero);
          }
          toks.push(Tok::Num(Sign::Negative, w, f));
          s = s_;
        }
        _ => {
          toks.push(Tok::Sym(Sym::Minus));
          s = rest;
        }
      },
      c if c.is_ascii_digit() => {
        let (w, f, s_) = lex_num_lit(s)?;
        toks.push(Tok::Num(Sign::Positive, w, f));
        s = s_;
      }
      c if c == '_' || is_xid_start(c) => {
        let i = s
          .find(|c: char| c != '_' && !is_xid_continue(c))
          .unwrap_or_else(|| s.len());
        let ident = &s[0..i];
        s = &s[i..];
        if let Ok(k) = ident.parse() {
          toks.push(Tok::Kw(k));
        } else {
          toks.push(Tok::Ident(ident.into()));
        }
      }
      '"' => {
        let (l, s_) = lex_string_lit(rest)?;
        toks.push(Tok::String(l));
        s = s_;
      }
      c if c.is_ascii_whitespace() => s = rest,
      _ => return Err(LexerError::UnrecognizedCharacter(c)),
    }
  }
  Ok(toks)
}

// TODO: Get a better testing framework, even if just Go-style table tests.
#[cfg(test)]
mod tests {
  use super::*;
  use proptest::{proptest, proptest_helper};

  #[test]
  fn kws_parse() {
    assert_eq!(Kw::Progressive, "progressive".parse().unwrap());
    assert_eq!(Kw::Enum, "enum".parse().unwrap());
    assert_eq!(Kw::To, "to".parse().unwrap());
    assert_eq!(Kw::Modify, "modify".parse().unwrap());
  }

  #[test]
  fn bad_kws_fail_parse() {
    assert!("foobar".parse::<Kw>().is_err());
    assert!("Requires".parse::<Kw>().is_err());
    assert!("".parse::<Kw>().is_err());
    assert!("samus".parse::<Kw>().is_err());
  }

  #[test]
  fn kws_display() {
    assert_eq!("alias", format!("{}", Kw::Alias));
    assert_eq!("link", format!("{}", Kw::Link));
    assert_eq!("items", format!("{}", Kw::Items));
    assert_eq!("in", format!("{}", Kw::In));
  }

  #[test]
  fn syms_parse() {
    assert_eq!(Sym::Plus, "+".parse().unwrap());
    assert_eq!(Sym::Dot, ".".parse().unwrap());
    assert_eq!(Sym::RBrace, "}".parse().unwrap());
    assert_eq!(Sym::DoubleArrow, "=>".parse().unwrap());
  }

  #[test]
  fn bad_syms_fail_parse() {
    assert!("\"".parse::<Sym>().is_err());
    assert!("".parse::<Sym>().is_err());
    assert!("++".parse::<Sym>().is_err());
  }

  #[test]
  fn toks_display() {
    assert_eq!("<=", format!("{}", Sym::LE));
    assert_eq!(")", format!("{}", Sym::RParen));
    assert_eq!("*", format!("{}", Sym::Star));
  }

  #[test]
  fn lex_syms() {
    use self::Sym::*;
    use self::Tok::*;

    let str = "=======";
    let toks = vec![Sym(Eq), Sym(Eq), Sym(Eq), Sym(Assign)];
    assert_eq!(toks, lex(str).unwrap());

    let str = "===>>>=!==";
    let toks = vec![
      Sym(Eq),
      Sym(DoubleArrow),
      Sym(GT),
      Sym(GE),
      Sym(NEq),
      Sym(Assign),
    ];
    assert_eq!(toks, lex(str).unwrap());

    let str = "--->+<<==";
    let toks = vec![
      Sym(Minus),
      Sym(Minus),
      Sym(Arrow),
      Sym(Plus),
      Sym(LT),
      Sym(LE),
      Sym(Assign),
    ];
    assert_eq!(toks, lex(str).unwrap());

    let str = "*+-/%.;:,{}()[]";
    let toks = vec![
      Sym(Star),
      Sym(Plus),
      Sym(Minus),
      Sym(Slash),
      Sym(Percent),
      Sym(Dot),
      Sym(Semi),
      Sym(Colon),
      Sym(Comma),
      Sym(LBrace),
      Sym(RBrace),
      Sym(LParen),
      Sym(RParen),
      Sym(LBrack),
      Sym(RBrack),
    ];
    assert_eq!(toks, lex(str).unwrap());

    let str = "- > = > = < = =";
    let toks = vec![
      Sym(Minus),
      Sym(GT),
      Sym(Assign),
      Sym(GT),
      Sym(Assign),
      Sym(LT),
      Sym(Assign),
      Sym(Assign),
    ];
    assert_eq!(toks, lex(str).unwrap());
  }

  #[test]
  fn lex_nums() {
    use self::Sym::*;
    use self::Tok::*;

    let str = "0";
    let toks = vec![Num(Sign::Positive, "0".into(), None)];
    assert_eq!(toks, lex(str).unwrap());

    let str = "1234567890";
    let toks = vec![Num(Sign::Positive, "1234567890".into(), None)];
    assert_eq!(toks, lex(str).unwrap());

    let str = "0.1";
    let toks = vec![Num(Sign::Positive, "0".into(), Some("1".into()))];
    assert_eq!(toks, lex(str).unwrap());

    let str = "99999999999999999999.00000000000000000000";
    let toks = vec![Num(
      Sign::Positive,
      "99999999999999999999".into(),
      Some("00000000000000000000".into()),
    )];
    assert_eq!(toks, lex(str).unwrap());

    let str = "1.1.1";
    let toks = vec![
      Num(Sign::Positive, "1".into(), Some("1".into())),
      Sym(Dot),
      Num(Sign::Positive, "1".into(), None),
    ];
    assert_eq!(toks, lex(str).unwrap());

    let str = ".1";
    let toks = vec![Sym(Dot), Num(Sign::Positive, "1".into(), None)];
    assert_eq!(toks, lex(str).unwrap());

    let str = "1 .1";
    let toks = vec![
      Num(Sign::Positive, "1".into(), None),
      Sym(Dot),
      Num(Sign::Positive, "1".into(), None),
    ];
    assert_eq!(toks, lex(str).unwrap());

    let str = "-1";
    let toks = vec![Num(Sign::Negative, "1".into(), None)];
    assert_eq!(toks, lex(str).unwrap());

    let str = "-2.2";
    let toks = vec![Num(Sign::Negative, "2".into(), Some("2".into()))];
    assert_eq!(toks, lex(str).unwrap());

    let str = "-0.1";
    let toks = vec![Num(Sign::Negative, "0".into(), Some("1".into()))];
    assert_eq!(toks, lex(str).unwrap());

    let str = "0.-1";
    let toks = vec![
      Num(Sign::Positive, "0".into(), None),
      Sym(Dot),
      Num(Sign::Negative, "1".into(), None),
    ];
    assert_eq!(toks, lex(str).unwrap());
  }

  #[test]
  fn lex_idents_kws() {
    use self::Kw::*;
    use self::Tok::*;

    let str = "a";
    let toks = vec![Ident("a".into())];
    assert_eq!(toks, lex(str).unwrap());

    let str = "A";
    let toks = vec![Ident("A".into())];
    assert_eq!(toks, lex(str).unwrap());

    let str = "z1";
    let toks = vec![Ident("z1".into())];
    assert_eq!(toks, lex(str).unwrap());

    let str = "_";
    let toks = vec![Ident("_".into())];
    assert_eq!(toks, lex(str).unwrap());

    let str = "the_quick_brown_fox_jumps_over_the_1234567890_lazy_dogs";
    let toks = vec![Ident(
      "the_quick_brown_fox_jumps_over_the_1234567890_lazy_dogs".into(),
    )];
    assert_eq!(toks, lex(str).unwrap());

    let str = "a b";
    let toks = vec![Ident("a".into()), Ident("b".into())];
    assert_eq!(toks, lex(str).unwrap());

    let str = "if";
    let toks = vec![Kw(If)];
    assert_eq!(toks, lex(str).unwrap());

    let str = "_if";
    let toks = vec![Ident("_if".into())];
    assert_eq!(toks, lex(str).unwrap());

    let str = "if9";
    let toks = vec![Ident("if9".into())];
    assert_eq!(toks, lex(str).unwrap());

    let str = "if than else";
    let toks = vec![Kw(If), Ident("than".into()), Kw(Else)];
    assert_eq!(toks, lex(str).unwrap());
  }

  #[test]
  fn lex_idents_whitespace() {
    use Tok::*;

    let str = "  \t\n  \r    ";
    let toks: Vec<Tok> = vec![];
    assert_eq!(toks, lex(str).unwrap());

    let str = "s\tv";
    let toks = vec![Ident("s".into()), Ident("v".into())];
    assert_eq!(toks, lex(str).unwrap());

    let str = "s\n\r\nq";
    let toks = vec![Ident("s".into()), Ident("q".into())];
    assert_eq!(toks, lex(str).unwrap());
  }

  #[test]
  fn lex_comments() {
    use self::Sym::*;
    use self::Tok::*;

    let str = "foo//bar\nbaz";
    let toks = vec![Ident("foo".into()), Ident("baz".into())];
    assert_eq!(toks, lex(str).unwrap());

    let str = "foo//bar";
    let toks = vec![Ident("foo".into())];
    assert_eq!(toks, lex(str).unwrap());

    let str = "//foo\n///bar\n//\n/\n/baz";
    let toks = vec![Sym(Slash), Sym(Slash), Ident("baz".into())];
    assert_eq!(toks, lex(str).unwrap());

    let str = "foo/*bar*/baz";
    let toks = vec![Ident("foo".into()), Ident("baz".into())];
    assert_eq!(toks, lex(str).unwrap());

    let str = "foo/*/ */bar\nbaz";
    let toks = vec![
      Ident("foo".into()),
      Ident("bar".into()),
      Ident("baz".into()),
    ];
    assert_eq!(toks, lex(str).unwrap());

    let str = "foo /* /* */ */ bar";
    let toks = vec![Ident("foo".into()), Ident("bar".into())];
    assert_eq!(toks, lex(str).unwrap());

    let str = "/**/";
    let toks: Vec<Tok> = vec![];
    assert_eq!(toks, lex(str).unwrap());

    let str = "/***/";
    let toks: Vec<Tok> = vec![];
    assert_eq!(toks, lex(str).unwrap());

    let str = "/*********/";
    let toks: Vec<Tok> = vec![];
    assert_eq!(toks, lex(str).unwrap());

    let str = "/*/ bar */";
    let toks: Vec<Tok> = vec![];
    assert_eq!(toks, lex(str).unwrap());

    let str = "/* */ */";
    let toks = vec![Sym(Star), Sym(Slash)];
    assert_eq!(toks, lex(str).unwrap());

    let str = "///*\n*/";
    let toks = vec![Sym(Star), Sym(Slash)];
    assert_eq!(toks, lex(str).unwrap());

    let str = "foo/*/*/*/*/**/*/*/*/*/";
    let toks = vec![Ident("foo".into())];
    assert_eq!(toks, lex(str).unwrap());

    let str = "/* /* */ /* */ */";
    let toks: Vec<Tok> = vec![];
    assert_eq!(toks, lex(str).unwrap());

    let str = "/* // */\n*/";
    let toks = vec![Sym(Star), Sym(Slash)];
    assert_eq!(toks, lex(str).unwrap());
  }

  #[test]
  fn lex_string_literals() {
    use Tok::*;

    let str = "\"\"";
    let toks = vec![String("".into())];
    assert_eq!(toks, lex(str).unwrap());

    let str = "\"abcd\"";
    let toks = vec![String("abcd".into())];
    assert_eq!(toks, lex(str).unwrap());

    let str = "\"\"\"\"";
    let toks = vec![String("".into()), String("".into())];
    assert_eq!(toks, lex(str).unwrap());

    let str = "\"\\\"\"";
    let toks = vec![String("\"".into())];
    assert_eq!(toks, lex(str).unwrap());

    let str = "\"\\\\\"";
    let toks = vec![String("\\".into())];
    assert_eq!(toks, lex(str).unwrap());

    let str = "\"a\\nb\\rc\\td\"";
    let toks = vec![String("a\nb\rc\td".into())];
    assert_eq!(toks, lex(str).unwrap());

    let str = "\"a b c \"";
    let toks = vec![String("a b c ".into())];
    assert_eq!(toks, lex(str).unwrap());

    let str = "a\"\"b";
    let toks = vec![Ident("a".into()), String("".into()), Ident("b".into())];
    assert_eq!(toks, lex(str).unwrap());
  }

  #[test]
  fn lex_errors() {
    let str = "!";
    assert!(lex(str).is_err());

    let str = "\0";
    assert!(lex(str).is_err());

    let str = "\x12";
    assert!(lex(str).is_err());

    let str = "=!";
    assert!(lex(str).is_err());

    let str = "\u{ffef}hi";
    assert!(lex(str).is_err());

    let str = "23l";
    assert!(lex(str).is_err());

    let str = "123é";
    assert!(lex(str).is_err());

    // A character that is XID_Continue but not XID_Start
    let str = "\u{00B7}";
    assert!(lex(str).is_err());
  }

  #[test]
  fn lex_unicode_idents() {
    use Tok::*;

    // Thanks to Principia, a KSP mod, for some sample Unicode identifiers.
    let str = "é";
    let toks = vec![Ident("é".into())];
    assert_eq!(toks, lex(str).unwrap());

    let str = "DormandالمكاوىPrince1986RKN434FM";
    let toks = vec![Ident("DormandالمكاوىPrince1986RKN434FM".into())];
    assert_eq!(toks, lex(str).unwrap());

    let str = "ЧебышёвSeries";
    let toks = vec![Ident("ЧебышёвSeries".into())];
    assert_eq!(toks, lex(str).unwrap());

    let str = "名前";
    let toks = vec![Ident("名前".into())];
    assert_eq!(toks, lex(str).unwrap());
  }

  proptest! {
      #[test]
      fn always_valid(ref s in ".*") {
          let _ = lex(s);
      }
  }
}
