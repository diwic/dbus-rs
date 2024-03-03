use darling::{ast, util::SpannedValue, FromDeriveInput, FromField};
use proc_macro2::{Span, TokenStream};
use quote::{quote, quote_spanned};
use syn::{spanned::Spanned, GenericParam, Lifetime, LifetimeParam};

use crate::util::{fields_to_constructor, fields_to_var_idents};

#[derive(Debug, FromField)]
#[darling(attributes(dbus_struct))]
struct DbusStructField {
    ident: Option<syn::Ident>,
    ty: syn::Type,
}

#[derive(Debug, FromDeriveInput)]
#[darling(
    attributes(dbus_struct),
    supports(struct_named, struct_tuple, struct_newtype)
)]
pub struct DbusStruct {
    ident: syn::Ident,
    generics: syn::Generics,
    data: ast::Data<darling::util::Ignored, SpannedValue<DbusStructField>>,
}

pub fn derive_struct(input: DbusStruct) -> TokenStream {
    let DbusStruct {
        ref ident,
        ref generics,
        data,
    } = input;
    let data = data.take_struct().unwrap(/* using #[darling(supports(struct_named, struct_tuple, struct_newtype))], should fail on previous step if enum */);

    let (impl_generics, ty_generics, where_clause) = generics.split_for_impl();
    let input_name = quote!(#ident #ty_generics);

    // Create modified generics parameter with additional lifetime for implementing Get trait
    let mut generics_with_lt = generics.clone();
    let lt = Lifetime::new("'derive_dbus_args", Span::call_site());
    let ltp = LifetimeParam::new(lt.clone());
    generics_with_lt.params.push(GenericParam::Lifetime(ltp));
    let (impl_with_lt, _, _) = generics_with_lt.split_for_impl();

    // Create a format string for format!() macro in Arg trait implementation
    let sig_format = format!("({})", "{}".to_string().repeat(data.len()));

    let field_idents: Vec<_> = data.iter().map(|f| f.ident.clone()).collect();
    let field_types: Vec<_> = data.iter().map(|f| f.ty.clone()).collect();
    let var_idents = fields_to_var_idents(&ident.span(), &data.style, &field_idents);
    let field_iterator = data.iter().zip(var_idents.iter());

    let struct_constructor = fields_to_constructor(&ident.span(), &data.style, &var_idents);

    // Generating this here instead of inserting directly into quote! to attach correct span
    let get_vars = field_iterator
        .clone()
        .map(|(f, var_id)| quote_spanned!(f.ty.span() => let #var_id = si.read().ok()?;));

    quote! {
        #[automatically_derived]
        impl #impl_generics ::dbus::arg::Arg for #input_name #where_clause {
            const ARG_TYPE: ::dbus::arg::ArgType = ::dbus::arg::ArgType::Struct;

            fn signature() -> ::dbus::Signature<'static> {
                ::dbus::Signature::from(format!(
                    #sig_format,
                    #(<#field_types as ::dbus::arg::Arg>::signature()),*
                ))
            }
        }

        #[automatically_derived]
        impl #impl_generics ::dbus::arg::Append for #input_name #where_clause {
            fn append_by_ref(&self, ia: &mut ::dbus::arg::IterAppend) {
                let #struct_constructor = self;
                ia.append_struct(|s| { #( <#field_types as ::dbus::arg::Append>::append_by_ref(#var_idents, s); )* });
            }
        }

        #[automatically_derived]
        impl #impl_with_lt ::dbus::arg::Get<#lt> for #input_name #where_clause {
            fn get(i: &mut ::dbus::arg::Iter<#lt>) -> ::core::option::Option<Self> {
                let mut si = i.recurse(::dbus::arg::ArgType::Struct)?;
                #(#get_vars)*
                ::core::option::Option::Some(#struct_constructor)
            }
        }
    }
}
