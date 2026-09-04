//! Transactional state accumulated while analyzing bindings

use std::ops::{Deref, DerefMut};

use crate::{
    lang::{
        common::{Id, ds::set::IdSet, source::Span},
        data::typ,
        il::ast,
    },
    phrase,
    runtime::{
        env::TDEnv,
        envs::elab::{MEnv, VEnv},
        typdef::TypeDef,
    },
};

use super::super::{AlgoError, AlgoErrorKind};

/// Environments and fresh state threaded through binding analysis
#[derive(Debug)]
pub struct Context {
    pub(crate) frees: IdSet,
    pub(crate) venv: VEnv,
    pub(crate) tdenv: TDEnv,
    pub(crate) menv: MEnv,
    undo: Vec<Undo>,
    checkpoints: Vec<usize>,
}

#[derive(Debug)]
enum Undo {
    AddFree(Id),
    AddBound(Id),
}

/// A checkpoint in the binding context
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct Checkpoint {
    depth: usize,
    undo_len: usize,
}

/// A scope that rolls back its context changes when dropped
pub struct Scope<'a> {
    ctx: &'a mut Context,
    checkpoint: Checkpoint,
}

impl Deref for Scope<'_> {
    type Target = Context;

    fn deref(&self) -> &Self::Target {
        self.ctx
    }
}

impl DerefMut for Scope<'_> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        self.ctx
    }
}

impl Drop for Scope<'_> {
    fn drop(&mut self) {
        self.ctx.rollback_scope(self.checkpoint);
    }
}

impl Context {
    // == Constructors

    pub fn new() -> Self {
        let mut menv = MEnv::new();
        for (name, typ) in [
            ("bool", typ::make::bool()),
            ("nat", typ::make::nat()),
            ("int", typ::make::int()),
            ("text", typ::make::text()),
        ] {
            let id = phrase!(node: name.to_owned(), span: Span::default());
            menv.insert(id, typ);
        }
        Self {
            frees: IdSet::new(),
            venv: VEnv::new(),
            tdenv: TDEnv::new(),
            menv,
            undo: vec![],
            checkpoints: vec![],
        }
    }

    // == Transactions

    fn assert_checkpoint(&self, checkpoint: Checkpoint) {
        assert_eq!(checkpoint.depth + 1, self.checkpoints.len());
        assert_eq!(Some(&checkpoint.undo_len), self.checkpoints.last());
    }

    fn checkpoint(&mut self) -> Checkpoint {
        let checkpoint = Checkpoint {
            depth: self.checkpoints.len(),
            undo_len: self.undo.len(),
        };
        self.checkpoints.push(checkpoint.undo_len);
        checkpoint
    }

    fn rollback(&mut self, checkpoint: Checkpoint) {
        self.assert_checkpoint(checkpoint);
        self.checkpoints.pop();
        while self.undo.len() > checkpoint.undo_len {
            match self.undo.pop().expect("recorded binding change") {
                Undo::AddFree(id) => {
                    self.frees.take(&id).expect("recorded free binding");
                }
                Undo::AddBound(id) => {
                    self.venv.remove(&id).expect("recorded bound binding");
                }
            }
        }
    }

    fn rollback_scope(&mut self, checkpoint: Checkpoint) {
        if std::thread::panicking() && self.checkpoints.len() > checkpoint.depth + 1 {
            self.checkpoints.truncate(checkpoint.depth + 1);
        }
        self.rollback(checkpoint);
    }

    pub fn scope(&mut self) -> Scope<'_> {
        let checkpoint = self.checkpoint();
        Scope {
            ctx: self,
            checkpoint,
        }
    }

    // == Adders

    pub fn add_free(&mut self, id: Id) {
        if self.frees.insert(id.clone()) && !self.checkpoints.is_empty() {
            self.undo.push(Undo::AddFree(id));
        }
    }

    pub fn add_frees(&mut self, ids: &IdSet) {
        for id in ids.iter() {
            self.add_free(id.clone());
        }
    }

    pub fn add_bounds(&mut self, venv: &VEnv) {
        for (id, dim) in venv.iter() {
            if !self.venv.contains_key(id) {
                self.venv.insert(id.clone(), dim.clone());
                if !self.checkpoints.is_empty() {
                    self.undo.push(Undo::AddBound(id.clone()));
                }
            }
        }
    }

    // == Finders

    pub fn find_typdef_opt(&self, id: &Id) -> Option<&TypeDef> {
        self.tdenv.get(id)
    }

    pub fn find_typdef(&self, id: &Id) -> Result<&TypeDef, AlgoError> {
        self.find_typdef_opt(id)
            .ok_or_else(|| AlgoError::new(AlgoErrorKind::UndefinedType, id.span.clone()))
    }

    // == Definition loading

    pub fn load_def(&mut self, def: &ast::Def) {
        match &def.node {
            ast::DefKind::ExternTyp(typ_def) => {
                self.tdenv.insert(typ_def.id.clone(), TypeDef::Extern);
            }
            ast::DefKind::Typ(typ_def) => {
                let value =
                    TypeDef::Defined(typ_def.tparams.clone(), Box::new(typ_def.def_typ.clone()));
                self.tdenv.insert(typ_def.id.clone(), value);
            }
            ast::DefKind::Var(var_def) => {
                self.menv.insert(var_def.id.clone(), var_def.typ.clone());
            }
            _ => {}
        }
    }

    pub fn load_spec(&mut self, spec: &ast::Spec) {
        for def in spec {
            self.load_def(def);
        }
    }
}
