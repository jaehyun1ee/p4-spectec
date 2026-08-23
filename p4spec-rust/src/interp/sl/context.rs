use std::rc::Rc;

use hashbrown::HashMap;

use crate::{
    domain::mixfix::Mixfix,
    interp::common::InterpError,
    lang::{
        il::ast::{DefTypKind, Id, Iter, Var},
        sl::ast::{Def, DefKind},
    },
    runtime::{
        dynamic::{
            envs::ValueEnv,
            var::{Variable, VariableRef},
        },
        dynamic_sl::{
            envs::{FunctionEnv, RelationEnv, TypeDefEnv},
            func::Function,
            rel::Relation,
        },
        r#type::{envs::TypeDefMap, subst::TypeSubstitution, typdef::TypeDef},
        value::{ValueRef, get},
    },
};

// Cursor

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Cursor {
    Global,
    Local,
}

// Context

// Global layer

#[derive(Debug, Default)]
struct Global {
    // Map from syntax ids to type definitions
    type_defs: TypeDefEnv,
    // Map from global variant type ids and notation shapes to source case indices
    variant_case_indices: HashMap<String, HashMap<u64, Vec<usize>>>,
    // Shared builder for fingerprints stored in the variant case index
    variant_shape_hasher: hashbrown::DefaultHashBuilder,
    // Map from relation ids to relations
    relations: RelationEnv,
    // Map from function ids to functions
    functions: FunctionEnv,
}

// Local layer

#[derive(Debug)]
enum Local {
    Empty,
    Relation {
        // Relation name
        id: Id,
        // Input values
        input_values: Rc<[ValueRef]>,
        // Map from variables to values
        values: ValueEnv,
    },
    Function {
        // Function name
        id: Id,
        // Input values
        input_values: Rc<[ValueRef]>,
        // Map from syntax ids to type definitions
        type_defs: TypeDefEnv,
        // Map from function ids to functions
        functions: FunctionEnv,
        // Map from variables to values
        values: ValueEnv,
    },
}

#[derive(Debug)]
enum Undo {
    Value(Variable, Option<ValueRef>),
    TypeDef(String, Option<TypeDef>),
    Function(String, Option<Rc<Function>>),
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ScopeMark {
    generation: u64,
    undo_len: usize,
}

#[derive(Debug)]
pub struct Context {
    deterministic: bool,
    global: Global,
    local: Local,
    value_env_pool: Vec<ValueEnv>,
    undo_pool: Vec<Vec<Undo>>,
    generation: u64,
    undo: Vec<Undo>,
}

impl Context {
    // Global initializer

    pub fn from_spec(deterministic: bool, spec: &[Def]) -> Result<Self, InterpError> {
        let mut context = Self {
            deterministic,
            global: Global::default(),
            local: Local::Empty,
            value_env_pool: Vec::new(),
            undo_pool: Vec::new(),
            generation: 0,
            undo: Vec::new(),
        };
        for definition in spec {
            context.load_definition(&definition.node)?;
        }
        Ok(context)
    }

    fn duplicate(id: &Id, kind: &str) -> InterpError {
        InterpError::new(
            id.span.clone(),
            format!("{kind} `{}` was already defined", id.node),
        )
    }

    fn undefined(id: &Id, kind: &str) -> InterpError {
        InterpError::new(
            id.span.clone(),
            format!("{kind} `{}` is undefined", id.node),
        )
    }

    fn add_type_def_global(&mut self, id: &Id, type_def: TypeDef) -> Result<(), InterpError> {
        if self.global.type_defs.contains_key(&id.node) {
            return Err(Self::duplicate(id, "type"));
        }
        if let TypeDef::Defined(_, def_type) = &type_def
            && let DefTypKind::VariantT(type_cases) = &def_type.node
        {
            let mut case_indices = HashMap::<u64, Vec<usize>>::with_capacity(type_cases.len());
            for (case_index, (not_type, _, _)) in type_cases.iter().enumerate() {
                case_indices
                    .entry(
                        not_type
                            .node
                            .shape_fingerprint(&self.global.variant_shape_hasher),
                    )
                    .or_default()
                    .push(case_index);
            }
            self.global
                .variant_case_indices
                .insert(id.node.clone(), case_indices);
        }
        self.global.type_defs.insert(id.node.clone(), type_def);
        Ok(())
    }

    fn add_relation_global(&mut self, id: &Id, relation: Relation) -> Result<(), InterpError> {
        if self.global.relations.contains_key(&id.node) {
            return Err(Self::duplicate(id, "relation"));
        }
        self.global
            .relations
            .insert(id.node.clone(), Rc::new(relation));
        Ok(())
    }

    fn add_function_global(&mut self, id: &Id, function: Function) -> Result<(), InterpError> {
        if self.global.functions.contains_key(&id.node) {
            return Err(Self::duplicate(id, "function"));
        }
        self.global
            .functions
            .insert(id.node.clone(), Rc::new(function));
        Ok(())
    }

    fn load_definition(&mut self, definition: &DefKind) -> Result<(), InterpError> {
        match definition {
            DefKind::ExternTypD(id, _) => self.add_type_def_global(id, TypeDef::Extern),
            DefKind::TypD(id, type_params, def_type, _) => self.add_type_def_global(
                id,
                TypeDef::Defined(type_params.clone(), Box::new(def_type.clone())),
            ),
            DefKind::VarD(..) => Ok(()),
            DefKind::ExternRelD((id, signature, _, _)) => {
                self.add_relation_global(id, Relation::Extern(signature.clone()))
            }
            DefKind::RelD((id, signature, matches, block, else_block, _)) => self
                .add_relation_global(
                    id,
                    Relation::Defined(
                        signature.clone(),
                        matches.clone(),
                        block.clone(),
                        else_block.clone(),
                    ),
                ),
            DefKind::ExternDecD((id, type_params, params, typ, _)) => self.add_function_global(
                id,
                Function::Extern(type_params.clone(), params.clone(), typ.clone()),
            ),
            DefKind::BuiltinDecD((id, type_params, params, typ, _)) => self.add_function_global(
                id,
                Function::Builtin(type_params.clone(), params.clone(), typ.clone()),
            ),
            DefKind::TableDecD((id, params, typ, rows, _)) => self.add_function_global(
                id,
                Function::Table(params.clone(), typ.clone(), rows.clone()),
            ),
            DefKind::FuncDecD((id, type_params, params, typ, block, else_block, _)) => self
                .add_function_global(
                    id,
                    Function::Defined(
                        type_params.clone(),
                        params.clone(),
                        typ.clone(),
                        block.clone(),
                        else_block.clone(),
                    ),
                ),
        }
    }

    pub fn deterministic(&self) -> bool {
        self.deterministic
    }

    pub fn current_id(&self) -> Option<&Id> {
        match &self.local {
            Local::Empty => None,
            Local::Relation { id, .. } | Local::Function { id, .. } => Some(id),
        }
    }

    // Finders for input values

    pub fn input_values(&self) -> Result<&[ValueRef], InterpError> {
        match &self.local {
            Local::Empty => Err(InterpError::new(
                crate::domain::source::Region::none(),
                "cannot find input values in empty local context",
            )),
            Local::Relation { input_values, .. } | Local::Function { input_values, .. } => {
                Ok(input_values)
            }
        }
    }

    // Finders for values

    pub fn find_value(&self, variable: &Variable) -> Result<&ValueRef, InterpError> {
        self.find_value_by(&variable.id, &variable.iters)
    }

    pub(crate) fn find_value_by(&self, id: &Id, iters: &[Iter]) -> Result<&ValueRef, InterpError> {
        let value = match &self.local {
            Local::Empty => None,
            Local::Relation { values, .. } | Local::Function { values, .. } => {
                values.get(&VariableRef::new(&id.node, iters))
            }
        };
        value.ok_or_else(|| Self::undefined(id, "value"))
    }

    pub fn is_value_bound(&self, variable: &Variable) -> bool {
        self.find_value(variable).is_ok()
    }

    // Finders for type definitions

    pub fn find_type_def(&self, id: &Id) -> Result<&TypeDef, InterpError> {
        self.find_type_def_with_cursor(id)
            .map(|(_cursor, type_def)| type_def)
    }

    pub(crate) fn find_type_def_with_cursor(
        &self,
        id: &Id,
    ) -> Result<(Cursor, &TypeDef), InterpError> {
        if let Local::Function { type_defs, .. } = &self.local
            && let Some(type_def) = type_defs.get(&id.node)
        {
            return Ok((Cursor::Local, type_def));
        }
        self.global
            .type_defs
            .get(&id.node)
            .map(|type_def| (Cursor::Global, type_def))
            .ok_or_else(|| Self::undefined(id, "type"))
    }

    pub fn is_type_def_bound(&self, id: &Id) -> bool {
        self.find_type_def(id).is_ok()
    }

    pub(crate) fn find_global_variant_case_indices<T>(
        &self,
        id: &Id,
        value_case: &Mixfix<T>,
    ) -> Option<&[usize]> {
        self.global
            .variant_case_indices
            .get(&id.node)?
            .get(&value_case.shape_fingerprint(&self.global.variant_shape_hasher))
            .map(Vec::as_slice)
    }

    // Finders for relations

    pub fn find_relation(&self, id: &Id) -> Result<&Rc<Relation>, InterpError> {
        self.global
            .relations
            .get(&id.node)
            .ok_or_else(|| Self::undefined(id, "relation"))
    }

    // Finders for functions

    pub fn find_function(&self, id: &Id) -> Result<(Cursor, &Rc<Function>), InterpError> {
        if let Local::Function { functions, .. } = &self.local
            && let Some(function) = functions.get(&id.node)
        {
            return Ok((Cursor::Local, function));
        }
        self.global
            .functions
            .get(&id.node)
            .map(|function| (Cursor::Global, function))
            .ok_or_else(|| Self::undefined(id, "function"))
    }

    pub fn is_function_bound(&self, id: &Id) -> bool {
        self.find_function(id).is_ok()
    }

    pub(crate) fn local_type_substitution(&self) -> TypeSubstitution {
        let Local::Function { type_defs, .. } = &self.local else {
            return TypeSubstitution::new();
        };
        type_defs
            .iter()
            .filter_map(|(id, type_def)| match type_def {
                TypeDef::Defined(type_params, def_type) if type_params.is_empty() => {
                    let DefTypKind::PlainT(typ) = &def_type.node else {
                        return None;
                    };
                    Some((id.clone(), typ.clone()))
                }
                _ => None,
            })
            .collect()
    }

    pub(crate) fn type_defs(&self) -> TypeDefMap {
        let mut type_defs = self.global.type_defs.clone();
        if let Local::Function {
            type_defs: local, ..
        } = &self.local
        {
            type_defs.extend(local.clone());
        }
        type_defs
    }

    // Adders

    pub fn bind_value(&mut self, variable: Variable, value: ValueRef) -> Result<(), InterpError> {
        let values = match &mut self.local {
            Local::Empty => {
                return Err(InterpError::new(
                    variable.id.span.clone(),
                    "cannot add value to empty local context",
                ));
            }
            Local::Relation { values, .. } | Local::Function { values, .. } => values,
        };
        let previous = values.insert(variable.clone(), value);
        self.undo.push(Undo::Value(variable, previous));
        Ok(())
    }

    pub fn bind_type(&mut self, id: Id, type_def: TypeDef) -> Result<(), InterpError> {
        if self.is_type_def_bound(&id) {
            return Err(Self::duplicate(&id, "type"));
        }
        let Local::Function { type_defs, .. } = &mut self.local else {
            let message = match self.local {
                Local::Empty => "cannot add type to empty local context",
                Local::Relation { .. } => "cannot add type to relation context",
                Local::Function { .. } => unreachable!(),
            };
            return Err(InterpError::new(id.span, message));
        };
        let previous = type_defs.insert(id.node.clone(), type_def);
        self.undo.push(Undo::TypeDef(id.node, previous));
        Ok(())
    }

    pub fn bind_function(&mut self, id: Id, function: Rc<Function>) -> Result<(), InterpError> {
        if self.is_function_bound(&id) {
            return Err(Self::duplicate(&id, "function"));
        }
        let Local::Function { functions, .. } = &mut self.local else {
            let message = match self.local {
                Local::Empty => "cannot add function to empty local context",
                Local::Relation { .. } => "cannot add function to relation context",
                Local::Function { .. } => unreachable!(),
            };
            return Err(InterpError::new(id.span, message));
        };
        let previous = functions.insert(id.node.clone(), function);
        self.undo.push(Undo::Function(id.node, previous));
        Ok(())
    }

    // Constructing a local context

    fn take_value_env(&mut self) -> ValueEnv {
        self.value_env_pool.pop().unwrap_or_default()
    }

    fn recycle_local(&mut self, local: Local) {
        match local {
            Local::Relation { mut values, .. } | Local::Function { mut values, .. } => {
                values.clear();
                self.value_env_pool.push(values);
            }
            Local::Empty => {}
        }
    }

    fn take_undo(&mut self) -> Vec<Undo> {
        self.undo_pool.pop().unwrap_or_default()
    }

    fn recycle_undo(&mut self, mut undo: Vec<Undo>) {
        undo.clear();
        self.undo_pool.push(undo);
    }

    fn replace_local(&mut self, local: Local) {
        let previous = std::mem::replace(&mut self.local, local);
        self.recycle_local(previous);
        self.generation = self.generation.wrapping_add(1);
        self.undo.clear();
    }

    pub fn clear_local(&mut self) {
        self.replace_local(Local::Empty);
    }

    pub fn enter_relation(&mut self, id: Id, input_values: Vec<ValueRef>) {
        let values = self.take_value_env();
        self.replace_local(Local::Relation {
            id,
            input_values: input_values.into(),
            values,
        });
    }

    pub fn enter_function(&mut self, id: Id, input_values: Vec<ValueRef>, type_defs: TypeDefEnv) {
        let values = self.take_value_env();
        self.replace_local(Local::Function {
            id,
            input_values: input_values.into(),
            type_defs,
            functions: HashMap::new(),
            values,
        });
    }

    pub(crate) fn with_function_frame<T>(
        &mut self,
        id: Id,
        input_values: Rc<[ValueRef]>,
        type_defs: TypeDefEnv,
        evaluate: impl FnOnce(&mut Self) -> Result<T, InterpError>,
    ) -> Result<T, InterpError> {
        let values = self.take_value_env();
        let local_previous = std::mem::replace(
            &mut self.local,
            Local::Function {
                id,
                input_values,
                type_defs,
                functions: HashMap::new(),
                values,
            },
        );
        let generation_previous = self.generation;
        let undo = self.take_undo();
        let undo_previous = std::mem::replace(&mut self.undo, undo);
        self.generation = self.generation.wrapping_add(1);
        let result = evaluate(self);
        let local_finished = std::mem::replace(&mut self.local, local_previous);
        self.recycle_local(local_finished);
        self.generation = generation_previous;
        let undo_finished = std::mem::replace(&mut self.undo, undo_previous);
        self.recycle_undo(undo_finished);
        result
    }

    pub(crate) fn with_relation_frame<T>(
        &mut self,
        id: Id,
        input_values: Rc<[ValueRef]>,
        evaluate: impl FnOnce(&mut Self) -> Result<T, InterpError>,
    ) -> Result<T, InterpError> {
        let values = self.take_value_env();
        let local_previous = std::mem::replace(
            &mut self.local,
            Local::Relation {
                id,
                input_values,
                values,
            },
        );
        let generation_previous = self.generation;
        let undo = self.take_undo();
        let undo_previous = std::mem::replace(&mut self.undo, undo);
        self.generation = self.generation.wrapping_add(1);
        let result = evaluate(self);
        let local_finished = std::mem::replace(&mut self.local, local_previous);
        self.recycle_local(local_finished);
        self.generation = generation_previous;
        let undo_finished = std::mem::replace(&mut self.undo, undo_previous);
        self.recycle_undo(undo_finished);
        result
    }

    // Constructing sub-context bindings

    pub fn optional_bindings(
        &self,
        vars: &[Var],
    ) -> Result<Option<Vec<(Variable, ValueRef)>>, InterpError> {
        // First collect the values that are to be iterated over.
        let values = vars
            .iter()
            .map(|(id, _typ, iters)| {
                let mut outer_iters = iters.clone();
                outer_iters.push(Iter::Opt);
                let value = self.find_value_by(id, &outer_iters)?;
                get::opt(value)
                    .map(|value| value.cloned())
                    .map_err(|error| {
                        InterpError::new(value.span.region().clone(), error.to_string())
                    })
            })
            .collect::<Result<Vec<_>, _>>()?;
        // Iteration is valid when all variables agree on their optionality.
        if values.iter().all(Option::is_some) {
            Ok(Some(
                vars.iter()
                    .zip(values)
                    .map(|((id, _typ, iters), value)| {
                        (
                            Variable::new(id.clone(), iters.clone()),
                            value.expect("all optional values were checked"),
                        )
                    })
                    .collect(),
            ))
        } else if values.iter().all(Option::is_none) {
            Ok(None)
        } else {
            Err(InterpError::new(
                crate::domain::source::Region::none(),
                "mismatch in optionality of iterated variables",
            ))
        }
    }

    pub fn list_binding_batches(
        &self,
        vars: &[Var],
    ) -> Result<Vec<Vec<(Variable, ValueRef)>>, InterpError> {
        // First break the values that are to be iterated over into batches.
        let rows = vars
            .iter()
            .map(|(id, _typ, iters)| {
                let mut outer_iters = iters.clone();
                outer_iters.push(Iter::List);
                let value = self.find_value_by(id, &outer_iters)?;
                get::list(value).map(<[ValueRef]>::to_vec).map_err(|error| {
                    InterpError::new(value.span.region().clone(), error.to_string())
                })
            })
            .collect::<Result<Vec<_>, _>>()?;
        let Some(first) = rows.first() else {
            return Ok(Vec::new());
        };
        let width = first.len();
        if rows.iter().any(|row| row.len() != width) {
            return Err(InterpError::new(
                crate::domain::source::Region::none(),
                "cannot transpose a matrix of value batches",
            ));
        }
        let mut batches = vec![Vec::with_capacity(vars.len()); width];
        for ((id, _typ, iters), row) in vars.iter().zip(rows) {
            for (batch, value) in batches.iter_mut().zip(row) {
                batch.push((Variable::new(id.clone(), iters.clone()), value));
            }
        }
        Ok(batches)
    }

    pub fn with_value_bindings<T>(
        &mut self,
        bindings: Vec<(Variable, ValueRef)>,
        evaluate: impl FnOnce(&mut Self) -> Result<T, InterpError>,
    ) -> Result<T, InterpError> {
        let mark = self.mark();
        let result = (|| {
            for (variable, value) in bindings {
                self.bind_value(variable, value)?;
            }
            evaluate(self)
        })();
        self.reset(mark)?;
        result
    }

    pub(crate) fn with_cleared_values<T>(
        &mut self,
        evaluate: impl FnOnce(&mut Self) -> Result<T, InterpError>,
    ) -> Result<T, InterpError> {
        let values_previous = match &mut self.local {
            Local::Empty => {
                return Err(InterpError::new(
                    crate::domain::source::Region::none(),
                    "cannot clear empty local context",
                ));
            }
            Local::Relation { values, .. } | Local::Function { values, .. } => {
                std::mem::take(values)
            }
        };
        let generation_previous = self.generation;
        let undo_previous = std::mem::take(&mut self.undo);
        self.generation = self.generation.wrapping_add(1);
        let result = evaluate(self);
        match &mut self.local {
            Local::Empty => unreachable!("cleared local context changed kind"),
            Local::Relation { values, .. } | Local::Function { values, .. } => {
                *values = values_previous;
            }
        }
        self.generation = generation_previous;
        self.undo = undo_previous;
        result
    }

    pub(crate) fn with_scope<T>(
        &mut self,
        evaluate: impl FnOnce(&mut Self) -> Result<T, InterpError>,
    ) -> Result<T, InterpError> {
        let mark = self.mark();
        let result = evaluate(self);
        self.reset(mark)?;
        result
    }

    // Scope and backtracking

    pub fn mark(&self) -> ScopeMark {
        ScopeMark {
            generation: self.generation,
            undo_len: self.undo.len(),
        }
    }

    pub fn reset(&mut self, mark: ScopeMark) -> Result<(), InterpError> {
        if mark.generation != self.generation {
            return Err(InterpError::new(
                crate::domain::source::Region::none(),
                "scope mark belongs to a different local frame",
            ));
        }
        if mark.undo_len > self.undo.len() {
            return Err(InterpError::new(
                crate::domain::source::Region::none(),
                "scope mark is no longer valid",
            ));
        }
        while self.undo.len() > mark.undo_len {
            let undo = self.undo.pop().expect("undo length was checked");
            match undo {
                Undo::Value(variable, previous) => {
                    let values = match &mut self.local {
                        Local::Relation { values, .. } | Local::Function { values, .. } => values,
                        Local::Empty => unreachable!("a valid mark has a local frame"),
                    };
                    match previous {
                        Some(value) => {
                            values.insert(variable, value);
                        }
                        None => {
                            values.remove(&variable);
                        }
                    }
                }
                Undo::TypeDef(id, previous) => {
                    let Local::Function { type_defs, .. } = &mut self.local else {
                        unreachable!("type definitions only exist in function frames")
                    };
                    match previous {
                        Some(type_def) => {
                            type_defs.insert(id, type_def);
                        }
                        None => {
                            type_defs.remove(&id);
                        }
                    }
                }
                Undo::Function(id, previous) => {
                    let Local::Function { functions, .. } = &mut self.local else {
                        unreachable!("local functions only exist in function frames")
                    };
                    match previous {
                        Some(function) => {
                            functions.insert(id, function);
                        }
                        None => {
                            functions.remove(&id);
                        }
                    }
                }
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        domain::{
            mixfix::Mixfix,
            source::{Region, Spanned},
        },
        lang::{il::ast::DefTypKind, sl::ast::DefKind},
        runtime::value::make,
    };

    fn id(name: impl Into<String>) -> Id {
        Spanned::new(name.into(), Region::none())
    }

    #[test]
    fn global_variant_index_preserves_source_order_for_equal_shapes() {
        let typ = || Spanned::new(crate::lang::il::ast::TypKind::BoolT, Region::none());
        let origin = || Spanned::new((id("Origin"), Vec::new()), Region::none());
        let case = |notation| (Spanned::new(notation, Region::none()), origin(), Vec::new());
        let cases = vec![
            case(Mixfix::Seq(vec![Mixfix::Arg(typ())])),
            case(Mixfix::Seq(vec![Mixfix::Seq(vec![Mixfix::Arg(typ())])])),
            case(Mixfix::Seq(vec![Mixfix::Arg(typ())])),
        ];
        let spec = vec![Spanned::new(
            DefKind::TypD(
                id("Choice"),
                Vec::new(),
                Spanned::new(DefTypKind::VariantT(cases), Region::none()),
                Vec::new(),
            ),
            Region::none(),
        )];
        let context = Context::from_spec(false, &spec).expect("valid variant definition");
        let value_case = Mixfix::Seq(vec![Mixfix::Arg(make::bool(true, Region::none()))]);

        assert_eq!(
            context.find_global_variant_case_indices(&id("Choice"), &value_case),
            Some(&[0, 2][..])
        );
    }

    #[test]
    fn completed_frames_reuse_value_environment_capacity() {
        let mut context = Context::from_spec(false, &[]).expect("valid empty spec");
        context
            .with_relation_frame(id("first"), Vec::new().into(), |context| {
                for index in 0..64 {
                    context.bind_value(
                        Variable::new(id(format!("value-{index}")), Vec::new()),
                        make::bool(true, Region::none()),
                    )?;
                }
                Ok(())
            })
            .unwrap();
        let capacity = context.value_env_pool[0].capacity();
        assert!(capacity >= 64);

        context
            .with_relation_frame(id("second"), Vec::new().into(), |context| {
                let Local::Relation { values, .. } = &context.local else {
                    panic!("expected relation frame")
                };
                assert!(values.capacity() >= capacity);
                Ok(())
            })
            .unwrap();
        assert_eq!(context.value_env_pool.len(), 1);
    }

    #[test]
    fn completed_frames_reuse_undo_log_capacity() {
        let mut context = Context::from_spec(false, &[]).expect("valid empty spec");
        context
            .with_relation_frame(id("first"), Vec::new().into(), |context| {
                for index in 0..64 {
                    context.bind_value(
                        Variable::new(id(format!("value-{index}")), Vec::new()),
                        make::bool(true, Region::none()),
                    )?;
                }
                Ok(())
            })
            .unwrap();
        let capacity = context.undo_pool[0].capacity();
        assert!(capacity >= 64);

        context
            .with_relation_frame(id("second"), Vec::new().into(), |context| {
                assert!(context.undo.capacity() >= capacity);
                Ok(())
            })
            .unwrap();
        assert_eq!(context.undo_pool.len(), 1);
    }

    #[test]
    fn local_frames_share_the_input_container() {
        let inputs: Rc<[ValueRef]> = vec![make::bool(true, Region::none())].into();
        let mut context = Context::from_spec(false, &[]).expect("valid empty spec");
        context
            .with_relation_frame(id("relation"), Rc::clone(&inputs), |context| {
                let Local::Relation { input_values, .. } = &context.local else {
                    panic!("expected relation frame")
                };
                assert!(Rc::ptr_eq(&inputs, input_values));
                Ok(())
            })
            .unwrap();
        context
            .with_function_frame(
                id("function"),
                Rc::clone(&inputs),
                TypeDefEnv::new(),
                |context| {
                    let Local::Function { input_values, .. } = &context.local else {
                        panic!("expected function frame")
                    };
                    assert!(Rc::ptr_eq(&inputs, input_values));
                    Ok(())
                },
            )
            .unwrap();
    }
}
