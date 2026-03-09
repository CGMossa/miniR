use proc_macro::TokenStream;
use proc_macro2::TokenStream as TokenStream2;
use quote::{format_ident, quote};
use syn::{parse_macro_input, ItemFn, LitInt, LitStr};

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
    prefix: &str,
    reg_prefix: &str,
    registry_path: TokenStream2,
    fn_type: TokenStream2,
) -> TokenStream2 {
    let fn_name = &input.sig.ident;
    let r_name = attr_args
        .name
        .unwrap_or_else(|| infer_r_name(&fn_name.to_string(), prefix));
    let min_args = attr_args.min_args as usize;

    let reg_name = format_ident!("__{}_{}", reg_prefix, fn_name.to_string().to_uppercase());

    let alias_regs = attr_args.names.iter().enumerate().map(|(i, alias)| {
        let alias_reg = format_ident!(
            "{}_{}_ALIAS_{}",
            reg_name,
            fn_name.to_string().to_uppercase(),
            i
        );
        quote! {
            #[linkme::distributed_slice(#registry_path)]
            static #alias_reg: (&str, #fn_type, usize) = (#alias, #fn_name, #min_args);
        }
    });

    quote! {
        #[doc(alias = #r_name)]
        #input

        #[linkme::distributed_slice(#registry_path)]
        static #reg_name: (&str, #fn_type, usize) = (#r_name, #fn_name, #min_args);

        #(#alias_regs)*
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
    let attr_args = parse_macro_input!(attr as BuiltinAttr);
    emit_builtin_registration(
        &input,
        attr_args,
        "builtin_",
        "BUILTIN_REG",
        quote!(crate::interpreter::builtins::BUILTIN_REGISTRY),
        quote!(crate::interpreter::builtins::BuiltinFn),
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
    let attr_args = parse_macro_input!(attr as BuiltinAttr);
    emit_builtin_registration(
        &input,
        attr_args,
        "interp_",
        "INTERP_REG",
        quote!(crate::interpreter::builtins::INTERPRETER_BUILTIN_REGISTRY),
        quote!(crate::interpreter::builtins::InterpreterBuiltinFn),
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
    let attr_args = parse_macro_input!(attr as BuiltinAttr);
    emit_builtin_registration(
        &input,
        attr_args,
        "pre_eval_",
        "PRE_EVAL_REG",
        quote!(crate::interpreter::builtins::PRE_EVAL_BUILTIN_REGISTRY),
        quote!(crate::interpreter::builtins::PreEvalBuiltinFn),
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
        static #reg_name: (&str, crate::interpreter::builtins::BuiltinFn, usize) =
            (#r_name, #fn_ident, #min_args);
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
