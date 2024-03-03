use darling::ast::Style;
use proc_macro2::{Span, TokenStream};
use proc_macro_error::{abort, emit_error};
use quote::{format_ident, quote, ToTokens};
use syn::{spanned::Spanned, DeriveInput, Ident, Type};

pub fn derive_input_style_span(input: DeriveInput) -> Span {
    match input.data {
        syn::Data::Enum(e) => e.enum_token.span(),
        syn::Data::Struct(s) => s.struct_token.span(),
        syn::Data::Union(u) => u.union_token.span(),
    }
}

/// Returns struct constructor that is appropriate for given struct style
pub fn fields_to_constructor(span: &Span, style: &Style, var_names: &[Ident]) -> TokenStream {
    match style {
        Style::Struct => {
            quote! {
                Self { #(#var_names),* }
            }
        }
        Style::Tuple => {
            quote! {
                Self ( #(#var_names),* )
            }
        }
        Style::Unit => {
            abort!(span, "Unit structs not supported")
        }
    }
}

/// Returns array of identifiers that could be used as variable name for each field
pub fn fields_to_var_idents(
    span: &Span,
    style: &Style,
    field_idents: &[Option<Ident>],
) -> Vec<Ident> {
    field_idents
        .iter()
        .enumerate()
        .map(|(idx, field)| match style {
            Style::Struct => field.clone().expect("Fields in structs should have names"),
            Style::Tuple => format_ident!("arg{idx}"),
            Style::Unit => abort!(span, "Unit structs not supported"),
        })
        .collect()
}

/// Extracts a generic argument idx from ty and parses it as syn::Type
/// Emits compilation error and returns None if containted type cannot be extracted
pub fn ty_generic_to_ty_contained(ty: &Type, container_name: &str, idx: usize) -> Option<Type> {
    let err_str = "dbus_derive - Will fail in case field is missing from dbus::PropMap".to_string();
    let hint_str = format!("Try this - {container_name}<{}T>", "_, ".repeat(idx));

    match ty {
        Type::Path(ty_path) => {
            let segments = &ty_path.path.segments;
            if segments.len() < idx + 1 {
                emit_error!(ty, err_str; hint=hint_str);
                return None;
            }
            let segment = segments.last().unwrap(/* Verified above */);

            let hint_str = format!(
                "Try this - {container_name}<{}{}>",
                "_, ".repeat(idx),
                segment.ident
            );
            if segment.ident != container_name {
                emit_error!(ty, err_str; hint=hint_str);
                return None;
            }
            let args = match &segment.arguments {
                syn::PathArguments::AngleBracketed(args) => args,
                _ => {
                    emit_error!(ty, err_str; hint=hint_str);
                    return None;
                }
            };
            let Some(arg_contained) = args.args.iter().nth(idx) else {
                emit_error!(ty, err_str; hint=hint_str);
                return None;
            };
            let Ok(ty_contained) = syn::parse::<Type>(arg_contained.to_token_stream().into())
            else {
                emit_error!(ty, err_str; hint=hint_str);
                return None;
            };
            Some(ty_contained)
        }
        _ => None,
    }
}
