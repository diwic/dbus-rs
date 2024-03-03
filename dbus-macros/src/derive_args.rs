use darling::{ast, util::SpannedValue, FromDeriveInput, FromField};
use proc_macro2::TokenStream;
use quote::{quote, quote_spanned};
use syn::spanned::Spanned;

use crate::util::{fields_to_constructor, fields_to_var_idents};

#[derive(Debug, FromField)]
#[darling(attributes(dbus_args))]
struct DbusArgsField {
    ident: Option<syn::Ident>,
    ty: syn::Type,
}

#[derive(Debug, FromDeriveInput)]
#[darling(
    attributes(dbus_args),
    supports(struct_named, struct_tuple, struct_newtype)
)]
pub struct DbusArgs {
    ident: syn::Ident,
    generics: syn::Generics,
    data: ast::Data<darling::util::Ignored, SpannedValue<DbusArgsField>>,
}

pub fn derive_args(input: DbusArgs) -> TokenStream {
    let DbusArgs {
        ref ident,
        ref generics,
        data,
    } = input;
    let data = data.take_struct().unwrap(/* using #[darling(supports(struct_named, struct_tuple, struct_newtype))], should fail on previous step if enum */);

    let (impl_generics, ty_generics, where_clause) = generics.split_for_impl();
    let input_name = quote!(#ident #ty_generics);

    let strs = core::iter::repeat(quote!(&'static str)).take(data.len());

    let field_idents: Vec<_> = data.iter().map(|f| f.ident.clone()).collect();
    let field_types: Vec<_> = data.iter().map(|f| f.ty.clone()).collect();
    let var_idents = fields_to_var_idents(&ident.span(), &data.style, &field_idents);
    let struct_constructor = fields_to_constructor(&ident.span(), &data.style, &var_idents);

    // Generating TokenStreams with calls to methods to attach correct field spans
    let (mut iter_append_vars, mut iter_read_vars) = (vec![], vec![]);
    for (f_id, f_ty) in var_idents.iter().zip(field_types.iter()) {
        iter_append_vars.push(quote_spanned!(f_ty.span() => ia.append(#f_id);));
        iter_read_vars.push(quote_spanned!(f_ty.span() => let #f_id = i.read()?;))
    }

    quote! {
        #[automatically_derived]
        impl #impl_generics ::dbus::arg::ArgAll for #input_name #where_clause {
            type strs = ( #(#strs),* );

            fn strs_sig<F: ::std::ops::FnMut(&'static str, ::dbus::Signature<'static>)>(strs: Self::strs, mut f: F) {
                let (#(#var_idents),*) = strs;
                #(f(#var_idents, <#field_types as ::dbus::arg::Arg>::signature());)*
            }
        }

        #[automatically_derived]
        impl #impl_generics ::dbus::arg::AppendAll for #input_name #where_clause {
            fn append(&self, ia: &mut ::dbus::arg::IterAppend) {
                let #struct_constructor = self;
                #(#iter_append_vars)*
            }
        }

        #[automatically_derived]
        impl #impl_generics ::dbus::arg::ReadAll for #input_name #where_clause {
            fn read(i: &mut ::dbus::arg::Iter) -> ::core::result::Result<Self, ::dbus::arg::TypeMismatchError> {
                #(#iter_read_vars)*
                ::core::result::Result::Ok(#struct_constructor)
            }
        }
    }
}
