use num_bigint::BigInt;

use crate::{
    domain::{
        atom::Atom,
        mixfix::{Mixfix, Mixop},
        source::{Region, Spanned},
    },
    interface::SpecCall,
    lang::{il::ast::Typ, xl::num},
    runtime::{
        r#type::typ::make as make_type,
        value::{ValueRef, get, make},
    },
};

use super::{SimError, core::Bits};

pub struct Spec<'a> {
    calls: &'a mut dyn SpecCall,
}

impl<'a> Spec<'a> {
    pub fn new(calls: &'a mut dyn SpecCall) -> Self {
        Self { calls }
    }

    fn func(&mut self, name: &str, values: &[ValueRef]) -> Result<ValueRef, SimError> {
        self.calls.eval_func(name, &[], values).map_err(Into::into)
    }

    fn rel<const N: usize>(
        &mut self,
        name: &str,
        values: &[ValueRef],
    ) -> Result<[ValueRef; N], SimError> {
        let outputs = self.calls.eval_rel(name, values)?;
        let actual = outputs.len();
        outputs.try_into().map_err(|_| {
            SimError::message(format!("{name} returned {actual} values; expected {N}"))
        })
    }

    pub fn write_value_from_bits(
        &mut self,
        target: &ValueRef,
        variable_size: usize,
        bits: &[bool],
    ) -> Result<ValueRef, SimError> {
        let bit_type = named_type("bit");
        let bit_list_type = make_type::list_type(bit_type);
        let bit_values = bits
            .iter()
            .map(|bit| make::bool(*bit, Region::none()))
            .collect();
        self.func(
            "write_value_from_bits",
            &[
                target.clone(),
                make::nat(BigInt::from(variable_size), Region::none()),
                make::list(&bit_list_type, bit_values, Region::none()),
            ],
        )
    }

    pub fn write_bits_from_value(&mut self, source: &ValueRef) -> Result<Bits, SimError> {
        let value = self.func("write_bits_from_value", std::slice::from_ref(source))?;
        get::list(&value)
            .map_err(value_error)?
            .iter()
            .map(|value| get::bool(value).map_err(value_error))
            .collect()
    }

    pub fn bitacc_range(
        &mut self,
        base: &ValueRef,
        high: &ValueRef,
        low: &ValueRef,
    ) -> Result<ValueRef, SimError> {
        self.func(
            "bitacc_range_op",
            &[base.clone(), high.clone(), low.clone()],
        )
    }

    pub fn default_value(&mut self, typ: &ValueRef) -> Result<ValueRef, SimError> {
        self.func("default", std::slice::from_ref(typ))
    }

    pub fn cast(&mut self, typ: &ValueRef, value: &ValueRef) -> Result<ValueRef, SimError> {
        self.func("cast_op", &[typ.clone(), value.clone()])
    }

    pub fn sizeof_min_bits(&mut self, typ: &ValueRef) -> Result<BigInt, SimError> {
        self.numeric_func("sizeof_minSizeInBits'", typ)
    }

    pub fn sizeof_max_bits(&mut self, typ: &ValueRef) -> Result<BigInt, SimError> {
        self.numeric_func("sizeof_maxSizeInBits'", typ)
    }

    fn numeric_func(&mut self, name: &str, value: &ValueRef) -> Result<BigInt, SimError> {
        let value = self.func(name, std::slice::from_ref(value))?;
        match get::num(&value).map_err(value_error)? {
            num::T::Nat(value) | num::T::Int(value) => Ok(value.clone()),
        }
    }

    pub fn key_interface(
        &mut self,
        table: &ValueRef,
    ) -> Result<Vec<(ValueRef, ValueRef, ValueRef)>, SimError> {
        let value = self.func("key_interface_of_tableObject", std::slice::from_ref(table))?;
        get::list(&value)
            .map_err(value_error)?
            .iter()
            .map(|value| {
                let (first, second, third) =
                    get::three(get::tuple(value).map_err(value_error)?).map_err(value_error)?;
                Ok((first.clone(), second.clone(), third.clone()))
            })
            .collect()
    }

    pub fn table_add_entry(
        &mut self,
        context: &ValueRef,
        table: &ValueRef,
        priority: &ValueRef,
        keys: &ValueRef,
        action: &ValueRef,
    ) -> Result<Option<ValueRef>, SimError> {
        let value = self.func(
            "tableObject_add_entry",
            &[
                context.clone(),
                table.clone(),
                priority.clone(),
                keys.clone(),
                action.clone(),
            ],
        )?;
        Ok(get::opt(&value).map_err(value_error)?.cloned())
    }

    pub fn table_add_default_action(
        &mut self,
        context: &ValueRef,
        table: &ValueRef,
        action: &ValueRef,
    ) -> Result<ValueRef, SimError> {
        self.func(
            "tableObject_add_default_action",
            &[context.clone(), table.clone(), action.clone()],
        )
    }

    pub fn find_object_qualified(
        &mut self,
        architecture: &ValueRef,
        object_id: &ValueRef,
    ) -> Result<Option<ValueRef>, SimError> {
        self.find_optional("find_object_qualified_e", architecture, object_id)
    }

    pub fn find_object_unqualified(
        &mut self,
        architecture: &ValueRef,
        object_id: &ValueRef,
    ) -> Result<Option<ValueRef>, SimError> {
        self.find_optional("find_object_unqualified_e", architecture, object_id)
    }

    fn find_optional(
        &mut self,
        name: &str,
        architecture: &ValueRef,
        object_id: &ValueRef,
    ) -> Result<Option<ValueRef>, SimError> {
        let value = self.func(name, &[architecture.clone(), object_id.clone()])?;
        Ok(get::opt(&value).map_err(value_error)?.cloned())
    }

    pub fn update_object_qualified(
        &mut self,
        architecture: &ValueRef,
        object_id: &ValueRef,
        object: &ValueRef,
    ) -> Result<ValueRef, SimError> {
        self.func(
            "update_object_qualified_e",
            &[architecture.clone(), object_id.clone(), object.clone()],
        )
    }

    pub fn update_object_unqualified(
        &mut self,
        architecture: &ValueRef,
        object_id: &ValueRef,
        object: &ValueRef,
    ) -> Result<ValueRef, SimError> {
        self.func(
            "update_object_unqualified_e",
            &[architecture.clone(), object_id.clone(), object.clone()],
        )
    }

    pub fn find_object_state(
        &mut self,
        architecture: &ValueRef,
        object_id: &ValueRef,
    ) -> Result<ValueRef, SimError> {
        self.required_optional(
            "find_objectState_e",
            &[architecture.clone(), object_id.clone()],
        )
    }

    pub fn update_object_state(
        &mut self,
        architecture: &ValueRef,
        object_id: &ValueRef,
        state: &ValueRef,
    ) -> Result<ValueRef, SimError> {
        self.required_optional(
            "update_objectState_e",
            &[architecture.clone(), object_id.clone(), state.clone()],
        )
    }

    pub fn find_arch_state(&mut self, architecture: &ValueRef) -> Result<ValueRef, SimError> {
        self.func("find_archState_e", std::slice::from_ref(architecture))
    }

    pub fn update_arch_state(
        &mut self,
        architecture: &ValueRef,
        state: &ValueRef,
    ) -> Result<ValueRef, SimError> {
        self.func("update_archState_e", &[architecture.clone(), state.clone()])
    }

    fn required_optional(&mut self, name: &str, values: &[ValueRef]) -> Result<ValueRef, SimError> {
        let value = self.func(name, values)?;
        get::opt(&value)
            .map_err(value_error)?
            .cloned()
            .ok_or_else(|| SimError::message(format!("{name} returned none")))
    }

    pub fn find_type(
        &mut self,
        cursor: &ValueRef,
        context: &ValueRef,
        name: &str,
    ) -> Result<ValueRef, SimError> {
        self.func(
            "find_type_e",
            &[
                cursor.clone(),
                context.clone(),
                make::text(name.to_owned(), Region::none()),
            ],
        )
    }

    pub fn find_var_value(
        &mut self,
        cursor: &ValueRef,
        context: &ValueRef,
        name: &str,
    ) -> Result<ValueRef, SimError> {
        self.func(
            "find_var_value_t",
            &[prefixed_name(name)?, cursor.clone(), context.clone()],
        )
    }

    pub fn find_var(
        &mut self,
        cursor: &ValueRef,
        context: &ValueRef,
        name: &str,
    ) -> Result<ValueRef, SimError> {
        self.func(
            "find_var_e",
            &[prefixed_name(name)?, cursor.clone(), context.clone()],
        )
    }

    pub fn substitute_type(
        &mut self,
        cursor: &ValueRef,
        context: &ValueRef,
        typ: &ValueRef,
    ) -> Result<ValueRef, SimError> {
        self.func(
            "subst_type_e",
            &[cursor.clone(), context.clone(), typ.clone()],
        )
    }

    pub fn lvalue_read(
        &mut self,
        cursor: &ValueRef,
        context: &ValueRef,
        architecture: &ValueRef,
        reference: &ValueRef,
    ) -> Result<ValueRef, SimError> {
        let [value] = self.rel::<1>(
            "Lvalue_read",
            &[
                cursor.clone(),
                context.clone(),
                architecture.clone(),
                reference.clone(),
            ],
        )?;
        Ok(value)
    }

    pub fn lvalue_write(
        &mut self,
        cursor: &ValueRef,
        context: &ValueRef,
        architecture: &ValueRef,
        reference: &ValueRef,
        value: &ValueRef,
    ) -> Result<ValueRef, SimError> {
        let [context] = self.rel::<1>(
            "Lvalue_write",
            &[
                cursor.clone(),
                context.clone(),
                architecture.clone(),
                reference.clone(),
                value.clone(),
            ],
        )?;
        Ok(context)
    }

    pub fn ebpf_init_packet_in(
        &mut self,
        context: &ValueRef,
        architecture: &ValueRef,
        packet_state: &ValueRef,
    ) -> Result<(ValueRef, ValueRef), SimError> {
        let [context, architecture] = self.rel::<2>(
            "EBPF_init_packet_in",
            &[context.clone(), architecture.clone(), packet_state.clone()],
        )?;
        Ok((context, architecture))
    }

    pub fn ebpf_init(&mut self, program: &ValueRef) -> Result<(ValueRef, ValueRef), SimError> {
        let [context, architecture] = self.rel::<2>("EBPF_init", std::slice::from_ref(program))?;
        Ok((context, architecture))
    }

    pub fn ebpf_init_globals(
        &mut self,
        context: &ValueRef,
        architecture: &ValueRef,
    ) -> Result<ValueRef, SimError> {
        let [context] = self.rel::<1>(
            "EBPF_init_globals",
            &[context.clone(), architecture.clone()],
        )?;
        Ok(context)
    }

    pub fn ebpf_parse(
        &mut self,
        context: &ValueRef,
        architecture: &ValueRef,
    ) -> Result<(ValueRef, ValueRef, ValueRef), SimError> {
        self.ebpf_stage("EBPF_parse", context, architecture)
    }

    pub fn ebpf_filter(
        &mut self,
        context: &ValueRef,
        architecture: &ValueRef,
    ) -> Result<(ValueRef, ValueRef, ValueRef), SimError> {
        self.ebpf_stage("EBPF_filter", context, architecture)
    }

    fn ebpf_stage(
        &mut self,
        name: &str,
        context: &ValueRef,
        architecture: &ValueRef,
    ) -> Result<(ValueRef, ValueRef, ValueRef), SimError> {
        let [context, architecture, result] =
            self.rel::<3>(name, &[context.clone(), architecture.clone()])?;
        Ok((context, architecture, result))
    }

    pub fn psa_init(&mut self, program: &ValueRef) -> Result<(ValueRef, ValueRef), SimError> {
        let [context, architecture] = self.rel::<2>("PSA_init", std::slice::from_ref(program))?;
        Ok((context, architecture))
    }

    pub fn psa_init_packet(
        &mut self,
        ingress: bool,
        output: bool,
        context: &ValueRef,
        architecture: &ValueRef,
        packet_state: &ValueRef,
    ) -> Result<(ValueRef, ValueRef), SimError> {
        let direction = if ingress { "ingress" } else { "egress" };
        let kind = if output { "out" } else { "in" };
        let name = format!("PSA_{direction}_init_packet_{kind}");
        let [context, architecture] = self.rel::<2>(
            &name,
            &[context.clone(), architecture.clone(), packet_state.clone()],
        )?;
        Ok((context, architecture))
    }

    pub fn psa_init_metadata(
        &mut self,
        ingress: bool,
        context: &ValueRef,
        architecture: &ValueRef,
        port: i64,
        path: &str,
        class_of_service: i32,
        instance: i32,
    ) -> Result<ValueRef, SimError> {
        let direction = if ingress { "ingress" } else { "egress" };
        let mut values = vec![
            context.clone(),
            architecture.clone(),
            make::int(BigInt::from(port), Region::none()),
            make::text(path.to_owned(), Region::none()),
        ];
        if !ingress {
            values.extend([
                make::int(BigInt::from(class_of_service), Region::none()),
                make::int(BigInt::from(instance), Region::none()),
            ]);
        }
        let [context] = self.rel::<1>(&format!("PSA_{direction}_init_metadata"), &values)?;
        Ok(context)
    }

    pub fn psa_init_globals(
        &mut self,
        ingress: bool,
        context: &ValueRef,
        architecture: &ValueRef,
        port: i64,
    ) -> Result<ValueRef, SimError> {
        let direction = if ingress { "ingress" } else { "egress" };
        let [context] = self.rel::<1>(
            &format!("PSA_{direction}_init_globals"),
            &[
                context.clone(),
                architecture.clone(),
                make::int(BigInt::from(port), Region::none()),
            ],
        )?;
        Ok(context)
    }

    pub fn psa_stage(
        &mut self,
        ingress: bool,
        stage: &str,
        context: &ValueRef,
        architecture: &ValueRef,
    ) -> Result<(ValueRef, ValueRef, ValueRef), SimError> {
        let direction = if ingress { "ingress" } else { "egress" };
        let name = if stage.is_empty() {
            format!("PSA_{direction}")
        } else {
            format!("PSA_{direction}_{stage}")
        };
        let [context, architecture, result] =
            self.rel::<3>(&name, &[context.clone(), architecture.clone()])?;
        Ok((context, architecture, result))
    }
}

pub fn local_cursor() -> Result<ValueRef, SimError> {
    atom_case("cursor", Atom::Keyword("LOCAL".to_owned()))
}

pub fn global_cursor() -> Result<ValueRef, SimError> {
    atom_case("cursor", Atom::Keyword("GLOBAL".to_owned()))
}

pub fn prefixed_name(name: &str) -> Result<ValueRef, SimError> {
    let mixop = Mixfix::Seq(vec![
        Mixfix::Atom(Spanned::new(Atom::Tag("BARE".to_owned()), Region::none())),
        Mixfix::Arg(()),
    ]);
    case_value(
        "prefixedNameIR",
        mixop,
        [make::text(name.to_owned(), Region::none())],
    )
}

pub fn storage_reference(parts: &[&str]) -> Result<ValueRef, SimError> {
    let Some((first, rest)) = parts.split_first() else {
        return Err(SimError::message("empty storage reference"));
    };
    let mut reference = prefixed_name(first)?;
    for part in rest {
        reference = case_value(
            "storageReference",
            Mixfix::Seq(vec![
                Mixfix::Arg(()),
                Mixfix::Atom(Spanned::new(Atom::Operator(".".to_owned()), Region::none())),
                Mixfix::Arg(()),
            ]),
            [reference, make::text((*part).to_owned(), Region::none())],
        )?;
    }
    Ok(reference)
}

fn atom_case(type_name: &str, atom: Atom) -> Result<ValueRef, SimError> {
    case_value(
        type_name,
        Mixfix::Atom(Spanned::new(atom, Region::none())),
        [],
    )
}

fn case_value(
    type_name: &str,
    mixop: Mixop,
    arguments: impl IntoIterator<Item = ValueRef>,
) -> Result<ValueRef, SimError> {
    let value_case =
        Mixop::fill(&mixop, arguments).map_err(|error| SimError::message(error.to_string()))?;
    Ok(make::case(
        &named_type(type_name),
        value_case,
        Region::none(),
    ))
}

fn named_type(name: &str) -> Typ {
    make_type::var_type(Spanned::new(name.to_owned(), Region::none()), Vec::new())
}

fn value_error(error: impl ToString) -> SimError {
    SimError::message(error.to_string())
}
