use super::mixfix::{ArityMismatch, AtomPhrase, Mixfix, Mixop};

impl Mixop {
    pub fn fill<T>(
        mixop: &Self,
        args: impl IntoIterator<Item = T>,
    ) -> Result<Mixfix<T>, ArityMismatch> {
        fn fill<T>(
            mixop: &Mixop,
            args: &mut impl Iterator<Item = T>,
        ) -> Result<Mixfix<T>, ArityMismatch> {
            match mixop {
                Mixfix::Arg(()) => args.next().map(Mixfix::Arg).ok_or(ArityMismatch::TooFew),
                Mixfix::Atom(atom) => Ok(Mixfix::Atom(atom.clone())),
                Mixfix::Brack(left, body, right) => Ok(Mixfix::Brack(
                    left.clone(),
                    Box::new(fill(body, args)?),
                    right.clone(),
                )),
                Mixfix::Infix(left, atom, right) => Ok(Mixfix::Infix(
                    Box::new(fill(left, args)?),
                    atom.clone(),
                    Box::new(fill(right, args)?),
                )),
                Mixfix::Seq(items) => Ok(Mixfix::Seq(
                    items
                        .iter()
                        .map(|item| fill(item, args))
                        .collect::<Result<_, _>>()?,
                )),
            }
        }

        let mut args = args.into_iter();
        let mixfix = fill(mixop, &mut args)?;
        if args.next().is_some() {
            Err(ArityMismatch::TooMany)
        } else {
            Ok(mixfix)
        }
    }
}

pub fn assemble(
    mixop: &Mixop,
    args: impl IntoIterator<Item = String>,
    string_of_atom: impl FnMut(&AtomPhrase) -> String,
) -> Result<String, ArityMismatch> {
    Ok(Mixop::fill(mixop, args)?.render(string_of_atom, Clone::clone))
}

#[cfg(test)]
mod tests {
    use super::{ArityMismatch, Mixfix, Mixop};
    use crate::domain::{
        atom::Atom,
        mixfix::AtomPhrase,
        source::{Region, Spanned},
    };

    fn atom(value: Atom) -> AtomPhrase {
        Spanned::new(value, Region::none())
    }

    fn nested_mixop() -> Mixop {
        Mixfix::Brack(
            atom(Atom::LParen),
            Box::new(Mixfix::Infix(
                Box::new(Mixfix::Arg(())),
                atom(Atom::Colon),
                Box::new(Mixfix::Seq(vec![
                    Mixfix::Atom(atom(Atom::Tag("MID".into()))),
                    Mixfix::Arg(()),
                ])),
            )),
            atom(Atom::RParen),
        )
    }

    #[test]
    fn fill_consumes_arguments_in_tree_order() {
        let filled = Mixop::fill(&nested_mixop(), ["left", "right"])
            .expect("operator accepts exactly two arguments");

        assert_eq!(
            filled.render(|atom| atom.node.render(), |argument| (*argument).to_owned()),
            "( left : _MID right )"
        );
    }

    #[test]
    fn fill_distinguishes_argument_count_mismatches() {
        assert_eq!(
            Mixop::fill(&nested_mixop(), ["only"]).expect_err("one argument is too few"),
            ArityMismatch::TooFew
        );
        assert_eq!(
            Mixop::fill(&nested_mixop(), ["one", "two", "three"])
                .expect_err("three arguments are too many"),
            ArityMismatch::TooMany
        );
    }
}
