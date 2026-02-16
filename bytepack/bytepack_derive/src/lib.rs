use proc_macro2::TokenStream;
use quote::quote;
use syn::{
    DataStruct, DeriveInput, Fields, Generics, Ident, LifetimeParam, TraitBound, parse_macro_input, parse_quote
};

#[proc_macro_derive(Pack)]
pub fn derive_pack(input: proc_macro::TokenStream) -> proc_macro::TokenStream {
    let input = parse_macro_input!(input as DeriveInput);

    let result = match input.data {
        syn::Data::Struct(data_struct) => {
            impl_pack_struct(input.ident, data_struct, input.generics)
        }
        _ => syn::Error::new(input.ident.span(), "Pack can only be derived on structs")
            .into_compile_error(),
    };

    proc_macro::TokenStream::from(result)
}

fn impl_pack_struct(ident: Ident, data: DataStruct, generics: Generics) -> TokenStream {
    let field_size_iter = data.fields.iter().map(|field| {
        let ty = &field.ty;
        quote! {
            <#ty as bytepack::Pack>::PACK_BYTES
        }
    });

    let field_name = data.fields.members();

    let growing_offset = growing_offset(&data.fields);

    let generics = insert_generics(generics, false);
    let (impl_gen, type_gen, where_clause) = generics.split_for_impl();

    quote! {
        #[automatically_derived]
        impl #impl_gen bytepack::Pack for #ident #type_gen #where_clause {
            const PACK_BYTES: u32 = #(#field_size_iter)+*;

            fn pack(&self, offset: u32, packer: &mut bytepack::BytePacker) {
                #(
                    bytepack::Pack::pack(&self.#field_name, offset + #growing_offset, packer);
                )*
            }
        }
    }
}

#[proc_macro_derive(Unpack)]
pub fn derive_unpack(input: proc_macro::TokenStream) -> proc_macro::TokenStream {
    let input = parse_macro_input!(input as DeriveInput);

    let result = match input.data {
        syn::Data::Struct(data_struct) => impl_unpack_struct(input.ident, data_struct, input.generics),
        _ => syn::Error::new(input.ident.span(), "Unpack can only be derived on structs")
            .into_compile_error(),
    };

    proc_macro::TokenStream::from(result)
}

fn impl_unpack_struct(ident: Ident, data: DataStruct, generics: Generics) -> TokenStream {
    let field_name = data.fields.members();

    let growing_offset = growing_offset(&data.fields);


    let lifetime_generics = insert_generics(generics.clone(), true);
    let (impl_gen, _, _) = lifetime_generics.split_for_impl();
    let (_, type_gen, where_clause) = generics.split_for_impl();

    quote! {
        #[automatically_derived]
        impl #impl_gen bytepack::Unpack<'b> for #ident #type_gen #where_clause {

            fn unpack(offset: u32, unpacker: &bytepack::ByteUnpacker<'b>) -> Option<Self> {
                Some(Self {
                    #(
                        #field_name: bytepack::Unpack::unpack(offset + #growing_offset, unpacker)?
                    ),*
                })
            }
        }
    }
}

fn growing_offset(fields: &Fields) -> impl Iterator<Item = TokenStream> {
    fields.iter().scan(quote! { 0 }, |acc, field| {
        let result = quote! {const { #acc }};

        let ty = &field.ty;
        *acc = quote! { #acc + <#ty as bytepack::Pack>::PACK_BYTES };

        Some(result)
    })
}

fn insert_generics(mut generics: Generics, unpack: bool) -> Generics {
    if unpack {
        generics.params.insert(0, syn::GenericParam::Lifetime(LifetimeParam {
            lifetime: parse_quote! {'b},
            attrs: Default::default(),
            colon_token: None,
            bounds: Default::default(),
        }));
    }

    for param in generics.type_params_mut() {
        param.bounds.push(syn::TypeParamBound::Trait(TraitBound {
            paren_token: None,
            modifier: syn::TraitBoundModifier::None,
            lifetimes: None,
            path: parse_quote! { bytepack::Pack },
        }));

        if unpack {
            param.bounds.push(syn::TypeParamBound::Trait(TraitBound {
                paren_token: None,
                modifier: syn::TraitBoundModifier::None,
                lifetimes: None,
                path: parse_quote! { bytepack::Unpack<'b> },
            }));
        }
    }

    generics
}
