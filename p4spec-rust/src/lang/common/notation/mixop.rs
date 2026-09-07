use std::{error::Error, fmt};

use crate::lang::{
    common::ds::set::IdSet,
    traits::{
        eq::SyntaxEq,
        free::Free,
        print::{Print, Printer},
    },
};

use super::{atom::Atom, mixfix::Mixfix};

/// A mixfix shape with unfilled argument positions
pub type Mixop = Mixfix<()>;

impl Print for Mixop {
    fn print(&self, printer: &mut Printer<'_>) -> fmt::Result {
        printer.write("`")?;
        self.print_shape(printer)?;
        printer.write("`")
    }
}

fn print_atom(atom: &Atom, printer: &mut Printer<'_>) -> fmt::Result {
    match atom {
        Atom::Tag(id) if id == "EMPTY" => printer.write("/* empty */"),
        Atom::Operator(op) => printer.write(op),
        Atom::LAngle => printer.write("<"),
        Atom::RAngle => printer.write(">"),
        Atom::LParen => printer.write("("),
        Atom::RParen => printer.write(")"),
        Atom::LBrack => printer.write("["),
        Atom::RBrack => printer.write("]"),
        Atom::LBrace => printer.write("{"),
        Atom::RBrace => printer.write("}"),
        _ => atom.print(printer),
    }
}

impl Mixop {
    fn print_shape(&self, printer: &mut Printer<'_>) -> fmt::Result {
        match self {
            Self::Arg(()) => printer.write("%"),
            Self::Atom(atom) => print_atom(&atom.node, printer),
            Self::Brack(atom_l, mixop, atom_r) => {
                print_atom(&atom_l.node, printer)?;
                mixop.print_shape(printer)?;
                print_atom(&atom_r.node, printer)
            }
            Self::Infix(mixop_l, atom, mixop_r) => {
                mixop_l.print_shape(printer)?;
                print_atom(&atom.node, printer)?;
                mixop_r.print_shape(printer)
            }
            Self::Seq(mixops) => {
                for (index, mixop) in mixops.iter().enumerate() {
                    if index != 0 {
                        printer.write(" ")?;
                    }
                    mixop.print_shape(printer)?;
                }
                Ok(())
            }
        }
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

// == Converting a mixfix to a mixop

impl<T> Mixfix<T> {
    /// Replaces every argument with an unfilled mixop position
    pub fn to_mixop(&self) -> Mixop {
        self.map(|_| ())
    }

    /// Separates the mixop shape from its arguments
    pub fn split(&self) -> (Mixop, Vec<&T>) {
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

impl Mixop {
    /// Fills a mixfix operator with arguments
    pub fn fill<T>(
        mixop: &Self,
        args: impl IntoIterator<Item = T>,
    ) -> Result<Mixfix<T>, ArityMismatch> {
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
    ) -> Result<Mixfix<T>, ArityMismatch> {
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
