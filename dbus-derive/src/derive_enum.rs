use darling::FromDeriveInput;
use proc_macro2::{Span, TokenStream};
use quote::{format_ident, quote, quote_spanned};
use syn::{GenericParam, Lifetime, LifetimeParam, Type};

#[derive(Debug, FromDeriveInput)]
#[darling(attributes(dbus_enum), supports(enum_unit))]
pub struct DbusEnum {
    ident: syn::Ident,
    generics: syn::Generics,
    as_type: Type,
}

pub fn derive_enum(input: DbusEnum) -> TokenStream {
    let DbusEnum {
        ref ident,
        ref generics,
        as_type,
    } = input;

    let (impl_generics, ty_generics, where_clause) = generics.split_for_impl();
    let input_name = quote!(#ident #ty_generics);

    let mut generics_with_lt = generics.clone();
    let lt = Lifetime::new("'derive_dbus_enum", Span::call_site());
    let ltp = LifetimeParam::new(lt.clone());
    generics_with_lt.params.push(GenericParam::Lifetime(ltp));
    let (impl_with_lt, _, _) = generics_with_lt.split_for_impl();

    // Generate a stub struct that will throw compile-time errors if trait bounds are not fullfiled
    let assert_struct_ident = format_ident!("_AssertDbusEnum{}", ident);
    let assert_struct = quote_spanned!(ident.span() =>
        struct #assert_struct_ident where #as_type: ::core::convert::From<#ident>, #ident: ::core::convert::TryFrom<#as_type>;
    );

    quote! {
        #assert_struct

        #[automatically_derived]
        impl #impl_generics ::dbus::arg::Arg for #input_name #where_clause {
            const ARG_TYPE: ::dbus::arg::ArgType = <#as_type as ::dbus::arg::Arg>::ARG_TYPE;

            fn signature() -> ::dbus::Signature<'static> {
                <#as_type as ::dbus::arg::Arg>::signature()
            }
        }

        #[automatically_derived]
        impl #impl_with_lt ::dbus::arg::Get<#lt> for #input_name #where_clause {
            fn get(i: &mut ::dbus::arg::Iter<#lt>) -> ::core::option::Option<Self> {
                let ::core::result::Result::Ok(val) = i.read::<#as_type>() else {
                    return ::core::option::Option::None;
                };
                ::core::convert::TryFrom::<#as_type>::try_from(val).ok()
            }
        }

        #[automatically_derived]
        impl #impl_generics ::dbus::arg::Append for #input_name #where_clause {
            fn append_by_ref(&self, ia: &mut ::dbus::arg::IterAppend) {
                ia.append(::core::convert::Into::<#as_type>::into(*self));
            }
        }
    }
}
