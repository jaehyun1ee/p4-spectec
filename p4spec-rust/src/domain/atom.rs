use thiserror::Error;

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Atom {
    Keyword(String),
    Tag(String),
    Operator(String),
    Sub,
    Sup,
    Turnstile,
    Tilesturn,
    Arrow,
    ArrowSub,
    DoubleArrowSub,
    DoubleArrowLong,
    SqArrow,
    SqArrowStar,
    Dot,
    Dot2,
    Dot3,
    Semicolon,
    Colon,
    ColonEq,
    Tilde2,
    Backslash,
    LAngle,
    RAngle,
    LParen,
    RParen,
    LBrack,
    RBrack,
    LBrace,
    RBrace,
}

impl Atom {
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

    pub fn render(&self) -> String {
        match self {
            Self::Keyword(identifier) => identifier.clone(),
            Self::Tag(identifier) if identifier == "EMPTY" => "/* empty */".into(),
            Self::Tag(identifier) => format!("_{identifier}"),
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

    pub fn keyword(identifier: impl Into<String>) -> Self {
        Self::Keyword(identifier.into())
    }

    pub fn tag(identifier: impl Into<String>) -> Result<Self, AtomError> {
        let identifier = identifier.into();
        if is_upid(&identifier) {
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

    pub fn is_operator(&self, operator: &str) -> bool {
        matches!(self, Self::Operator(current) if current == operator)
    }
}

pub fn is_upid(identifier: &str) -> bool {
    let mut characters = identifier.chars();
    matches!(characters.next(), Some('A'..='Z'))
        && characters.all(|character| character.is_ascii_alphanumeric() || "_'".contains(character))
}

#[derive(Clone, Debug, Error, PartialEq, Eq)]
pub enum AtomError {
    #[error("expected uppercase identifier, got `{0}`")]
    InvalidTag(String),

    #[error("operator cannot be quoted: `{0}`")]
    UnquotableOperator(String),
}
