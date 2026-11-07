//! Printing the walk-through for the mini K and Redex specifications
//!
//! The walk-through invokes `P4Stmt_eval` on the statement
//! `if (hdr.dst == 0x42) ...` of filter.p4 under an environment where
//! `hdr.dst` is not 0x42, so rule `if_true` is unmatched and `if_false` matches.
//! `emit <out-dir>` writes, for both specifications, the encoded mini P4
//! specification, the call, and the result of the same call under meta-execution:
//! `walkthrough.rktd` as one s-expression `(spec id (val ...) expected)`,
//! and `walkthrough.k` as one K term `run(spec, id, vals, expected)`.

use std::{path::PathBuf, rc::Rc};

use p4spectec::{
    lang::data::{arena::Arena, value::tree::ValueKind},
    runner,
};

use crate::{
    encode::Encoder,
    externs::PacketEngine,
    value::{self as v, Error, Registry, Tree},
};

/// The environment and statement of the walk-through, as P4 meta-values.
fn walkthrough(p4: &Registry) -> Result<(Tree, Tree), Error> {
    let bit = |n: u64| p4.case("BitV %", vec![v::nat(n)]);
    let field =
        |name: &str, n: u64| -> Result<Tree, Error> { Ok(v::tuple(vec![v::text(name), bit(n)?])) };
    let fields = v::list(v::typ_var("E"), vec![field("src", 0x01)?, field("dst", 0x43)?]);
    let hdr = p4.case("HdrV % %", vec![v::text("H"), fields])?;
    let pass = p4.case("BoolV %", vec![v::bool(false)])?;
    let env = v::list(
        v::typ_var("E"),
        vec![v::tuple(vec![v::text("hdr"), hdr]), v::tuple(vec![v::text("pass"), pass])],
    );
    // if (hdr.dst == 0x42) { pass = false; } else { pass = true; }
    let var = p4.case("VarE %", vec![v::text("hdr")])?;
    let dst = p4.case("FieldE % %", vec![var, v::text("dst")])?;
    let cond = p4.case("EqE % %", vec![dst, p4.case("BitE %", vec![v::nat(0x42)])?])?;
    let assign = |b: bool| -> Result<Tree, Error> {
        let exp = p4.case("BoolE %", vec![v::bool(b)])?;
        p4.case("AssignS % %", vec![v::text("pass"), exp])
    };
    let stmt = p4.case("IfS % % %", vec![cond, assign(false)?, assign(true)?])?;
    Ok((env, stmt))
}

pub fn run(args: Vec<String>) -> Result<(), Error> {
    let [out] = <[_; 1]>::try_from(args).map_err(|_| "usage: metasq-example emit <out-dir>")?;
    let out = PathBuf::from(out);
    let spec_p4 =
        p4spectec::algo(&[crate::root().join("p4/p4.watsup")]).map_err(|r| r.to_string())?;
    let spec_st = p4spectec::algo(&crate::files("st-in-st")).map_err(|r| r.to_string())?;
    let p4 = Rc::new(Registry::new(&spec_p4));
    let st = Registry::new(&spec_st);
    let encoder = Encoder::new(&st);
    // Run the call under meta-execution for the expected result
    let mut arena = Arena::new();
    let (env, stmt) = walkthrough(&p4)?;
    let inputs = vec![v::intern(&mut arena, env)?, v::intern(&mut arena, stmt)?];
    let external = PacketEngine { p4: p4.clone() };
    let mut runner = runner::build_al(&mut arena, spec_p4.clone(), crate::config(), external)
        .map_err(|r| r.to_string())?;
    let outputs = runner
        .eval_rel(&mut arena, "P4Stmt_eval", &inputs)
        .result
        .map_err(|e| e.to_string())?;
    let expected = v::read_many(&arena, &outputs)
        .into_iter()
        .map(|output| encoder.val(output))
        .collect::<Result<Vec<_>, _>>()?;
    // Encode the specification and the call
    let spec = encoder.spec(&spec_p4)?;
    let (env, stmt) = walkthrough(&p4)?;
    let vals = vec![encoder.val(env)?, encoder.val(stmt)?];
    let id = v::text("P4Stmt_eval");
    // Write the s-expression for Redex
    let rktd = format!(
        "({}\n {}\n {}\n {})\n",
        sexp(&spec)?,
        sexp(&id)?,
        sexp_list(&vals)?,
        sexp_list(&expected)?
    );
    std::fs::write(out.join("walkthrough.rktd"), rktd).map_err(|e| e.to_string())?;
    // Write the K term
    let k = format!(
        "run({},\n  {},\n  {},\n  {})\n",
        kterm(&spec)?,
        kterm(&id)?,
        klist("Vals", &vals)?,
        klist("Vals", &expected)?
    );
    std::fs::write(out.join("walkthrough.k"), k).map_err(|e| e.to_string())?;
    println!("wrote {}", out.display());
    Ok(())
}

// = S-expressions

fn num(n: &p4spectec::lang::common::prim::num::Number) -> Result<String, Error> {
    match n {
        p4spectec::lang::common::prim::num::Number::Nat(n) => Ok(n.as_bigint().to_string()),
        p4spectec::lang::common::prim::num::Number::Int(_) => {
            Err("mini p4st has no int".to_owned())
        }
    }
}

fn string_literal(text: &str) -> String {
    serde_json::to_string(text).expect("a text serializes")
}

fn sexp_list(values: &[Tree]) -> Result<String, Error> {
    let items = values.iter().map(sexp).collect::<Result<Vec<_>, _>>()?;
    Ok(format!("({})", items.join(" ")))
}

/// Prints a meta-meta-value as an s-expression: a case as its atoms and arguments in order.
fn sexp(value: &Tree) -> Result<String, Error> {
    Ok(match &value.node {
        ValueKind::Bool(b) => if *b { "#t" } else { "#f" }.to_owned(),
        ValueKind::Num(n) => num(n)?,
        ValueKind::Text(t) => string_literal(t),
        ValueKind::Extern(json) => string_literal(&json.to_string()),
        ValueKind::List(values) | ValueKind::Tuple(values) => sexp_list(values)?,
        ValueKind::Opt(value) => match value {
            Some(value) => format!("({})", sexp(value)?),
            None => "()".to_owned(),
        },
        ValueKind::Case(_) => {
            let items = v::items(value)?;
            let nullary = matches!(items.as_slice(), [Ok(_)]);
            let items = items
                .into_iter()
                .map(|item| item.or_else(sexp))
                .collect::<Result<Vec<_>, _>>()?;
            if nullary { items.concat() } else { format!("({})", items.join(" ")) }
        }
        _ => return Err("no s-expression for this value".to_owned()),
    })
}

// = K terms

/// The K sort of a list of meta-meta-values of type `elem`.
fn klist_sort(elem: &p4spectec::lang::data::typ::TypKind) -> Result<&'static str, Error> {
    use p4spectec::lang::data::typ::TypKind;
    let TypKind::Var(id, _) = elem else { return Err("a list of named values".to_owned()) };
    Ok(match &*id.node {
        "val" => "Vals",
        "exp" => "Exps",
        "it" => "Its",
        "prem" => "Prems",
        "rul" => "Ruls",
        "clause" => "Clauses",
        "defn" => "Defns",
        "ctor" => "Ctors",
        other => return Err(format!("no K sort for a list of {other}")),
    })
}

/// A K program writes a list without its terminator, which only rules may name.
fn klist(_sort: &str, values: &[Tree]) -> Result<String, Error> {
    let items = values.iter().map(kterm).collect::<Result<Vec<_>, _>>()?;
    Ok(format!("({})", items.join(", ")))
}

/// Prints a meta-meta-value as a K term: a case as `ATOM(args)`, a list as `(x, y)`.
fn kterm(value: &Tree) -> Result<String, Error> {
    use p4spectec::lang::data::typ::TypKind;
    Ok(match &value.node {
        ValueKind::Bool(b) => b.to_string(),
        ValueKind::Num(n) => num(n)?,
        ValueKind::Text(t) => string_literal(t),
        ValueKind::Extern(json) => string_literal(&json.to_string()),
        ValueKind::List(values) => {
            let TypKind::Iter(elem, _) = &value.note else { return Err("a list type".to_owned()) };
            klist(klist_sort(&elem.node)?, values)?
        }
        ValueKind::Opt(opt) => {
            let TypKind::Iter(elem, _) = &value.note else {
                return Err("an option type".to_owned());
            };
            let sort = klist_sort(&elem.node)?;
            match opt {
                Some(value) => klist(sort, std::slice::from_ref(value))?,
                None => klist(sort, &[])?,
            }
        }
        ValueKind::Case(_) => {
            let items = v::items(value)?;
            let atoms = items
                .iter()
                .filter_map(|item| item.as_ref().ok())
                .collect::<Vec<_>>();
            let args = items
                .iter()
                .filter_map(|item| item.as_ref().err())
                .map(|arg| kterm(arg))
                .collect::<Result<Vec<_>, _>>()?;
            // A rule or a clause has no atom of its own
            let name = match (atoms.as_slice(), args.len()) {
                ([atom], _) => atom.to_string(),
                ([], 4) => "rul".to_owned(),
                ([], 3) => "clause".to_owned(),
                _ => return Err(format!("no K constructor for {atoms:?}")),
            };
            if args.is_empty() { name } else { format!("{name}({})", args.join(", ")) }
        }
        _ => return Err("no K term for this value".to_owned()),
    })
}
