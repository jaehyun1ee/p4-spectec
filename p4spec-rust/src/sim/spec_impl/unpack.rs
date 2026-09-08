use crate::{
    lang::data::value::{Value, ValueArena, get},
    runner::ExternError,
};

fn failure(error: impl std::fmt::Display) -> ExternError {
    ExternError::Failure(error.to_string())
}

pub fn p4_bool(arena: &ValueArena, value: &Value) -> Result<bool, ExternError> {
    get::matches! { arena,
        value,
        "_B bool" => |values| {
            let [value] = values.as_slice() else {
                return Err(ExternError::Failure(format!(
                    "expected exactly 1 values, got {}",
                    values.len()
                )));
            };
            get::bool(arena, value).map_err(failure)
        },
        _ => Err(ExternError::Failure("expected P4 bool value".to_owned())),
    }
}

pub fn p4_string(arena: &ValueArena, value: &Value) -> Result<String, ExternError> {
    get::matches! { arena,
        value,
        "'\"' text '\"'" => |values| {
            let [value] = values.as_slice() else {
                return Err(ExternError::Failure(format!(
                    "expected exactly 1 values, got {}",
                    values.len()
                )));
            };
            get::text(arena, value).map(str::to_owned).map_err(failure)
        },
        _ => Err(ExternError::Failure("expected P4 string value".to_owned())),
    }
}
