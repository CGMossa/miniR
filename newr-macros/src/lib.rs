use proc_macro::TokenStream;
use quote::{format_ident, quote};
use syn::{parse_macro_input, ItemFn, LitInt, LitStr};

/// Infer R function name from a Rust ident like `builtin_is_null` → `"is.null"`.
///
/// Strips the `builtin_` prefix (if present) and replaces `_` with `.`.
fn infer_r_name(ident: &str) -> String {
    let base = ident.strip_prefix("builtin_").unwrap_or(ident);
    base.replace('_', ".")
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
///
/// // Override the inferred name:
/// #[builtin(name = "seq_len", min_args = 1)]
/// fn builtin_seq_len(args: &[RValue], _: &[(String, RValue)]) -> Result<RValue, RError> {
///     ...
/// }
/// ```
#[proc_macro_attribute]
pub fn builtin(attr: TokenStream, item: TokenStream) -> TokenStream {
    let input = parse_macro_input!(item as ItemFn);
    let attr_args = parse_macro_input!(attr as BuiltinAttr);

    let fn_name = &input.sig.ident;
    let r_name = attr_args
        .name
        .unwrap_or_else(|| infer_r_name(&fn_name.to_string()));
    let min_args = attr_args.min_args as usize;

    // Generate a unique static name for the linkme entry
    let reg_name = format_ident!("__BUILTIN_REG_{}", fn_name.to_string().to_uppercase());

    let expanded = quote! {
        #input

        #[linkme::distributed_slice(crate::interpreter::builtins::BUILTIN_REGISTRY)]
        static #reg_name: (&str, crate::interpreter::builtins::BuiltinFn, usize) =
            (#r_name, #fn_name, #min_args);
    };

    expanded.into()
}

struct BuiltinAttr {
    name: Option<String>,
    min_args: u64,
}

impl syn::parse::Parse for BuiltinAttr {
    fn parse(input: syn::parse::ParseStream) -> syn::Result<Self> {
        let mut name = None;
        let mut min_args = 0u64;

        while !input.is_empty() {
            let key: syn::Ident = input.parse()?;
            input.parse::<syn::Token![=]>()?;

            match key.to_string().as_str() {
                "name" => {
                    let lit: LitStr = input.parse()?;
                    name = Some(lit.value());
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

        Ok(BuiltinAttr { name, min_args })
    }
}
