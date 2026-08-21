use p4spec_rust::{
    domain::source::Region,
    interface::{ExternError, SpecCall},
    lang::il::ast::Typ,
    runtime::value::{ValueRef, make},
    sim::{architecture::Architecture, ebpf::Ebpf},
};

struct NoCalls;

impl SpecCall for NoCalls {
    fn eval_func(
        &mut self,
        _name: &str,
        _type_args: &[Typ],
        _values: &[ValueRef],
    ) -> Result<ValueRef, ExternError> {
        panic!("unexpected function call")
    }

    fn eval_rel(
        &mut self,
        _name: &str,
        _values: &[ValueRef],
    ) -> Result<Vec<ValueRef>, ExternError> {
        panic!("unexpected relation call")
    }
}

#[test]
fn architecture_control_plane_hooks_default_to_an_error() {
    let architecture = make::text("arch".to_owned(), Region::none());
    let error =
        <Ebpf as Architecture>::register_write(&mut NoCalls, architecture, "main.register", 2, 7)
            .unwrap_err();

    assert!(
        error
            .to_string()
            .contains("register write is not supported")
    );
}
