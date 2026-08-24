use crate::lang::{
    el::{
        ast::{Atom, Exp, ExpKind, Hole as ElHole, Text},
        print,
    },
    hints::input,
};
use thiserror::Error;

// Alternation hints

#[derive(Clone, Debug, PartialEq)]
pub enum Hole {
    Next,
    Num(i64),
}

#[derive(Clone, Debug, PartialEq)]
pub enum AlterationHint {
    TextH(Text),
    AtomH(Atom),
    SeqH(Vec<AlterationHint>),
    BrackH(Atom, Box<AlterationHint>, Atom),
    HoleH(Hole),
    FuseH(Box<AlterationHint>, Box<AlterationHint>),
    OtherH(Exp),
}

#[derive(Clone, Debug, Error, PartialEq, Eq)]
pub enum AlterationError {
    #[error("alteration hint index {index} is out of bounds for {item_count} items")]
    IndexOutOfBounds { index: i64, item_count: usize },

    #[error("alteration hint index {0} is missing from the realignment")]
    MissingIndex(i64),
}

pub fn to_string(hint: &AlterationHint) -> String {
    format!("hint(alter {})", string(hint))
}
fn string(hint: &AlterationHint) -> String {
    match hint {
        AlterationHint::TextH(text) => print::string_of_text(text),
        AlterationHint::AtomH(atom) => print::string_of_atom(atom),
        AlterationHint::SeqH(hints) => hints.iter().map(string).collect::<Vec<_>>().join(" "),
        AlterationHint::BrackH(left, inner, right) => format!(
            "{} {} {}",
            print::string_of_atom(left),
            string(inner),
            print::string_of_atom(right)
        ),
        AlterationHint::HoleH(Hole::Next) => "%".into(),
        AlterationHint::HoleH(Hole::Num(index)) => format!("%{index}"),
        AlterationHint::FuseH(left, right) => format!("{}#{}", string(left), string(right)),
        AlterationHint::OtherH(exp) => print::string_of_exp(exp),
    }
}
// Creating hints

pub fn init(exp: &Exp) -> Option<AlterationHint> {
    Some(match &exp.node {
        ExpKind::TextE(text) => AlterationHint::TextH(text.clone()),
        ExpKind::AtomE(atom) => AlterationHint::AtomH(atom.clone()),
        ExpKind::SeqE(exps) => AlterationHint::SeqH(exps.iter().map(init).collect::<Option<_>>()?),
        ExpKind::BrackE(left, exp, right) => {
            AlterationHint::BrackH(left.clone(), Box::new(init(exp)?), right.clone())
        }
        ExpKind::HoleE(ElHole::Next) => AlterationHint::HoleH(Hole::Next),
        ExpKind::HoleE(ElHole::Num(index)) => AlterationHint::HoleH(Hole::Num(*index)),
        ExpKind::FuseE(left, right) => {
            AlterationHint::FuseH(Box::new(init(left)?), Box::new(init(right)?))
        }
        _ => AlterationHint::OtherH(exp.clone()),
    })
}
// Validating hints

pub fn validate<Item>(hint: &AlterationHint, items: &[Item]) -> Result<(), AlterationError> {
    validate_at(hint, items, 0).map(|_| ())
}
fn validate_at<Item>(
    hint: &AlterationHint,
    items: &[Item],
    cursor: usize,
) -> Result<usize, AlterationError> {
    match hint {
        AlterationHint::TextH(_) | AlterationHint::AtomH(_) | AlterationHint::OtherH(_) => {
            Ok(cursor)
        }
        AlterationHint::SeqH(hints) => hints
            .iter()
            .try_fold(cursor, |cursor, hint| validate_at(hint, items, cursor)),
        AlterationHint::BrackH(_, hint, _) => validate_at(hint, items, cursor),
        AlterationHint::HoleH(Hole::Next) if cursor < items.len() => Ok(cursor + 1),
        AlterationHint::HoleH(Hole::Next) => Err(AlterationError::IndexOutOfBounds {
            index: i64::try_from(cursor).unwrap_or(i64::MAX),
            item_count: items.len(),
        }),
        AlterationHint::HoleH(Hole::Num(index))
            if *index >= 0 && (*index as usize) < items.len() =>
        {
            Ok(cursor)
        }
        AlterationHint::HoleH(Hole::Num(index)) => Err(AlterationError::IndexOutOfBounds {
            index: *index,
            item_count: items.len(),
        }),
        AlterationHint::FuseH(left, right) => {
            validate_at(right, items, validate_at(left, items, cursor)?)
        }
    }
}
// Re-alignment of alternation indices

pub fn collect(hint: &AlterationHint) -> Vec<i64> {
    fn go(h: &AlterationHint, out: &mut Vec<i64>) {
        match h {
            AlterationHint::HoleH(Hole::Num(i)) => out.insert(0, *i),
            AlterationHint::SeqH(xs) => {
                for x in xs {
                    go(x, out)
                }
            }
            AlterationHint::BrackH(_, x, _) => go(x, out),
            AlterationHint::FuseH(l, r) => {
                go(l, out);
                go(r, out)
            }
            _ => {}
        }
    }
    let mut out = Vec::new();
    go(hint, &mut out);
    out
}
pub fn realign(
    hint: &AlterationHint,
    inputs: &input::InputHint,
) -> Result<AlterationHint, AlterationError> {
    let outputs = collect(hint);
    let mut all = inputs.indices().to_vec();
    all.extend(&outputs);
    all.sort();
    let mut pairs = Vec::new();
    for index in all {
        if outputs.contains(&index) {
            pairs.push((index, pairs.len() as i64));
        }
    }
    fn go(h: &AlterationHint, pairs: &[(i64, i64)]) -> Result<AlterationHint, AlterationError> {
        Ok(match h {
            AlterationHint::SeqH(xs) => {
                AlterationHint::SeqH(xs.iter().map(|x| go(x, pairs)).collect::<Result<_, _>>()?)
            }
            AlterationHint::BrackH(l, x, r) => {
                AlterationHint::BrackH(l.clone(), Box::new(go(x, pairs)?), r.clone())
            }
            AlterationHint::HoleH(Hole::Num(i)) => AlterationHint::HoleH(Hole::Num(
                pairs
                    .iter()
                    .find(|(old, _)| old == i)
                    .ok_or(AlterationError::MissingIndex(*i))?
                    .1,
            )),
            AlterationHint::FuseH(l, r) => {
                AlterationHint::FuseH(Box::new(go(l, pairs)?), Box::new(go(r, pairs)?))
            }
            _ => h.clone(),
        })
    }
    go(hint, &pairs)
}
// Alternation

#[allow(clippy::too_many_arguments)]
pub fn alternate<Item, D>(
    hint: &AlterationHint,
    items: &[Item],
    empty: D,
    text: impl Fn(&str) -> Option<D>,
    atom: impl Fn(&Atom) -> D,
    join: impl Fn(Vec<D>) -> D,
    fuse: impl Fn(D, D) -> D,
    other: impl Fn(&Exp) -> D,
    render: impl Fn(&Item) -> D,
) -> Result<D, AlterationError>
where
    D: Clone,
{
    #[allow(clippy::too_many_arguments)]
    fn go<Item, D>(
        h: &AlterationHint,
        items: &[Item],
        cursor: usize,
        empty: &D,
        text: &impl Fn(&str) -> Option<D>,
        atom: &impl Fn(&Atom) -> D,
        join: &impl Fn(Vec<D>) -> D,
        fuse: &impl Fn(D, D) -> D,
        other: &impl Fn(&Exp) -> D,
        render: &impl Fn(&Item) -> D,
    ) -> Result<(usize, Option<D>), AlterationError>
    where
        D: Clone,
    {
        Ok(match h {
            AlterationHint::TextH(s) => (cursor, text(s)),
            AlterationHint::AtomH(a) => (cursor, Some(atom(a))),
            AlterationHint::SeqH(xs) => {
                let mut c = cursor;
                let mut ds = Vec::new();
                for x in xs {
                    let (n, d) = go(x, items, c, empty, text, atom, join, fuse, other, render)?;
                    c = n;
                    ds.push(d.unwrap_or_else(|| empty.clone()));
                }
                (c, Some(join(ds)))
            }
            AlterationHint::BrackH(l, x, r) => {
                let (c, d) = go(
                    x, items, cursor, empty, text, atom, join, fuse, other, render,
                )?;
                let mut ds = vec![atom(l)];
                if let Some(d) = d {
                    ds.push(d);
                }
                ds.push(atom(r));
                (c, Some(join(ds)))
            }
            AlterationHint::HoleH(Hole::Next) => {
                let item = items.get(cursor).ok_or(AlterationError::IndexOutOfBounds {
                    index: i64::try_from(cursor).unwrap_or(i64::MAX),
                    item_count: items.len(),
                })?;
                (cursor + 1, Some(render(item)))
            }
            AlterationHint::HoleH(Hole::Num(i)) => {
                let item = usize::try_from(*i)
                    .ok()
                    .and_then(|index| items.get(index))
                    .ok_or(AlterationError::IndexOutOfBounds {
                        index: *i,
                        item_count: items.len(),
                    })?;
                (cursor, Some(render(item)))
            }
            AlterationHint::FuseH(l, r) => {
                let (c, a) = go(
                    l, items, cursor, empty, text, atom, join, fuse, other, render,
                )?;
                let (c, b) = go(r, items, c, empty, text, atom, join, fuse, other, render)?;
                (
                    c,
                    Some(fuse(
                        a.unwrap_or_else(|| empty.clone()),
                        b.unwrap_or_else(|| empty.clone()),
                    )),
                )
            }
            AlterationHint::OtherH(e) => (cursor, Some(other(e))),
        })
    }
    Ok(go(
        hint, items, 0, &empty, &text, &atom, &join, &fuse, &other, &render,
    )?
    .1
    .unwrap_or(empty))
}
