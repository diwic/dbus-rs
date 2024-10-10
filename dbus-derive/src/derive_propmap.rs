use darling::{ast, util::SpannedValue, FromDeriveInput, FromField};
use proc_macro2::{Span, TokenStream};
use quote::{format_ident, quote};
use syn::{GenericParam, Lifetime, LifetimeParam};

use crate::util::{fields_to_constructor, fields_to_var_idents, ty_generic_to_ty_contained};

#[derive(Debug, FromField)]
#[darling(attributes(dbus_propmap))]
struct DbusPropmapField {
    ident: Option<syn::Ident>,
    ty: syn::Type,
    rename: Option<String>,
}

#[derive(Debug, FromDeriveInput)]
#[darling(attributes(dbus_propmap), supports(struct_named))]
pub struct DbusPropmap {
    ident: syn::Ident,
    generics: syn::Generics,
    data: ast::Data<darling::util::Ignored, SpannedValue<DbusPropmapField>>,
}

pub fn derive_propmap(input: DbusPropmap) -> TokenStream {
    let DbusPropmap {
        ref ident,
        ref generics,
        data,
    } = input;
    let data = data.take_struct().unwrap(/* using #[darling(supports(struct_named, struct_tuple, struct_newtype))], should fail on previous step if enum */);

    let (impl_generics, ty_generics, where_clause) = generics.split_for_impl();
    let input_name = quote!(#ident #ty_generics);

    // Create modified generics parameter with additional lifetime for implementing Get trait
    let mut generics_with_lt = generics.clone();
    let lt = Lifetime::new("'derive_dbus_enum", Span::call_site());
    let ltp = LifetimeParam::new(lt.clone());
    generics_with_lt.params.push(GenericParam::Lifetime(ltp));
    let (impl_with_lt, _, _) = generics_with_lt.split_for_impl();

    // Use the same identifier for all HashMaps, just to not make silly mistakes
    let map_ident = format_ident!("m");

    let field_idents: Vec<_> = data.iter().map(|f| f.ident.clone()).collect();
    let field_types: Vec<_> = data
        .iter()
        .filter_map(|f| ty_generic_to_ty_contained(&f.ty, "Option", 0))
        .collect();
    let var_names = fields_to_var_idents(&ident.span(), &data.style, &field_idents);
    let self_constructor = fields_to_constructor(&ident.span(), &data.style, &var_names);

    // Could not figure out correct type for every field of target struct, bail out
    if field_types.len() < data.len() {
        return TokenStream::new();
    }

    // Check if field has a rename attribute on it, if so - use provided name to access hashmap
    let var_name_strs: Vec<_> = var_names
        .iter()
        .zip(data.iter())
        .map(|(n, f)| {
            if let Some(rename) = &f.rename {
                rename.clone()
            } else {
                n.to_string()
            }
        })
        .collect();

    quote! {
        #[automatically_derived]
        impl #impl_generics ::dbus::arg::Arg for #input_name #where_clause {
            const ARG_TYPE: ::dbus::arg::ArgType = ::dbus::arg::ArgType::Array;

            fn signature() -> ::dbus::Signature<'static> {
                ::dbus::arg::PropMap::signature()
            }
        }

        #[automatically_derived]
        impl #impl_with_lt ::dbus::arg::Get<#lt> for #input_name #where_clause {
            fn get(i: &mut ::dbus::arg::Iter<#lt>) -> ::core::option::Option<Self> {
                let #map_ident: ::dbus::arg::PropMap = ::dbus::arg::Dict::get(i).map(|d| d.collect())?;
                #(let #var_names = #map_ident.get(#var_name_strs)
                    .and_then(|f| f.0.as_any().downcast_ref::<#field_types>())
                    .cloned();
                )*
                ::core::option::Option::Some(#self_constructor)
            }
        }

        #[automatically_derived]
        impl #impl_generics ::dbus::arg::Append for #input_name #where_clause {
            fn append_by_ref(&self, ia: &mut ::dbus::arg::IterAppend) {
                let mut #map_ident = ::dbus::arg::PropMap::new();
                let (#self_constructor) = self;
                #(if let ::core::option::Option::Some(f) = #var_names.as_ref().cloned() {
                    #map_ident.insert(#var_name_strs.to_string(), ::dbus::arg::Variant(::std::boxed::Box::new(f)));
                })*
                ::dbus::arg::Dict::new(#map_ident.iter()).append_by_ref(ia);
            }
        }
    }
}
