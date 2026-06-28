use proc_macro2::TokenStream;
use quote::quote;
use syn::{Data, DeriveInput, Fields, Lit, Type};

pub(crate) struct ProjectionDerive<'a> {
    input: &'a DeriveInput,
}

impl<'a> ProjectionDerive<'a> {
    pub(crate) fn new(input: &'a DeriveInput) -> Self {
        Self { input }
    }

    pub(crate) fn expand(&self) -> syn::Result<TokenStream> {
        match &self.input.data {
            Data::Struct(data) => match &data.fields {
                Fields::Named(fields) => self.expand_named(fields),
                Fields::Unnamed(fields) if fields.unnamed.len() == 1 => self.expand_newtype(),
                _ => Err(syn::Error::new_spanned(
                    &self.input.ident,
                    "Projection can only be derived for named structs or single-field newtypes",
                )),
            },
            _ => Err(syn::Error::new_spanned(
                &self.input.ident,
                "Projection can only be derived for structs",
            )),
        }
    }

    fn expand_named(&self, fields: &syn::FieldsNamed) -> syn::Result<TokenStream> {
        let name = &self.input.ident;
        let mut entries = vec![];

        for field in &fields.named {
            let field_name = field.ident.as_ref().unwrap().to_string();
            let has_project = field.attrs.iter().any(|a| a.path().is_ident("project"));

            if has_project {
                let inner = Self::peel_container(&field.ty);
                entries.push(quote! {
                    for f in <#inner as doclayer_core::query::Projection>::fields() {
                        __fields.push(format!("{}.{}", #field_name, f));
                    }
                });
            } else {
                entries.push(quote! {
                    __fields.push(#field_name.to_string());
                });
            }
        }

        Ok(quote! {
            impl doclayer_core::query::Projection for #name {
                fn fields() -> Vec<String> {
                    let mut __fields = vec![];
                    #(#entries)*
                    __fields
                }
            }
        })
    }

    fn expand_newtype(&self) -> syn::Result<TokenStream> {
        let name = &self.input.ident;
        let fields = self.parse_projection_fields()?;

        Ok(quote! {
            impl doclayer_core::query::Projection for #name {
                fn fields() -> Vec<String> {
                    vec![#(#fields.to_string()),*]
                }
            }
        })
    }

    fn parse_projection_fields(&self) -> syn::Result<Vec<String>> {
        for attr in &self.input.attrs {
            if !attr.path().is_ident("projection") {
                continue;
            }

            let mut result = vec![];

            attr.parse_nested_meta(|meta| {
                if !meta.path.is_ident("fields") {
                    return Err(meta.error("expected `fields = [...]`"));
                }

                let value = meta.value()?;
                let array: syn::ExprArray = value.parse()?;

                for elem in &array.elems {
                    match elem {
                        syn::Expr::Lit(syn::ExprLit { lit: Lit::Str(s), .. }) => {
                            result.push(s.value());
                        }
                        _ => {
                            return Err(syn::Error::new_spanned(
                                elem,
                                "expected string literal in fields list",
                            ));
                        }
                    }
                }

                Ok(())
            })?;

            return Ok(result);
        }

        Err(syn::Error::new_spanned(
            &self.input.ident,
            "#[derive(Projection)] on a newtype requires a #[projection(fields = [...])] attribute",
        ))
    }

    /// Peels one layer of `Vec<T>` or `Option<T>` to reach the inner type.
    fn peel_container(ty: &Type) -> &Type {
        if let Type::Path(type_path) = ty {
            if let Some(segment) = type_path.path.segments.last() {
                let ident = segment.ident.to_string();
                if ident == "Vec" || ident == "Option" {
                    if let syn::PathArguments::AngleBracketed(args) = &segment.arguments {
                        if let Some(syn::GenericArgument::Type(inner)) = args.args.first() {
                            return inner;
                        }
                    }
                }
            }
        }

        ty
    }
}
