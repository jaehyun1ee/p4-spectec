use crate::lang::{
    el::{
        ast::{Atom, Exp, ExpKind, Hole as ElHole, Text},
        print,
    },
    hints::input,
};

#[derive(Clone, Debug, PartialEq)]
pub enum Hole {
    Next,
    Num(i64),
}

#[derive(Clone, Debug, PartialEq)]
pub enum T {
    TextH(Text),
    AtomH(Atom),
    SeqH(Vec<T>),
    BrackH(Atom, Box<T>, Atom),
    HoleH(Hole),
    FuseH(Box<T>, Box<T>),
    OtherH(Exp),
}

pub fn to_string(hint: &T) -> String {
    format!("hint(alter {})", string(hint))
}
fn string(hint: &T) -> String {
    match hint {
        T::TextH(text) => print::string_of_text(text),
        T::AtomH(atom) => print::string_of_atom(atom),
        T::SeqH(hints) => hints.iter().map(string).collect::<Vec<_>>().join(" "),
        T::BrackH(left, inner, right) => format!(
            "{} {} {}",
            print::string_of_atom(left),
            string(inner),
            print::string_of_atom(right)
        ),
        T::HoleH(Hole::Next) => "%".into(),
        T::HoleH(Hole::Num(index)) => format!("%{index}"),
        T::FuseH(left, right) => format!("{}#{}", string(left), string(right)),
        T::OtherH(exp) => print::string_of_exp(exp),
    }
}
pub fn init(exp: &Exp) -> Option<T> {
    Some(match &exp.node {
        ExpKind::TextE(text) => T::TextH(text.clone()),
        ExpKind::AtomE(atom) => T::AtomH(atom.clone()),
        ExpKind::SeqE(exps) => T::SeqH(exps.iter().map(init).collect::<Option<_>>()?),
        ExpKind::BrackE(left, exp, right) => {
            T::BrackH(left.clone(), Box::new(init(exp)?), right.clone())
        }
        ExpKind::HoleE(ElHole::Next) => T::HoleH(Hole::Next),
        ExpKind::HoleE(ElHole::Num(index)) => T::HoleH(Hole::Num(*index)),
        ExpKind::FuseE(left, right) => T::FuseH(Box::new(init(left)?), Box::new(init(right)?)),
        _ => T::OtherH(exp.clone()),
    })
}
pub fn validate<Item>(hint: &T, items: &[Item]) -> Result<(), String> {
    validate_at(hint, items, 0).map(|_| ())
}
fn validate_at<Item>(hint: &T, items: &[Item], cursor: usize) -> Result<usize, String> {
    match hint {
        T::TextH(_) | T::AtomH(_) | T::OtherH(_) => Ok(cursor),
        T::SeqH(hints) => hints
            .iter()
            .try_fold(cursor, |cursor, hint| validate_at(hint, items, cursor)),
        T::BrackH(_, hint, _) => validate_at(hint, items, cursor),
        T::HoleH(Hole::Next) => Ok(cursor + 1),
        T::HoleH(Hole::Num(index)) if *index >= 0 && (*index as usize) < items.len() => Ok(cursor),
        T::HoleH(Hole::Num(index)) => Err(format!("index {index} out of bounds")),
        T::FuseH(left, right) => validate_at(right, items, validate_at(left, items, cursor)?),
    }
}
pub fn collect(hint: &T) -> Vec<i64> {
    fn go(h: &T, out: &mut Vec<i64>) {
        match h {
            T::HoleH(Hole::Num(i)) => out.insert(0, *i),
            T::SeqH(xs) => {
                for x in xs {
                    go(x, out)
                }
            }
            T::BrackH(_, x, _) => go(x, out),
            T::FuseH(l, r) => {
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
pub fn realign(hint: &T, inputs: &input::T) -> Result<T, String> {
    let outputs = collect(hint);
    let mut all = inputs.clone();
    all.extend(&outputs);
    all.sort();
    let mut pairs = Vec::new();
    for index in all {
        if outputs.contains(&index) {
            pairs.push((index, pairs.len() as i64));
        }
    }
    fn go(h: &T, p: &[(i64, i64)]) -> Result<T, String> {
        Ok(match h {
            T::SeqH(xs) => T::SeqH(xs.iter().map(|x| go(x, p)).collect::<Result<_, _>>()?),
            T::BrackH(l, x, r) => T::BrackH(l.clone(), Box::new(go(x, p)?), r.clone()),
            T::HoleH(Hole::Num(i)) => T::HoleH(Hole::Num(
                p.iter()
                    .find(|(old, _)| old == i)
                    .ok_or_else(|| format!("index {i} missing"))?
                    .1,
            )),
            T::FuseH(l, r) => T::FuseH(Box::new(go(l, p)?), Box::new(go(r, p)?)),
            _ => h.clone(),
        })
    }
    go(hint, &pairs)
}
#[allow(clippy::too_many_arguments)]
pub fn alternate<Item, D>(
    hint: &T,
    items: &[Item],
    empty: D,
    text: impl Fn(&str) -> Option<D>,
    atom: impl Fn(&Atom) -> D,
    join: impl Fn(Vec<D>) -> D,
    fuse: impl Fn(D, D) -> D,
    other: impl Fn(&Exp) -> D,
    render: impl Fn(&Item) -> D,
) -> Result<D, String>
where
    D: Clone,
{
    #[allow(clippy::too_many_arguments)]
    fn go<Item, D>(
        h: &T,
        items: &[Item],
        cursor: usize,
        empty: &D,
        text: &impl Fn(&str) -> Option<D>,
        atom: &impl Fn(&Atom) -> D,
        join: &impl Fn(Vec<D>) -> D,
        fuse: &impl Fn(D, D) -> D,
        other: &impl Fn(&Exp) -> D,
        render: &impl Fn(&Item) -> D,
    ) -> Result<(usize, Option<D>), String>
    where
        D: Clone,
    {
        Ok(match h {
            T::TextH(s) => (cursor, text(s)),
            T::AtomH(a) => (cursor, Some(atom(a))),
            T::SeqH(xs) => {
                let mut c = cursor;
                let mut ds = Vec::new();
                for x in xs {
                    let (n, d) = go(x, items, c, empty, text, atom, join, fuse, other, render)?;
                    c = n;
                    ds.push(d.unwrap_or_else(|| empty.clone()));
                }
                (c, Some(join(ds)))
            }
            T::BrackH(l, x, r) => {
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
            T::HoleH(Hole::Next) => {
                let item = items
                    .get(cursor)
                    .ok_or_else(|| format!("index {cursor} out of bounds"))?;
                (cursor + 1, Some(render(item)))
            }
            T::HoleH(Hole::Num(i)) => {
                let item = items
                    .get(*i as usize)
                    .ok_or_else(|| format!("index {i} out of bounds"))?;
                (cursor, Some(render(item)))
            }
            T::FuseH(l, r) => {
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
            T::OtherH(e) => (cursor, Some(other(e))),
        })
    }
    Ok(go(
        hint, items, 0, &empty, &text, &atom, &join, &fuse, &other, &render,
    )?
    .1
    .unwrap_or(empty))
}
