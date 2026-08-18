use std::fmt;

// Positions and regions

#[derive(Clone, Debug, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Position {
    pub file: String,
    pub line: i64,
    pub column: i64,
}

#[derive(Clone, Debug, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Region {
    pub left: Position,
    pub right: Position,
}

impl Position {
    pub fn new(file: impl Into<String>, line: i64, column: i64) -> Self {
        Self {
            file: file.into(),
            line,
            column,
        }
    }
}

impl Region {
    pub fn new(left: Position, right: Position) -> Self {
        Self { left, right }
    }

    pub fn none() -> Self {
        Self::default()
    }
}

impl Position {
    pub fn for_file(file: impl Into<String>) -> Self {
        Self::new(file, 0, 0)
    }
}

impl Region {
    pub fn for_file(file: impl Into<String>) -> Self {
        let position = Position::for_file(file);
        Self::new(position.clone(), position)
    }

    pub fn before(&self) -> Self {
        Self::new(self.left.clone(), self.left.clone())
    }

    pub fn after(&self) -> Self {
        Self::new(self.right.clone(), self.right.clone())
    }
}

impl fmt::Display for Position {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.line == -1 {
            write!(formatter, "0x{:x}", self.column)
        } else {
            write!(formatter, "{}.{}", self.line, self.column + 1)
        }
    }
}

impl fmt::Display for Region {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self == &Self::for_file(&self.left.file) {
            return formatter.write_str(&self.left.file);
        }
        write!(formatter, "{}:{}", self.left.file, self.left)?;
        if self.left != self.right {
            write!(formatter, "-{}", self.right)?;
        }
        Ok(())
    }
}

// Phrases

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Info<I, N, A> {
    pub it: I,
    pub note: N,
    pub at: A,
}

pub type NotePhrase<I, N> = Info<I, N, Region>;
pub type Note<I, N> = Info<I, N, ()>;
pub type Phrase<I> = Info<I, (), Region>;

impl<I> Info<I, (), Region> {
    pub fn new(it: I, at: Region) -> Self {
        Self { it, note: (), at }
    }
}

impl<I, N> Info<I, N, Region> {
    pub fn with_note(it: I, at: Region, note: N) -> Self {
        Self { it, note, at }
    }
}

impl<I, N, A> Info<I, N, A> {
    pub fn map<J>(self, map: impl FnOnce(I) -> J) -> Info<J, N, A> {
        Info {
            it: map(self.it),
            note: self.note,
            at: self.at,
        }
    }
}

impl Region {
    pub fn over(regions: &[Self]) -> Self {
        let Some((first, rest)) = regions.split_first() else {
            return Self::none();
        };
        rest.iter().fold(first.clone(), |region_over, region| {
            Self::new(
                region_over.left.min(region.left.clone()),
                region_over.right.max(region.right.clone()),
            )
        })
    }
}

pub fn phrase_list_region<I, N>(phrases: &[Info<I, N, Region>]) -> Region {
    match phrases {
        [] => Region::none(),
        [phrase] => phrase.at.clone(),
        [first, .., last] => Region::new(first.at.left.clone(), last.at.right.clone()),
    }
}
