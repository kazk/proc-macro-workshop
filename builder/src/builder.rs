use darling::{ast, FromDeriveInput};
use quote::{format_ident, quote};

pub fn derive(input: proc_macro2::TokenStream) -> proc_macro2::TokenStream {
    let input: syn::DeriveInput = match syn::parse2(input) {
        Err(err) => return err.to_compile_error(),
        Ok(input) => input,
    };
    let input = match BuilderInput::from_derive_input(&input) {
        Err(err) => return err.write_errors(),
        Ok(input) => input,
    };

    let builder_method = impl_builder_method(&input);
    let builder_struct = define_builder_struct(&input);
    let impl_builder = impl_builder_struct(&input);
    quote! {
        #builder_method
        #builder_struct
        #impl_builder
    }
}

#[derive(Debug, FromField)]
#[darling(attributes(builder))]
struct BuilderField {
    ident: Option<syn::Ident>,
    ty: syn::Type,
    #[darling(default)]
    each: Option<String>,
}

#[derive(Debug, FromDeriveInput)]
#[darling(attributes(builder), supports(struct_named))]
struct BuilderInput {
    ident: syn::Ident,
    data: BuilderInputData,
}

type BuilderInputData = ast::Data<darling::util::Ignored, BuilderField>;

fn impl_builder_method(input: &BuilderInput) -> proc_macro2::TokenStream {
    match &input.data {
        ast::Data::Struct(ref fields) => {
            let ident = &input.ident;
            let fields = fields.fields.iter().map(init_field);
            let builder_name = format_ident!("{}Builder", ident);
            quote! {
                impl #ident {
                    pub fn builder() -> #builder_name {
                        #builder_name {
                            #(#fields),*
                        }
                    }
                }
            }
        }
        _ => unreachable!(),
    }
}

fn define_builder_struct(input: &BuilderInput) -> proc_macro2::TokenStream {
    match &input.data {
        ast::Data::Struct(ref data) => {
            let ident = &input.ident;
            let fields = data.fields.iter().map(optionize_field);
            let builder_name = format_ident!("{}Builder", ident);
            quote! {
                pub struct #builder_name {
                    #(#fields),*
                }
            }
        }
        _ => unreachable!(),
    }
}

fn impl_builder_struct(input: &BuilderInput) -> proc_macro2::TokenStream {
    match &input.data {
        ast::Data::Struct(ref data) => {
            let ident = &input.ident;
            let setters = data.fields.iter().map(gen_setter).collect::<Vec<_>>();
            let assigns = data.fields.iter().map(assign_field);
            let builder_name = format_ident!("{}Builder", ident);
            quote! {
                impl #builder_name {
                    pub fn build(&mut self) -> std::result::Result<#ident, std::boxed::Box<dyn std::error::Error>> {
                        std::result::Result::Ok(#ident {
                            #(#assigns),*
                        })
                    }
                    #(#setters)*
                }
            }
        }
        _ => unreachable!(),
    }
}

fn init_field(field: &BuilderField) -> proc_macro2::TokenStream {
    let field_name = &field.ident;
    let field_type = &field.ty;
    if inner_type_of(field_type, "Vec").is_some() {
        quote! { #field_name: std::option::Option::Some(vec!()) }
    } else {
        quote! { #field_name: std::option::Option::None }
    }
}

fn optionize_field(field: &BuilderField) -> proc_macro2::TokenStream {
    let field_name = &field.ident;
    let field_type = &field.ty;
    if inner_type_of(field_type, "Option").is_some() {
        quote! { #field_name: #field_type }
    } else {
        quote! { #field_name: std::option::Option<#field_type> }
    }
}

fn assign_field(field: &BuilderField) -> proc_macro2::TokenStream {
    let field_name = &field.ident;
    let field_type = &field.ty;

    if inner_type_of(field_type, "Option").is_some() {
        quote! { #field_name: self.#field_name.clone() }
    } else {
        let err = format!("{} was not set", field_name.as_ref().unwrap().to_string());
        quote! { #field_name: self.#field_name.clone().ok_or(#err)? }
    }
}

fn gen_setter(field: &BuilderField) -> proc_macro2::TokenStream {
    let field_name = field.ident.as_ref().unwrap();
    let field_type = if let Some(inner_ty) = inner_type_of(&field.ty, "Option") {
        inner_ty
    } else {
        &field.ty
    };

    let each_setter = if let Some(each_value) = &field.each {
        let each_ident = format_ident!("{}", each_value);
        let inner_type = inner_type_of(field_type, "Vec").unwrap();
        let each = quote! {
            pub fn #each_ident(&mut self, #each_ident: #inner_type) -> &mut Self {
                if let std::option::Option::Some(ref mut vs) = self.#field_name {
                    vs.push(#each_ident);
                } else {
                    self.#field_name = std::option::Option::Some(vec![#each_ident]);
                }
                self
            }
        };
        if &each_ident == field_name {
            return each;
        }
        Some(each)
    } else {
        None
    };

    quote! {
        #each_setter
        pub fn #field_name(&mut self, #field_name: #field_type) -> &mut Self {
            self.#field_name = std::option::Option::Some(#field_name);
            self
        }
    }
}

fn inner_type_of<'a>(ty: &'a syn::Type, container: &str) -> Option<&'a syn::Type> {
    match ty {
        syn::Type::Path(ref type_path) => {
            let segments = &type_path.path.segments;
            match segments.last() {
                Some(syn::PathSegment {
                    ident,
                    arguments:
                        syn::PathArguments::AngleBracketed(syn::AngleBracketedGenericArguments {
                            args,
                            ..
                        }),
                }) if ident == container => {
                    if let Some(syn::GenericArgument::Type(ty)) = args.last() {
                        return Some(ty);
                    }
                    None
                }
                _ => None,
            }
        }

        _ => None,
    }
}
