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

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum AtomError {
    InvalidTag(String),
    InvalidOperator(String),
}

impl fmt::Display for AtomError {
    fn fmt(&self, fmt: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidTag(id) => {
                write!(
                    fmt,
                    "invalid tag identifier {id:?}: expected an uppercase identifier"
                )
            }
            Self::InvalidOperator(op) => {
                write!(
                    fmt,
                    "invalid operator {op:?}: must not contain a quote or newline"
                )
            }
        }
    }
}

impl Error for AtomError {}

// == String conversion and parsing

impl Atom {
    /// String representation of the atom
    pub fn to_string(&self) -> String {
        match self {
            Self::Keyword(id) => id.clone(),
            Self::Tag(id) => format!("_{id}"),
            Self::Operator(op) => format!("'{op}'"),
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

    /// Parses a string into an atom
    pub fn of_string(source: &str) -> Self {
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
}

// == Rendering

impl Atom {
    /// Returns the display spelling, omitting source-only quoting where applicable
    pub fn render(&self) -> String {
        match self {
            Self::Tag(id) if id == "EMPTY" => "/* empty */".into(),
            Self::Operator(op) => op.clone(),
            Self::LAngle => "<".into(),
            Self::RAngle => ">".into(),
            Self::LParen => "(".into(),
            Self::RParen => ")".into(),
            Self::LBrack => "[".into(),
            Self::RBrack => "]".into(),
            Self::LBrace => "{".into(),
            Self::RBrace => "}".into(),
            _ => self.to_string(),
        }
    }
}

// == Constructors

impl Atom {
    // - Keyword

    /// Constructs a keyword atom from an identifier
    pub fn keyword(id: impl Into<String>) -> Self {
        Self::Keyword(id.into())
    }

    // - Tag

    fn is_upid(id: &str) -> bool {
        let Some((c_first, s_rest)) = id.as_bytes().split_first() else {
            return false;
        };

        c_first.is_ascii_uppercase()
            && s_rest
                .iter()
                .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'_' | b'\''))
    }

    /// Constructs a tag atom when the identifier is a valid upper identifier
    pub fn tag(id: impl Into<String>) -> Result<Self, AtomError> {
        let id = id.into();
        if Self::is_upid(&id) {
            Ok(Self::Tag(id))
        } else {
            Err(AtomError::InvalidTag(id))
        }
    }

    // - Operator

    /// Constructs an operator atom when it can be represented in source syntax
    pub fn operator(op: impl Into<String>) -> Result<Self, AtomError> {
        let op = op.into();
        if op.contains(['\'', '\n']) {
            Err(AtomError::InvalidOperator(op))
        } else {
            Ok(Self::Operator(op))
        }
    }
}
