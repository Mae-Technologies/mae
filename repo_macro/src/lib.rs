#![feature(inherent_associated_types)]
#![allow(incomplete_features)]
extern crate proc_macro;
use proc_macro::Ident;
use proc_macro::TokenStream;
use quote::{format_ident, quote};
use syn::Data::Struct;
use syn::Fields::Named;
use syn::FieldsNamed;
use syn::parse_macro_input;
use syn::{Data, DataStruct, DeriveInput, Field, Fields, LitStr};

#[proc_macro_attribute]
pub fn schema(args: TokenStream, input: TokenStream) -> TokenStream {
    let repo_name = parse_macro_input!(args as LitStr).value();
    let ast = parse_macro_input!(input as DeriveInput);

    let repo_ident = &ast.ident;

    // confirm the macro is being called on a Struct Type and extract the fields.
    let fields = match ast.data {
        Struct(DataStruct {
            fields: Named(FieldsNamed { ref named, .. }),
            ..
        }) => named,
        _ => unimplemented!("Only works for structs"),
    };

    // rebuild the struct fields
    let params = fields.iter().map(|f| {
        let name = &f.ident;
        let ty = &f.ty;
        quote! {pub #name: #ty}
    });

    // rebuild repo struct with the existing fields and default fields for the repo
    // NOTE: here, we are deriving the Repo with the proc_macro_derive fn from above
    let repo = quote! {
        #[derive(mae_repo_macro::MaeRepo, sqlx::FromRow, serde::Serialize, serde::Deserialize, Clone)]
        pub struct #repo_ident {
            #[id] pub id: i32,
            pub sys_client: i32,
            pub status: mae::repo::default::DomainStatus,
            #(#params,)*
            pub comment: Option<String>,
            #[sqlx(json)]
            pub tags: serde_json::Value,
            #[sqlx(json)]
            pub sys_detail: serde_json::Value,
            #[from_context] pub created_by: i32,
            #[from_context] pub updated_by: i32,
            #[gen_date] pub created_at: chrono::DateTime<chrono::Utc>,
            pub updated_at: chrono::DateTime<chrono::Utc>,
        }

        impl mae::repo::__private__::Build<Context, Row, Field, PatchField> for #repo_ident {
            fn schema() -> String {
                #repo_name.to_string()
            }
        }
    };
    repo.into()
}

#[proc_macro_derive(MaeRepo, attributes(id, from_context, gen_date))]
pub fn derive_mae_repo(item: TokenStream) -> TokenStream {
    let ast = parse_macro_input!(item as DeriveInput);

    // Making sure it the derive macro is called on a struct;
    let fields = match &ast.data {
        Data::Struct(DataStruct {
            fields: Fields::Named(fields),
            ..
        }) => &fields.named,
        _ => panic!("expected a struct with named fields"),
    };

    let repo_ident = &ast.ident;
    let (repo_option, repo_options_ident) = as_option(&ast);
    let (repo_typed, repo_typed_ident) = as_typed(&ast);
    let (repo_variant, repo_variant_ident) = as_variant(&ast);

    quote! {
        #repo_option
        #repo_variant
        #repo_typed

    }
    .into()
}

type Body = proc_macro2::TokenStream;
type BodyIdent = proc_macro2::TokenStream;

fn as_typed(ast: &DeriveInput) -> (Body, BodyIdent) {
    let fields = match &ast.data {
        Data::Struct(DataStruct {
            fields: Fields::Named(fields),
            ..
        }) => &fields.named,
        _ => panic!("expected a struct with named fields"),
    };
    let mut to_arg = vec![];
    let mut to_string = vec![];
    let body_ident = quote! {PatchField};
    let typed_enum = fields.iter().map(|f| {
        let name = &f.ident;
        let ty = &f.ty;
        let name_str = f.ident.as_ref().unwrap().to_string();
        to_arg.push(quote! {
            #body_ident::#name(arg) => args.add(arg)
        });
        to_string.push(quote! {
            #body_ident::#name(_) => #name_str.to_string()
        });
        quote! {#name(#ty)}
    });
    let body = quote! {
        enum #body_ident {
            #(#typed_enum,)*
        }

        impl std::fmt::Display for #body_ident {
            fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
                write!(f, "{}", match self {
                    #(#to_string,)*
                })
            }
        }

        impl mae::repo::__private__::ToSql for #body_ident {
            fn sql_insert(&self) -> String {
                panic!("SQL_UPDATE NOT IMPLEMENTED")
            }
            fn sql_update(&self) -> String {
                panic!("SQL_UPDATE NOT IMPLEMENTED")
            }
            fn sql_select(&self) -> String {
                panic!("SQL_SELECT NOT IMPLEMENTED")
            }
            fn sql_patch(&self) -> String {
                // TODO: This has to look something like this for an update many:
                //UPDATE users u
                // SET
                //     name = v.name,
                //     age  = v.age
                // FROM (
                //     VALUES
                //         (1, 'Alice', 30),
                //         (2, 'Bob',   25),
                //         (3, 'Carol', 40)
                // ) AS v(id, name, age)
                // WHERE u.id = v.id;
                self.to_string()
            }
        }

        impl mae::repo::__private__::BindArgs for #body_ident {
            fn bind(&self, mut args: &mut sqlx::postgres::PgArguments) {
                let _ = match self {
                    #(#to_arg,)*
                };
            }
            fn bind_len(&self) -> usize {
                // NOTE: There will always be one arg for a PatchField
                1
            }
        }
    };
    (body, body_ident)
}

fn as_variant(ast: &DeriveInput) -> (Body, BodyIdent) {
    let fields = match &ast.data {
        Data::Struct(DataStruct {
            fields: Fields::Named(fields),
            ..
        }) => &fields.named,
        _ => panic!("expected a struct with named fields"),
    };
    let mut to_string = vec![];
    let body_ident = quote! {Field};
    let variant = fields.iter().map(|f| {
        let name = &f.ident;
        let ty = &f.ty;
        let name_str = f.ident.as_ref().unwrap().to_string();
        to_string.push(quote! {
            #body_ident::#name => #name_str.to_string()
        });
        quote! {#name}
    });
    let body = quote! {
        enum #body_ident {
            #(#variant,)*
        }

        impl #body_ident {
            fn sql(&self) -> String {
                match self {
                    #(#to_string,)*
                }
            }
        }

        impl std::fmt::Display for #body_ident {
            fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
                write!(f, "{}", match self {
                    #(#to_string,)*
                })
            }
        }
    };
    (body, body_ident)
}
fn as_option(ast: &DeriveInput) -> (Body, BodyIdent) {
    let fields = match &ast.data {
        Data::Struct(DataStruct {
            fields: Fields::Named(fields),
            ..
        }) => &fields.named,
        _ => panic!("expected a struct with named fields"),
    };
    let body_ident = quote! { Row };
    let typed = fields.iter().map(|f| {
        // TODO: some fields are auto generated from ctx, others from things like now().
        // These fields should't be accessable in the option.
        // these are tagged, filter on that.
        // get_date, id, and from_context are the tags, but for the from context, we should add a
        // function to generate that context
        let name = &f.ident;
        let ty = &f.ty;
        quote! {#name: Option<#ty>}
    });
    let string_some = fields.iter().map(|f| {
        let name = &f.ident;
        let name_str = name.as_ref().unwrap().to_string();
        quote! {
            if let Some(v) = &self.#name {
                sql.push(format!("{}", #name_str));
                sql_i.push(format!("${}", i));
                i += 1;
            }
        }
    });
    let bind_some = fields.iter().map(|f| {
        let name = &f.ident;
        quote! {
            if let Some(v) = &self.#name {
                args.add(v);
            }
        }
    });
    let bind_len = fields.iter().map(|f| {
        let name = &f.ident;
        quote! {
            if let Some(v) = &self.#name {
                count += 1;
            }
        }
    });
    let body = quote! {
        struct #body_ident {
            #(#typed,)*
        }

        impl #body_ident {
            fn sql(&self) -> (String, String) {
                let mut i = 1;
                let mut sql = vec![];
                let mut sql_i = vec![];
                #(#string_some)*

                return (sql.join(", "), sql_i.join(", "))
            }
        }

        impl mae::repo::__private__::ToSql for #body_ident {
            fn sql_insert(&self) -> String {
                let (fields_str, values_str) = self.sql();
                return format!("({}) VALUES ({})", fields_str, values_str);
            }
            fn sql_update(&self) -> String {
                        let (fields_str, values_str) = self.sql();
                        // TODO: This has to look something like this for an update many:
                        //UPDATE users u
                        // SET
                        //     name = v.name,
                        //     age  = v.age
                        // FROM (
                        //     VALUES
                        //         (1, 'Alice', 30),
                        //         (2, 'Bob',   25),
                        //         (3, 'Carol', 40)
                        // ) AS v(id, name, age)
                        // WHERE u.id = v.id;
                        return format!("({fields_str}) = (VALUES ({values_str}))");
            }
            fn sql_select(&self) -> String {
                panic!("SQL_SELECT NOT IMPLEMENTED")
            }
            fn sql_patch(&self) -> String {
                panic!("SQL_PATCH NOT IMPLEMENTED")
            }
        }

        impl mae::repo::__private__::BindArgs for #body_ident {
            fn bind(&self, mut args: &mut sqlx::postgres::PgArguments) {
                #(#bind_some)*
            }
            fn bind_len(&self) -> usize {
                let mut count = 0;
                #(#bind_len)*
                count
            }
        }
    };
    (body, body_ident)
}
