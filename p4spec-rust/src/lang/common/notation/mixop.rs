use std::{cell::RefCell, collections::HashMap, error::Error, fmt, rc::Rc};

use crate::frontend;

use crate::lang::{
    common::{ds::set::IdSet, source::Span},
    traits::{
        eq::SyntaxEq,
        free::Free,
        print::{Print, Printer},
    },
};

use super::mixfix::Mixfix;

/// A mixfix shape with unfilled argument positions
pub type Mixop<S = Span> = Mixfix<(), S>;

impl<S> Print for Mixop<S> {
    fn print(&self, printer: &mut Printer<'_>) -> fmt::Result {
        self.print_with(printer, |(), printer| printer.write("%"))
    }
}

// == Syntax operations

impl SyntaxEq for () {
    fn syntax_eq(&self, _other: &Self) -> bool {
        true
    }
}

impl Free for () {
    fn free(&self) -> IdSet {
        IdSet::new()
    }
}

// = Shape parsing

thread_local! {
    static SHAPE_CACHE: RefCell<HashMap<Rc<str>, Rc<Mixop>>> = RefCell::new(HashMap::new());
}

pub(crate) fn shape(shape_text: &str) -> Rc<Mixop> {
    SHAPE_CACHE.with(|cache| {
        if let Some(mixop) = cache.borrow().get(shape_text).cloned() {
            return mixop;
        }

        let mixop = frontend::parse::parse_mixop(shape_text)
            .expect("value constructor contains a valid SpecTec mixop");
        let mixop = Rc::new(mixop);
        cache
            .borrow_mut()
            .insert(Rc::from(shape_text), Rc::clone(&mixop));
        mixop
    })
}

// == Converting a mixfix to a mixop

impl<T, S: Clone> Mixfix<T, S> {
    /// Replaces every argument with an unfilled mixop position
    pub fn to_mixop(&self) -> Mixop<S> {
        self.map(|_| ())
    }

    /// Separates the mixop shape from its arguments
    pub fn split(&self) -> (Mixop<S>, Vec<&T>) {
        (self.to_mixop(), self.args())
    }
}

// == Filling a mixop with arguments

/// An error caused by a mismatch between mixop arity and supplied arguments
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ArityMismatch {
    /// Fewer arguments were supplied than the mixop requires
    TooFew,
    /// More arguments were supplied than the mixop requires
    TooMany,
}

impl fmt::Display for ArityMismatch {
    fn fmt(&self, fmt: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::TooFew => fmt.write_str("Mixop.fill: too few arguments"),
            Self::TooMany => fmt.write_str("Mixop.fill: too many arguments"),
        }
    }
}

impl Error for ArityMismatch {}

impl<S: Clone> Mixop<S> {
    /// Fills a mixfix operator with arguments
    pub fn fill<T>(
        mixop: &Self,
        args: impl IntoIterator<Item = T>,
    ) -> Result<Mixfix<T, S>, ArityMismatch> {
        let mut args = args.into_iter();
        let mixfix = mixop.fill_inner(&mut args)?;
        if args.next().is_some() {
            Err(ArityMismatch::TooMany)
        } else {
            Ok(mixfix)
        }
    }

    fn fill_inner<T>(
        &self,
        args: &mut impl Iterator<Item = T>,
    ) -> Result<Mixfix<T, S>, ArityMismatch> {
        match self {
            Self::Arg(()) => args.next().map(Mixfix::Arg).ok_or(ArityMismatch::TooFew),
            Self::Atom(atom) => Ok(Mixfix::Atom(atom.clone())),
            Self::Brack(atom_l, mixfix, atom_r) => Ok(Mixfix::Brack(
                atom_l.clone(),
                Box::new(mixfix.fill_inner(args)?),
                atom_r.clone(),
            )),
            Self::Infix(mixfix_l, atom, mixfix_r) => Ok(Mixfix::Infix(
                Box::new(mixfix_l.fill_inner(args)?),
                atom.clone(),
                Box::new(mixfix_r.fill_inner(args)?),
            )),
            Self::Seq(mixfixes) => Ok(Mixfix::Seq(
                mixfixes
                    .iter()
                    .map(|mixfix| mixfix.fill_inner(args))
                    .collect::<Result<_, _>>()?,
            )),
        }
    }
}
