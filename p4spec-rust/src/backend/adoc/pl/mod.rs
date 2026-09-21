//! AsciiDoc rendering for prose-language definitions

pub mod document;
pub mod fallthrough;
mod render;
pub mod utils;

pub use render::{
    render_builtin_func_def, render_builtin_func_def_with_anchor, render_def,
    render_def_with_anchor, render_defined_func_def, render_defined_func_def_with_anchor,
    render_defined_rel_def, render_defined_rel_def_dispatch,
    render_defined_rel_def_dispatch_with_anchor, render_defined_rel_def_with_anchor, render_defs,
    render_extern_func_def, render_extern_func_def_with_anchor, render_extern_rel_def,
    render_extern_rel_def_with_anchor, render_func_header, render_func_header_with_anchor,
    render_func_title, render_func_title_with_anchor, render_rel_title,
    render_rel_title_with_anchor, render_rulegroup, render_rulegroup_else,
    render_rulegroup_else_with_anchor, render_rulegroup_with_anchor, render_spec,
    render_table_func_def, render_table_func_def_with_anchor,
};
