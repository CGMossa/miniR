use proc_macro::TokenStream;
use proc_macro2::TokenStream as TokenStream2;
use quote::{format_ident, quote};
use syn::spanned::Spanned;
use syn::{parse_macro_input, FnArg, ItemFn, Lit, LitInt, LitStr, ReturnType, Type};

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

fn descriptor_literal(
    name: &str,
    aliases: &[String],
    implementation: TokenStream2,
    min_args: usize,
    max_args: Option<usize>,
    doc: &str,
) -> TokenStream2 {
    let max_args = match max_args {
        Some(max_args) => quote!(Some(#max_args)),
        None => quote!(None),
    };

    quote! {
        crate::interpreter::builtins::BuiltinDescriptor {
            name: #name,
            aliases: &[#(#aliases),*],
            implementation: #implementation,
            min_args: #min_args,
            max_args: #max_args,
            doc: #doc,
        }
    }
}

/// Extract doc comments (`///` or `#[doc = "..."]`) from a function's attributes.
fn extract_doc_string(input: &ItemFn) -> String {
    input
        .attrs
        .iter()
        .filter_map(|attr| {
            if attr.path().is_ident("doc") {
                if let syn::Meta::NameValue(meta) = &attr.meta {
                    if let syn::Expr::Lit(lit) = &meta.value {
                        if let syn::Lit::Str(s) = &lit.lit {
                            return Some(s.value());
                        }
                    }
                }
            }
            None
        })
        .collect::<Vec<_>>()
        .join("\n")
        .trim()
        .to_string()
}

fn emit_descriptor_registration(reg_name: &syn::Ident, descriptor: TokenStream2) -> TokenStream2 {
    quote! {
        #[linkme::distributed_slice(crate::interpreter::builtins::BUILTIN_REGISTRY)]
        static #reg_name: crate::interpreter::builtins::BuiltinDescriptor = #descriptor;
    }
}

/// Shared code generation for all three builtin macro variants.
///
/// Emits: `#[doc(alias)] fn ...` + one descriptor entry carrying aliases.
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
    let max_args = attr_args.max_args.map(|value| value as usize);
    let implementation = kind.registry_ctor(fn_name);

    let doc = extract_doc_string(input);
    let reg_name = format_ident!("__{}_{}", reg_prefix, fn_name.to_string().to_uppercase());
    let descriptor = descriptor_literal(
        &r_name,
        &attr_args.names,
        implementation,
        min_args,
        max_args,
        &doc,
    );
    let registration = emit_descriptor_registration(&reg_name, descriptor);
    let alias_docs = attr_args
        .names
        .iter()
        .map(|alias| quote!(#[doc(alias = #alias)]));

    quote! {
        #(#alias_docs)*
        #[doc(alias = #r_name)]
        #input

        #registration
    }
}

fn validate_signature(input: &ItemFn, kind: BuiltinKind) -> syn::Result<()> {
    let expected_len = match kind {
        BuiltinKind::Builtin => 2,
        BuiltinKind::Interpreter | BuiltinKind::PreEval => 3,
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
        (BuiltinKind::PreEval, 1) => is_ref_to_named(ty, "Environment"),
        (BuiltinKind::Interpreter, 2) | (BuiltinKind::PreEval, 2) => {
            is_ref_to_named(ty, "BuiltinContext")
        }
        (BuiltinKind::PreEval, 0) => is_ref_to_slice_of_named(ty, "Arg"),
        _ => false,
    }
}

fn expected_parameter_description(kind: BuiltinKind, index: usize) -> &'static str {
    match (kind, index) {
        (BuiltinKind::Builtin, 0) | (BuiltinKind::Interpreter, 0) => "`&[RValue]`",
        (BuiltinKind::Builtin, 1) | (BuiltinKind::Interpreter, 1) => "`&[(String, RValue)]`",
        (BuiltinKind::Interpreter, 2) | (BuiltinKind::PreEval, 2) => "`&BuiltinContext`",
        (BuiltinKind::PreEval, 1) => "`&Environment`",
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
/// **Deprecated:** Prefer `#[derive(FromArgs)]` + `impl Builtin` for new builtins.
/// This macro remains for existing builtins during migration.
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

/// Attribute macro for interpreter-level builtins that need `BuiltinContext`.
///
/// These builtins require calling back into the interpreter (e.g. to evaluate
/// sub-expressions or inspect environments). The explicit context keeps
/// dispatch tied to the active interpreter instance instead of hidden TLS
/// lookup.
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
    let args = parse_macro_input!(input as StubArgs);
    let r_name = &args.name;
    let min_args = args.min_args as usize;

    let fn_ident = format_ident!("__noop_{}", r_name_to_ident(r_name));
    let reg_name = format_ident!("__BUILTIN_REG_{}", fn_ident.to_string().to_uppercase());
    let descriptor = descriptor_literal(
        r_name,
        &[],
        quote!(crate::interpreter::builtins::BuiltinImplementation::Eager(#fn_ident)),
        min_args,
        None,
        "",
    );
    let registration = emit_descriptor_registration(&reg_name, descriptor);

    let expanded = quote! {
        fn #fn_ident(
            args: &[crate::interpreter::value::RValue],
            _named: &[(String, crate::interpreter::value::RValue)],
        ) -> Result<crate::interpreter::value::RValue, crate::interpreter::value::RError> {
            Ok(args.first().cloned().unwrap_or(crate::interpreter::value::RValue::Null))
        }

        #registration
    };

    expanded.into()
}

/// Function-like macro to declare an explicit unimplemented stub builtin.
///
/// Generates a function that returns a clear runtime error and registers it in
/// the builtin registry.
///
/// # Usage
///
/// ```ignore
/// stub_builtin!("url", 1);
/// stub_builtin!("open", 1, "connections are not implemented yet");
/// ```
#[proc_macro]
pub fn stub_builtin(input: TokenStream) -> TokenStream {
    let args = parse_macro_input!(input as StubArgs);
    let r_name = &args.name;
    let min_args = args.min_args as usize;
    let message = args
        .message
        .unwrap_or_else(|| format!("{r_name}() is not implemented yet"));

    let fn_ident = format_ident!("__stub_{}", r_name_to_ident(r_name));
    let reg_name = format_ident!("__BUILTIN_REG_{}", fn_ident.to_string().to_uppercase());
    let descriptor = descriptor_literal(
        r_name,
        &[],
        quote!(crate::interpreter::builtins::BuiltinImplementation::Eager(#fn_ident)),
        min_args,
        None,
        "",
    );
    let registration = emit_descriptor_registration(&reg_name, descriptor);

    let expanded = quote! {
        fn #fn_ident(
            _args: &[crate::interpreter::value::RValue],
            _named: &[(String, crate::interpreter::value::RValue)],
        ) -> Result<crate::interpreter::value::RValue, crate::interpreter::value::RError> {
            Err(crate::interpreter::value::RError::other(#message))
        }

        #registration
    };

    expanded.into()
}

#[derive(Debug)]
struct BuiltinAttr {
    name: Option<String>,
    names: Vec<String>,
    min_args: u64,
    max_args: Option<u64>,
}

impl syn::parse::Parse for BuiltinAttr {
    fn parse(input: syn::parse::ParseStream) -> syn::Result<Self> {
        let mut name = None;
        let mut names = Vec::new();
        let mut min_args = 0u64;
        let mut max_args = None;

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
                "max_args" => {
                    let lit: LitInt = input.parse()?;
                    max_args = Some(lit.base10_parse()?);
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

        if let Some(max_args) = max_args {
            if max_args < min_args {
                return Err(syn::Error::new(
                    input.span(),
                    "`max_args` cannot be smaller than `min_args`",
                ));
            }
        }

        Ok(BuiltinAttr {
            name,
            names,
            min_args,
            max_args,
        })
    }
}

struct StubArgs {
    name: String,
    min_args: u64,
    message: Option<String>,
}

impl syn::parse::Parse for StubArgs {
    fn parse(input: syn::parse::ParseStream) -> syn::Result<Self> {
        let name: LitStr = input.parse()?;
        let mut min_args = 0u64;
        let mut message = None;

        if input.peek(syn::Token![,]) {
            input.parse::<syn::Token![,]>()?;
            match input.parse::<Lit>()? {
                Lit::Int(lit) => min_args = lit.base10_parse()?,
                Lit::Str(lit) => message = Some(lit.value()),
                other => {
                    return Err(syn::Error::new(
                        other.span(),
                        "expected an integer min_args or string message",
                    ));
                }
            }
        }

        if input.peek(syn::Token![,]) {
            if message.is_some() {
                return Err(syn::Error::new(
                    input.span(),
                    "stub macros accept at most name, min_args, and message",
                ));
            }
            input.parse::<syn::Token![,]>()?;
            let lit: LitStr = input.parse()?;
            message = Some(lit.value());
        }

        Ok(StubArgs {
            name: name.value(),
            min_args,
            message,
        })
    }
}

// region: FromArgs derive macro

/// Derive macro for decoding R call arguments into a typed struct.
///
/// Each field becomes an R parameter. Field names map to named R arguments.
/// Fields are matched by name first (with partial matching), then positionally.
///
/// Supported field types: `f64`, `i64`, `bool`, `String`, `Option<T>` (optional params).
/// Use `#[default(value)]` for parameters with default values.
///
/// # Example
///
/// ```ignore
/// /// Random normal deviates.
/// #[derive(FromArgs)]
/// #[builtin(name = "rnorm")]
/// struct RnormArgs {
///     /// number of observations
///     n: i64,
///     /// mean of the distribution
///     #[default(0.0)]
///     mean: f64,
///     /// standard deviation
///     #[default(1.0)]
///     sd: f64,
/// }
/// ```
#[proc_macro_derive(FromArgs, attributes(default, builtin))]
pub fn derive_from_args(input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input as syn::DeriveInput);
    match emit_from_args(&input) {
        Ok(tokens) => tokens.into(),
        Err(err) => err.to_compile_error().into(),
    }
}

fn emit_from_args(input: &syn::DeriveInput) -> syn::Result<TokenStream2> {
    let name = &input.ident;

    let syn::Data::Struct(data) = &input.data else {
        return Err(syn::Error::new(
            input.ident.span(),
            "FromArgs can only be derived for structs",
        ));
    };
    let syn::Fields::Named(fields) = &data.fields else {
        return Err(syn::Error::new(
            input.ident.span(),
            "FromArgs requires named fields",
        ));
    };

    // Extract #[builtin(name = "...")] from struct attrs
    let r_name =
        extract_builtin_name(&input.attrs).unwrap_or_else(|| name.to_string().to_lowercase());

    // Extract aliases
    let aliases = extract_builtin_aliases(&input.attrs);

    // Extract struct-level doc comment
    let struct_doc = extract_doc_from_attrs(&input.attrs);

    // Process fields
    let mut field_decoders = Vec::new();
    let mut field_names_str = Vec::new();
    let mut field_docs = Vec::new();
    let mut min_args = 0usize;

    for field in &fields.named {
        let field_name = field.ident.as_ref().unwrap();
        let field_name_str = field_name.to_string();
        let field_ty = &field.ty;
        let field_doc = extract_doc_from_attrs(&field.attrs);
        let default_val = extract_default_attr(field)?;

        field_names_str.push(field_name_str.clone());
        if !field_doc.is_empty() {
            field_docs.push(format!("@param {} {}", field_name_str, field_doc));
        }

        let decoder = if let Some(default_expr) = default_val {
            // Optional param with default
            quote! {
                let #field_name: #field_ty = {
                    let raw = crate::interpreter::value::find_arg(
                        args, named, #field_name_str, #min_args
                    );
                    match raw {
                        Some(v) => crate::interpreter::value::coerce_arg::<#field_ty>(v, #field_name_str)?,
                        None => #default_expr,
                    }
                };
            }
        } else {
            // Required param
            let decoder = quote! {
                let #field_name: #field_ty = {
                    let raw = crate::interpreter::value::find_arg(
                        args, named, #field_name_str, #min_args
                    ).ok_or_else(|| crate::interpreter::value::RError::new(
                        crate::interpreter::value::RErrorKind::Argument,
                        format!("argument '{}' is missing, with no default", #field_name_str),
                    ))?;
                    crate::interpreter::value::coerce_arg::<#field_ty>(raw, #field_name_str)?
                };
            };
            min_args += 1;
            decoder
        };
        field_decoders.push(decoder);
    }

    let field_idents: Vec<_> = fields.named.iter().map(|f| &f.ident).collect();
    let max_args = fields.named.len();

    // Build doc string: struct doc + field @param docs
    let mut full_doc_parts = vec![struct_doc.clone()];
    full_doc_parts.extend(field_docs);
    let full_doc = full_doc_parts.join("\n");

    let alias_tokens: Vec<_> = aliases.iter().map(|a| quote!(#a)).collect();

    let reg_name = format_ident!("__BUILTIN_REG_TRAIT_{}", name.to_string().to_uppercase());

    Ok(quote! {
        impl crate::interpreter::value::FromArgs for #name {
            fn from_args(
                args: &[crate::interpreter::value::RValue],
                named: &[(String, crate::interpreter::value::RValue)],
            ) -> Result<Self, crate::interpreter::value::RError> {
                #(#field_decoders)*
                Ok(#name { #(#field_idents),* })
            }

            fn info() -> &'static crate::interpreter::value::BuiltinInfo {
                static INFO: crate::interpreter::value::BuiltinInfo = crate::interpreter::value::BuiltinInfo {
                    name: #r_name,
                    aliases: &[#(#alias_tokens),*],
                    min_args: #min_args,
                    max_args: Some(#max_args),
                    doc: #full_doc,
                    params: &[#(#field_names_str),*],
                };
                &INFO
            }
        }

        // Auto-register into the builtin registry via linkme.
        // The wrapper decodes args via FromArgs, then calls Builtin::call.
        #[linkme::distributed_slice(crate::interpreter::builtins::BUILTIN_REGISTRY)]
        static #reg_name: crate::interpreter::builtins::BuiltinDescriptor =
            crate::interpreter::builtins::BuiltinDescriptor {
                name: #r_name,
                aliases: &[#(#alias_tokens),*],
                implementation: crate::interpreter::builtins::BuiltinImplementation::Interpreter(
                    |args, named, ctx| {
                        let decoded = <#name as crate::interpreter::value::FromArgs>::from_args(args, named)?;
                        <#name as crate::interpreter::value::Builtin>::call(decoded, ctx)
                    }
                ),
                min_args: #min_args,
                max_args: Some(#max_args),
                doc: #full_doc,
            };
    })
}

fn extract_builtin_name(attrs: &[syn::Attribute]) -> Option<String> {
    for attr in attrs {
        if attr.path().is_ident("builtin") {
            let mut name = None;
            let _ = attr.parse_nested_meta(|meta| {
                if meta.path.is_ident("name") {
                    let value = meta.value()?;
                    let s: LitStr = value.parse()?;
                    name = Some(s.value());
                }
                Ok(())
            });
            return name;
        }
    }
    None
}

fn extract_builtin_aliases(attrs: &[syn::Attribute]) -> Vec<String> {
    let mut aliases = Vec::new();
    for attr in attrs {
        if attr.path().is_ident("builtin") {
            let _ = attr.parse_nested_meta(|meta| {
                if meta.path.is_ident("names") {
                    let value = meta.value()?;
                    // Parse as a bracketed list of string literals: ["a", "b"]
                    let content;
                    syn::bracketed!(content in value);
                    while !content.is_empty() {
                        let s: LitStr = content.parse()?;
                        aliases.push(s.value());
                        if content.is_empty() {
                            break;
                        }
                        let _: syn::Token![,] = content.parse()?;
                    }
                }
                Ok(())
            });
        }
    }
    aliases
}

fn extract_doc_from_attrs(attrs: &[syn::Attribute]) -> String {
    attrs
        .iter()
        .filter_map(|attr| {
            if attr.path().is_ident("doc") {
                if let syn::Meta::NameValue(meta) = &attr.meta {
                    if let syn::Expr::Lit(lit) = &meta.value {
                        if let syn::Lit::Str(s) = &lit.lit {
                            return Some(s.value());
                        }
                    }
                }
            }
            None
        })
        .collect::<Vec<_>>()
        .join("\n")
        .trim()
        .to_string()
}

fn extract_default_attr(field: &syn::Field) -> syn::Result<Option<TokenStream2>> {
    for attr in &field.attrs {
        if attr.path().is_ident("default") {
            let tokens: TokenStream2 = attr.parse_args()?;
            return Ok(Some(tokens));
        }
    }
    Ok(None)
}

// endregion

#[cfg(test)]
mod tests {
    use super::{validate_signature, BuiltinAttr, BuiltinKind, StubArgs};
    use syn::{parse_quote, parse_str};

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

    #[test]
    fn builtin_attr_accepts_max_args() {
        let attr = parse_str::<BuiltinAttr>(r#"name = "globalenv", max_args = 0"#)
            .expect("failed to parse builtin attr");

        assert_eq!(attr.max_args, Some(0));
    }

    #[test]
    fn builtin_attr_rejects_max_args_below_min_args() {
        let err = parse_str::<BuiltinAttr>(r#"min_args = 2, max_args = 1"#)
            .expect_err("attr unexpectedly parsed");

        assert!(err.to_string().contains("max_args"));
    }

    #[test]
    fn stub_args_accept_custom_message() {
        let args = parse_str::<StubArgs>(r#""url", 1, "connections are not implemented yet""#)
            .expect("failed to parse stub args");

        assert_eq!(args.name, "url");
        assert_eq!(args.min_args, 1);
        assert_eq!(
            args.message.as_deref(),
            Some("connections are not implemented yet")
        );
    }
}
