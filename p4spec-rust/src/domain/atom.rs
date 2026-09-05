//! Atoms

use std::{error::Error, fmt};

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Atom {
    /// Concrete object word such as `INT`
    Keyword(String),
    /// Silent meta case label such as `_NUM`
    Tag(String),
    /// Concrete operator such as `'+'`, `'->'`, or `'#'`
    Operator(String),
    /// `<:`
    Sub,
    /// `:>`
    Sup,
    /// `|-`
    Turnstile,
    /// `-|`
    Tilesturn,
    /// `->`
    Arrow,
    /// `->_`
    ArrowSub,
    /// `=>_`
    DoubleArrowSub,
    /// `==>`
    DoubleArrowLong,
    /// `~>`
    SqArrow,
    /// `~>*`
    SqArrowStar,
    /// `.`
    Dot,
    /// `..`
    Dot2,
    /// `...`
    Dot3,
    /// `;`
    Semicolon,
    /// `:`
    Colon,
    /// `:=`
    ColonEq,
    /// `~~`
    Tilde2,
    /// `\`
    Backslash,
    /// `` `< ``
    LAngle,
    /// `` `> ``
    RAngle,
    /// `` `( ``
    LParen,
    /// `` `) ``
    RParen,
    /// `` `[ ``
    LBrack,
    /// `` `] ``
    RBrack,
    /// `` `{ ``
    LBrace,
    /// `` `} ``
    RBrace,
}

impl Atom {
    // Parse-faithful: round-trips through `from_source`
    pub fn source_string(&self) -> String {
        match self {
            Self::Keyword(identifier) => identifier.clone(),
            Self::Tag(identifier) => format!("_{identifier}"),
            Self::Operator(operator) => format!("'{operator}'"),
            Self::Sub => "<:".into(),
            Self::Sup => ":>".into(),
            Self::Turnstile => "|-".into(),
            Self::Tilesturn => "-|".into(),
            Self::Arrow => "->".into(),
            Self::ArrowSub => "->_".into(),
            Self::DoubleArrowSub => "=>_".into(),
            Self::DoubleArrowLong => "==>".into(),
            Self::SqArrow => "~>".into(),
            Self::SqArrowStar => "~>*".into(),
            Self::Dot => ".".into(),
            Self::Dot2 => "..".into(),
            Self::Dot3 => "...".into(),
            Self::Semicolon => ";".into(),
            Self::Colon => ":".into(),
            Self::ColonEq => ":=".into(),
            Self::Tilde2 => "~~".into(),
            Self::Backslash => "\\".into(),
            Self::LAngle => "`<".into(),
            Self::RAngle => "`>".into(),
            Self::LParen => "`(".into(),
            Self::RParen => "`)".into(),
            Self::LBrack => "`[".into(),
            Self::RBrack => "`]".into(),
            Self::LBrace => "`{".into(),
            Self::RBrace => "`}".into(),
        }
    }

    pub fn from_source(source: &str) -> Self {
        match source {
            "<:" => Self::Sub,
            ":>" => Self::Sup,
            "|-" => Self::Turnstile,
            "-|" => Self::Tilesturn,
            "->" => Self::Arrow,
            "->_" => Self::ArrowSub,
            "=>_" => Self::DoubleArrowSub,
            "==>" => Self::DoubleArrowLong,
            "~>" => Self::SqArrow,
            "~>*" => Self::SqArrowStar,
            "." => Self::Dot,
            ".." => Self::Dot2,
            "..." => Self::Dot3,
            ";" => Self::Semicolon,
            ":" => Self::Colon,
            ":=" => Self::ColonEq,
            "~~" => Self::Tilde2,
            "\\" => Self::Backslash,
            "`<" => Self::LAngle,
            "`>" => Self::RAngle,
            "`(" => Self::LParen,
            "`)" => Self::RParen,
            "`[" => Self::LBrack,
            "`]" => Self::RBrack,
            "`{" => Self::LBrace,
            "`}" => Self::RBrace,
            _ if source.len() >= 2 && source.starts_with('\'') && source.ends_with('\'') => {
                Self::Operator(source[1..source.len() - 1].to_owned())
            }
            _ if source.len() >= 2 && source.starts_with('_') => Self::Tag(source[1..].to_owned()),
            _ => Self::Keyword(source.to_owned()),
        }
    }

    // Lossy display glyph
    pub fn render(&self) -> String {
        match self {
            Self::Tag(identifier) if identifier == "EMPTY" => "/* empty */".into(),
            Self::Operator(operator) => operator.clone(),
            Self::LAngle => "<".into(),
            Self::RAngle => ">".into(),
            Self::LParen => "(".into(),
            Self::RParen => ")".into(),
            Self::LBrack => "[".into(),
            Self::RBrack => "]".into(),
            Self::LBrace => "{".into(),
            Self::RBrace => "}".into(),
            _ => self.source_string(),
        }
    }

    pub fn is_operator(&self, operator: &str) -> bool {
        matches!(self, Self::Operator(current) if current == operator)
    }

    fn is_upid(identifier: &str) -> bool {
        let Some((first, rest)) = identifier.as_bytes().split_first() else {
            return false;
        };

        first.is_ascii_uppercase()
            && rest
                .iter()
                .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'_' | b'\''))
    }

    // Constructors

    pub fn keyword(identifier: impl Into<String>) -> Self {
        Self::Keyword(identifier.into())
    }

    pub fn tag(identifier: impl Into<String>) -> Result<Self, AtomError> {
        let identifier = identifier.into();
        if Self::is_upid(&identifier) {
            Ok(Self::Tag(identifier))
        } else {
            Err(AtomError::InvalidTag(identifier))
        }
    }

    pub fn operator(operator: impl Into<String>) -> Result<Self, AtomError> {
        let operator = operator.into();
        if operator.contains(['\'', '\n']) {
            Err(AtomError::UnquotableOperator(operator))
        } else {
            Ok(Self::Operator(operator))
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum AtomError {
    InvalidTag(String),
    UnquotableOperator(String),
}

impl fmt::Display for AtomError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidTag(identifier) => {
                write!(formatter, "Atom.tag: expected upid: {identifier}")
            }
            Self::UnquotableOperator(operator) => {
                write!(formatter, "Atom.operator: unquotable operator: {operator}")
            }
        }
    }
}

impl Error for AtomError {}

#[cfg(test)]
mod tests {
    use super::Atom;

    #[test]
    fn source_round_trip_preserves_distinct_atom_kinds() {
        let spellings = ["INT", "_NUM", "'->'", "->", "`("];

        for spelling in spellings {
            assert_eq!(Atom::from_source(spelling).source_string(), spelling);
        }
    }

    #[test]
    fn rendering_uses_display_glyphs_instead_of_source_quotes() {
        assert_eq!(Atom::from_source("'->'").render(), "->");
        assert_eq!(Atom::from_source("`<").render(), "<");
        assert_eq!(Atom::from_source("_EMPTY").render(), "/* empty */");
    }
}
