use proc_macro::TokenStream;
use proc_macro2::TokenStream as TokenStream2;
use quote::{format_ident, quote};
use syn::spanned::Spanned;
use syn::{parse_macro_input, FnArg, ItemFn, LitInt, LitStr, ReturnType, Type};

#[derive(Clone, Copy)]
enum BuiltinKind {
    Builtin,
    Interpreter,
    PreEval,
}

/// Infer R function name from a Rust ident, stripping a prefix and replacing `_` with `.`.
fn infer_r_name(ident: &str, prefix: &str) -> String {
    let base = ident.strip_prefix(prefix).unwrap_or(ident);
    base.replace('_', ".")
}

/// Create a valid Rust identifier from an R function name.
fn r_name_to_ident(name: &str) -> String {
    name.replace('.', "_")
        .replace('<', "_lt_")
        .replace('>', "_gt_")
        .replace('-', "_")
}

/// Shared code generation for all three builtin macro variants.
///
/// Emits: `#[doc(alias)] fn ...` + registry entry + alias entries.
fn emit_builtin_registration(
    input: &ItemFn,
    attr_args: BuiltinAttr,
    kind: BuiltinKind,
    prefix: &str,
    reg_prefix: &str,
) -> TokenStream2 {
    let fn_name = &input.sig.ident;
    let r_name = attr_args
        .name
        .unwrap_or_else(|| infer_r_name(&fn_name.to_string(), prefix));
    let min_args = attr_args.min_args as usize;
    let implementation = kind.registry_ctor(fn_name);

    let reg_name = format_ident!("__{}_{}", reg_prefix, fn_name.to_string().to_uppercase());

    let alias_regs = attr_args.names.iter().enumerate().map(|(i, alias)| {
        let alias_reg = format_ident!(
            "{}_{}_ALIAS_{}",
            reg_name,
            fn_name.to_string().to_uppercase(),
            i
        );
        quote! {
            #[linkme::distributed_slice(crate::interpreter::builtins::BUILTIN_REGISTRY)]
            static #alias_reg: crate::interpreter::builtins::BuiltinDescriptor =
                crate::interpreter::builtins::BuiltinDescriptor {
                    name: #alias,
                    implementation: #implementation,
                    min_args: #min_args,
                };
        }
    });

    quote! {
        #[doc(alias = #r_name)]
        #input

        #[linkme::distributed_slice(crate::interpreter::builtins::BUILTIN_REGISTRY)]
        static #reg_name: crate::interpreter::builtins::BuiltinDescriptor =
            crate::interpreter::builtins::BuiltinDescriptor {
                name: #r_name,
                implementation: #implementation,
                min_args: #min_args,
            };

        #(#alias_regs)*
    }
}

fn validate_signature(input: &ItemFn, kind: BuiltinKind) -> syn::Result<()> {
    let expected_len = match kind {
        BuiltinKind::Builtin | BuiltinKind::PreEval => 2,
        BuiltinKind::Interpreter => 3,
    };

    if input.sig.inputs.len() != expected_len {
        return Err(syn::Error::new(
            input.sig.inputs.span(),
            format!(
                "{} handlers must take exactly {} parameter(s)",
                kind.label(),
                expected_len
            ),
        ));
    }

    for (index, arg) in input.sig.inputs.iter().enumerate() {
        let FnArg::Typed(arg) = arg else {
            return Err(syn::Error::new(
                arg.span(),
                format!("{} handlers cannot take a receiver", kind.label()),
            ));
        };

        if !signature_arg_matches(kind, index, &arg.ty) {
            return Err(syn::Error::new(
                arg.ty.span(),
                format!(
                    "{} parameter {} must be {}",
                    kind.label(),
                    index + 1,
                    expected_parameter_description(kind, index)
                ),
            ));
        }
    }

    validate_return_type(&input.sig.output, kind)
}

fn signature_arg_matches(kind: BuiltinKind, index: usize, ty: &Type) -> bool {
    match (kind, index) {
        (BuiltinKind::Builtin, 0) | (BuiltinKind::Interpreter, 0) => {
            is_ref_to_slice_of_named(ty, "RValue")
        }
        (BuiltinKind::Builtin, 1) | (BuiltinKind::Interpreter, 1) => {
            is_ref_to_slice_of_string_rvalue_pairs(ty)
        }
        (BuiltinKind::Interpreter, 2) | (BuiltinKind::PreEval, 1) => {
            is_ref_to_named(ty, "Environment")
        }
        (BuiltinKind::PreEval, 0) => is_ref_to_slice_of_named(ty, "Arg"),
        _ => false,
    }
}

fn expected_parameter_description(kind: BuiltinKind, index: usize) -> &'static str {
    match (kind, index) {
        (BuiltinKind::Builtin, 0) | (BuiltinKind::Interpreter, 0) => "`&[RValue]`",
        (BuiltinKind::Builtin, 1) | (BuiltinKind::Interpreter, 1) => "`&[(String, RValue)]`",
        (BuiltinKind::Interpreter, 2) | (BuiltinKind::PreEval, 1) => "`&Environment`",
        (BuiltinKind::PreEval, 0) => "`&[Arg]`",
        _ => "the expected builtin parameter type",
    }
}

fn validate_return_type(output: &ReturnType, kind: BuiltinKind) -> syn::Result<()> {
    match output {
        ReturnType::Type(_, ty) if is_result_of_named(ty, "RValue", "RError") => Ok(()),
        _ => Err(syn::Error::new(
            output.span(),
            format!(
                "{} handlers must return `Result<RValue, RError>`",
                kind.label()
            ),
        )),
    }
}

fn is_ref_to_slice_of_named(ty: &Type, name: &str) -> bool {
    match ty {
        Type::Reference(reference) => match reference.elem.as_ref() {
            Type::Slice(slice) => type_ends_with(slice.elem.as_ref(), name),
            _ => false,
        },
        _ => false,
    }
}

fn is_ref_to_slice_of_string_rvalue_pairs(ty: &Type) -> bool {
    match ty {
        Type::Reference(reference) => match reference.elem.as_ref() {
            Type::Slice(slice) => match slice.elem.as_ref() {
                Type::Tuple(tuple) if tuple.elems.len() == 2 => {
                    type_ends_with(&tuple.elems[0], "String")
                        && type_ends_with(&tuple.elems[1], "RValue")
                }
                _ => false,
            },
            _ => false,
        },
        _ => false,
    }
}

fn is_ref_to_named(ty: &Type, name: &str) -> bool {
    match ty {
        Type::Reference(reference) => type_ends_with(reference.elem.as_ref(), name),
        _ => false,
    }
}

fn is_result_of_named(ty: &Type, ok_name: &str, err_name: &str) -> bool {
    let Type::Path(type_path) = ty else {
        return false;
    };
    let Some(segment) = type_path.path.segments.last() else {
        return false;
    };
    if segment.ident != "Result" {
        return false;
    }
    let syn::PathArguments::AngleBracketed(args) = &segment.arguments else {
        return false;
    };
    if args.args.len() != 2 {
        return false;
    }

    let mut args_iter = args.args.iter();
    let Some(syn::GenericArgument::Type(ok_ty)) = args_iter.next() else {
        return false;
    };
    let Some(syn::GenericArgument::Type(err_ty)) = args_iter.next() else {
        return false;
    };

    type_ends_with(ok_ty, ok_name) && type_ends_with(err_ty, err_name)
}

fn type_ends_with(ty: &Type, name: &str) -> bool {
    match ty {
        Type::Path(type_path) => type_path
            .path
            .segments
            .last()
            .map(|segment| segment.ident == name)
            .unwrap_or(false),
        _ => false,
    }
}

impl BuiltinKind {
    fn label(self) -> &'static str {
        match self {
            BuiltinKind::Builtin => "`#[builtin]`",
            BuiltinKind::Interpreter => "`#[interpreter_builtin]`",
            BuiltinKind::PreEval => "`#[pre_eval_builtin]`",
        }
    }

    fn registry_ctor(self, fn_name: &syn::Ident) -> TokenStream2 {
        match self {
            BuiltinKind::Builtin => {
                quote!(crate::interpreter::builtins::BuiltinImplementation::Eager(#fn_name))
            }
            BuiltinKind::Interpreter => {
                quote!(crate::interpreter::builtins::BuiltinImplementation::Interpreter(#fn_name))
            }
            BuiltinKind::PreEval => {
                quote!(crate::interpreter::builtins::BuiltinImplementation::PreEval(#fn_name))
            }
        }
    }
}

/// Attribute macro for builtin R function definitions.
///
/// Auto-registers the function in the builtin registry via linkme.
/// The R name is inferred from the function name (`builtin_is_null` → `"is.null"`),
/// or can be overridden with `name = "..."`.
///
/// # Usage
///
/// ```ignore
/// #[builtin(min_args = 1)]
/// fn builtin_abs(args: &[RValue], _: &[(String, RValue)]) -> Result<RValue, RError> {
///     math_unary(args, f64::abs)
/// }
/// ```
#[proc_macro_attribute]
pub fn builtin(attr: TokenStream, item: TokenStream) -> TokenStream {
    let input = parse_macro_input!(item as ItemFn);
    if let Err(err) = validate_signature(&input, BuiltinKind::Builtin) {
        return err.to_compile_error().into();
    }
    let attr_args = parse_macro_input!(attr as BuiltinAttr);
    emit_builtin_registration(
        &input,
        attr_args,
        BuiltinKind::Builtin,
        "builtin_",
        "BUILTIN_REG",
    )
    .into()
}

/// Attribute macro for interpreter-level builtins that need `&Environment` access.
///
/// These builtins require calling back into the interpreter (e.g. to evaluate
/// sub-expressions, look up environments). They access the interpreter via
/// `crate::interpreter::with_interpreter()`.
///
/// The R name is inferred from the function name (stripping `interp_` prefix),
/// or can be overridden with `name = "..."`.
#[proc_macro_attribute]
pub fn interpreter_builtin(attr: TokenStream, item: TokenStream) -> TokenStream {
    let input = parse_macro_input!(item as ItemFn);
    if let Err(err) = validate_signature(&input, BuiltinKind::Interpreter) {
        return err.to_compile_error().into();
    }
    let attr_args = parse_macro_input!(attr as BuiltinAttr);
    emit_builtin_registration(
        &input,
        attr_args,
        BuiltinKind::Interpreter,
        "interp_",
        "INTERP_REG",
    )
    .into()
}

/// Attribute macro for pre-eval builtins that intercept before argument evaluation.
///
/// These builtins need access to raw AST arguments (e.g. tryCatch, quote)
/// because they must control when/whether arguments are evaluated.
///
/// The R name is inferred from the function name (stripping `pre_eval_` prefix),
/// or can be overridden with `name = "..."`.
#[proc_macro_attribute]
pub fn pre_eval_builtin(attr: TokenStream, item: TokenStream) -> TokenStream {
    let input = parse_macro_input!(item as ItemFn);
    if let Err(err) = validate_signature(&input, BuiltinKind::PreEval) {
        return err.to_compile_error().into();
    }
    let attr_args = parse_macro_input!(attr as BuiltinAttr);
    emit_builtin_registration(
        &input,
        attr_args,
        BuiltinKind::PreEval,
        "pre_eval_",
        "PRE_EVAL_REG",
    )
    .into()
}

/// Function-like macro to declare a noop stub builtin.
///
/// Generates a function that returns its first argument (or NULL) and
/// registers it in the builtin registry.
///
/// # Usage
///
/// ```ignore
/// noop_builtin!("on.exit");
/// noop_builtin!("UseMethod", 1);
/// ```
#[proc_macro]
pub fn noop_builtin(input: TokenStream) -> TokenStream {
    let args = parse_macro_input!(input as NoopArgs);
    let r_name = &args.name;
    let min_args = args.min_args as usize;

    let fn_ident = format_ident!("__noop_{}", r_name_to_ident(r_name));
    let reg_name = format_ident!("__BUILTIN_REG_{}", fn_ident.to_string().to_uppercase());

    let expanded = quote! {
        fn #fn_ident(
            args: &[crate::interpreter::value::RValue],
            _named: &[(String, crate::interpreter::value::RValue)],
        ) -> Result<crate::interpreter::value::RValue, crate::interpreter::value::RError> {
            Ok(args.first().cloned().unwrap_or(crate::interpreter::value::RValue::Null))
        }

        #[linkme::distributed_slice(crate::interpreter::builtins::BUILTIN_REGISTRY)]
        static #reg_name: crate::interpreter::builtins::BuiltinDescriptor =
            crate::interpreter::builtins::BuiltinDescriptor {
                name: #r_name,
                implementation: crate::interpreter::builtins::BuiltinImplementation::Eager(#fn_ident),
                min_args: #min_args,
            };
    };

    expanded.into()
}

struct BuiltinAttr {
    name: Option<String>,
    names: Vec<String>,
    min_args: u64,
}

impl syn::parse::Parse for BuiltinAttr {
    fn parse(input: syn::parse::ParseStream) -> syn::Result<Self> {
        let mut name = None;
        let mut names = Vec::new();
        let mut min_args = 0u64;

        while !input.is_empty() {
            let key: syn::Ident = input.parse()?;
            input.parse::<syn::Token![=]>()?;

            match key.to_string().as_str() {
                "name" => {
                    let lit: LitStr = input.parse()?;
                    name = Some(lit.value());
                }
                "names" => {
                    let content;
                    syn::bracketed!(content in input);
                    while !content.is_empty() {
                        let lit: LitStr = content.parse()?;
                        names.push(lit.value());
                        if content.peek(syn::Token![,]) {
                            content.parse::<syn::Token![,]>()?;
                        }
                    }
                }
                "min_args" => {
                    let lit: LitInt = input.parse()?;
                    min_args = lit.base10_parse()?;
                }
                other => {
                    return Err(syn::Error::new(
                        key.span(),
                        format!("unknown attribute: {}", other),
                    ));
                }
            }

            if input.peek(syn::Token![,]) {
                input.parse::<syn::Token![,]>()?;
            }
        }

        Ok(BuiltinAttr {
            name,
            names,
            min_args,
        })
    }
}

struct NoopArgs {
    name: String,
    min_args: u64,
}

impl syn::parse::Parse for NoopArgs {
    fn parse(input: syn::parse::ParseStream) -> syn::Result<Self> {
        let name: LitStr = input.parse()?;
        let min_args = if input.peek(syn::Token![,]) {
            input.parse::<syn::Token![,]>()?;
            let lit: LitInt = input.parse()?;
            lit.base10_parse()?
        } else {
            0
        };
        Ok(NoopArgs {
            name: name.value(),
            min_args,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::{validate_signature, BuiltinKind};
    use syn::parse_quote;

    #[test]
    fn builtin_signature_accepts_expected_shape() {
        let function = parse_quote! {
            fn builtin_abs(args: &[RValue], named: &[(String, RValue)]) -> Result<RValue, RError> {
                unimplemented!()
            }
        };

        assert!(validate_signature(&function, BuiltinKind::Builtin).is_ok());
    }

    #[test]
    fn interpreter_signature_rejects_missing_environment() {
        let function = parse_quote! {
            fn interp_eval(args: &[RValue], named: &[(String, RValue)]) -> Result<RValue, RError> {
                unimplemented!()
            }
        };

        assert!(validate_signature(&function, BuiltinKind::Interpreter).is_err());
    }

    #[test]
    fn pre_eval_signature_accepts_fully_qualified_types() {
        let function = parse_quote! {
            fn pre_eval_quote(
                args: &[crate::parser::ast::Arg],
                env: &crate::interpreter::environment::Environment,
            ) -> Result<crate::interpreter::value::RValue, crate::interpreter::value::RError> {
                unimplemented!()
            }
        };

        assert!(validate_signature(&function, BuiltinKind::PreEval).is_ok());
    }

    #[test]
    fn builtin_signature_rejects_wrong_return_type() {
        let function = parse_quote! {
            fn builtin_abs(args: &[RValue], named: &[(String, RValue)]) -> RValue {
                unimplemented!()
            }
        };

        assert!(validate_signature(&function, BuiltinKind::Builtin).is_err());
    }
}
