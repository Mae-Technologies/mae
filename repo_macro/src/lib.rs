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

// SQL Method
enum Method {
    Insert,
    // TODO: implement the following methods
    // Select,
    // Update,
}

// check if a field has a specific attribute
fn has_attribute(field: &Field, attr_name: &str) -> bool {
    field
        .attrs
        .iter()
        .any(|attr| attr.path().is_ident(attr_name))
}

// get the TokenStream, SQL syntactical represnetation, sql field_name, and the returning sql
// field_name.
// if these return as null / empty, then they will not be populated in their respective section.
fn get_sql_parts(
    field: &Field,
    method: Method,
    i: usize,
) -> (proc_macro2::TokenStream, String, String, String) {
    let ident = &field.ident;

    let field_name = field
        .ident
        .clone()
        .as_ref()
        .map(|id| format!("{}", id.to_string()))
        .unwrap();
    match method {
        Method::Insert => {
            if has_attribute(field, "id") {
                return (quote! {}, String::from(""), String::from(""), field_name);
            }
            if has_attribute(field, "gen_date") {
                return (
                    quote! {},
                    String::from("now()"),
                    field_name.clone(),
                    field_name.clone(),
                );
            }
            if has_attribute(field, "from_context") {
                return (
                    quote! {ctx.session.user_id},
                    format!("${}", i),
                    field_name.clone(),
                    field_name.clone(),
                );
            }
            return (
                quote! {data.#ident},
                format!("${}", i),
                field_name.clone(),
                field_name.clone(),
            );
        } // _ => {} // Method::Update => todo!(),
          // Method::Select => todo!(),
    }
}

// Macro to impl Repo:
// Methods:
//  Insert(ctx, Insert[repo_name]) -> Result<impl Repo, sqlx::Error>;
//  Select(ctx, Select[repo_name]) -> Result<impl Repo, sqlx::Error>;
//  Update(ctx, Update[repo_name]) -> Result<impl Repo, sqlx::Error>;
#[proc_macro_derive(Repo, attributes(id, from_context, gen_date,))]
pub fn derive_repo(item: TokenStream) -> TokenStream {
    let ast = parse_macro_input!(item as DeriveInput);

    // Making sure it the derive macro is called on a struct;
    let fields = match &ast.data {
        Data::Struct(DataStruct {
            fields: Fields::Named(fields),
            ..
        }) => &fields.named,
        _ => panic!("expected a struct with named fields"),
    };

    // get the sql parts

    let mut idents = vec![];
    let mut sql_reprs = vec![];
    let mut field_names = vec![];
    let mut returning = vec![];

    fields.iter().enumerate().for_each(|(i, f)| {
        let (ident, sql_repr, field_name, returning_field) = get_sql_parts(f, Method::Insert, i);
        if ident.is_empty() == false {
            idents.push(ident);
        }
        if sql_repr.is_empty() == false {
            sql_reprs.push(sql_repr);
        }
        if field_name.is_empty() == false {
            field_names.push(field_name);
        }
        if returning_field.is_empty() == false {
            returning.push(returning_field);
        }
    });

    // convert sql parts into strings

    let sql_reprs_str = sql_reprs.into_iter().collect::<Vec<_>>().join(", ");
    let field_names_string: String = field_names.into_iter().collect::<Vec<_>>().join(", ");
    let returning_string: String = returning.into_iter().collect::<Vec<_>>().join(", ");

    // get the struct details
    let struct_name = &ast.ident;

    // create the SQL Method params

    let create_fn_data_type = format_ident!("Insert{}", ast.ident);
    let fields_type = format_ident!("{}Fields", ast.ident);
    let update_fields_type = format_ident!("{}UpdateFields", ast.ident);

    quote! {

            impl #struct_name {
                pub async fn insert(ctx: &RequestContext, data: #create_fn_data_type) -> Result<#struct_name, sqlx::Error> {

                    let sql = format!(
                    "INSERT INTO {} ({}) VALUES ({}) RETURNING {};",
                    #struct_name::get_repo_name(),
                    #field_names_string,
                    #sql_reprs_str,
                    #returning_string);

                    let result: #struct_name = sqlx::query_as (
                        &sql)#(.bind(#idents))*
                    .fetch_one(ctx.db_pool()).await?;

                    Ok(result)
                }

                fn update_builder(update_fields: Vec<#update_fields_type>, sys_client: u64) -> Result<UpdateRepo<#update_fields_type, #fields_type>, anyhow::Error> {
                UpdateRepo::<#update_fields_type, #fields_type>::update_builder(update_fields, sys_client)
                }

                pub fn select_builder(sys_client: u64) -> Result<SelectRepo<#fields_type>, anyhow::Error> {
                SelectRepo::<#fields_type>::select_builder(sys_client) 
                }
            }
        }
    .into()
}

// procedural macro to populate require structs for working with a PgRepo
#[proc_macro_attribute]
pub fn repo(args: TokenStream, input: TokenStream) -> TokenStream {
    let table_name = parse_macro_input!(args as LitStr);
    let ast = parse_macro_input!(input as DeriveInput);
    let table_name = table_name.value();

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

        #[derive(Repo, sqlx::FromRow, Serialize, Deserialize, Clone, Debug)]
        pub struct #repo_ident {
            #[id] pub id: i32,
            pub sys_client: i32,
            pub status: DomainStatus,
            #(#params,)*
            pub comment: Option<String>,
            #[sqlx(json)]
            pub tags: Value,
            #[sqlx(json)]
            pub sys_detail: Value,
            #[from_context] pub created_by: i32,
            #[from_context] pub updated_by: i32,
            #[gen_date] pub created_at: DateTime<Utc>,
            #[gen_date] pub updated_at: DateTime<Utc>,
        }

        impl #repo_ident {
            fn get_repo_name() -> String {
               String::from(#table_name)
            }
        }
    };

    // create DATA structs for CRUD Operations
    let params = fields.iter().map(|f| {
        let name = &f.ident;

        let ty = &f.ty;
        quote! {#name: #ty}
    });

    // REPO INSERT DATA
    let create_repo_ident = format_ident!("Insert{}", &ast.ident);
    let create_repo = quote! {
        pub struct #create_repo_ident {
            #(pub #params,)*
            pub sys_client: i32,
            pub status: DomainStatus,
            pub comment: Option<String>,
            pub tags: Value,
            pub sys_detail: Value,
        }
    };

    // Defining Column Names
    let table_columns = fields.iter().map(|f| {
        let name = &f.ident;
        quote! { #name }
    });

    let columns_ident = format_ident!("{}Fields", &ast.ident);
    let columns_enum = quote! {
        #[derive(Debug, Clone)]
        #[allow(non_camel_case_types)]
        pub enum #columns_ident {
            All,
            #(#table_columns,)*
            sys_client,
            status,
            comment,
            tags,
            sys_detail,
            id,
            created_by,
            updated_by,
            created_at,
            updated_at,
        }

        impl Display for #columns_ident {
            fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
                panic!("unimplemented!")
        }
        }

    };
    // Defining Column Names for Update
    let table_update_columns = fields.iter().map(|f| {
        let name = &f.ident;
        let ty = &f.ty;
        quote! { #name(#ty) }
    });

    let update_columns_ident = format_ident!("{}UpdateFields", &ast.ident);
    let impl_update_cols = fields.iter().map(|f| {
        let name = &f.ident;
        quote! { Self::#name(v) => args.add(v) }
    });

    let update_columns_enum = quote! {
        #[derive(Debug, Clone)]
        #[allow(non_camel_case_types)]
        pub enum #update_columns_ident {
            #(#table_update_columns,)*
            status(DomainStatus),
            comment(String),
            tags(Value),
            sys_detail(Value),
        }

        impl BindTo for #update_columns_ident {
            fn bind<'q>(&'q self, args: &mut sqlx::postgres::PgArguments) {
                let _ = match self {
                    #(#impl_update_cols,)*
                    Self::status(v) => args.add(v),
                    Self::comment(v) => args.add(v),
                    Self::tags(v) => args.add(v),
                    Self::sys_detail(v) => args.add(v),
                };
            }
        }

        impl Display for #update_columns_ident {
            fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
                write!(f, "{}", format!("{:?}", self).to_lowercase())
            }
        }
    };

    // Return the existing Repo with default fields and the structs that support SQL Methods

    quote! {
        #repo

        #create_repo

        #update_columns_enum

        #columns_enum
    }
    .into()
}

#[proc_macro_attribute]
pub fn mae_repo(args: TokenStream, input: TokenStream) -> TokenStream {

    let repo_name= parse_macro_input!(args as LitStr).value();
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

    // 
    // For ToSql Trait
    //

    // fields.iter().enumerate().for_each(|(i, f)| {
    // let (ident, sql_repr, field_name, returning_field) = get_sql_parts(f, Method::Insert, i);
    // });
    // fields = ;
    // values = ;

    // rebuild repo struct with the existing fields and default fields for the repo
    // NOTE: here, we are deriving the Repo with the proc_macro_derive fn from above
    let repo = quote! {
        #[derive(mae::repo::MaeRepo, sqlx::FromRow, serde::Serialize, serde::Deserialize, Clone, Debug)]
        pub struct #repo_ident {
            #[id] pub id: i32,
            pub sys_client: i32,
            pub status: mae::repo::fields::DomainStatus,
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

        impl mae::repo::builder::Interface<Context, _Row, Field> for #repo_ident {
        }

        impl mae::repo::builder::Build<Context, _Row, Field> for #repo_ident {
            fn table_ident() -> String {
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
        #[derive(Debug)]
        #repo_option
        #[derive(Debug)]
        #repo_variant
        #[derive(Debug)]
        #repo_typed

        #[derive(Debug)]
        enum _Row {
            Options(#repo_options_ident),
            Variant(#repo_variant_ident),
            Typed(#repo_typed_ident),
            ForUpdate(#repo_options_ident),
            ForSelect(#repo_variant_ident),
            ForInsert(#repo_options_ident),
        }

        impl mae::repo::builder::BindArgs for _Row {
            fn bind(&self, mut args: &mut sqlx::postgres::PgArguments) {
                let _ = match &self {
                    _Row::ForInsert(opts) | _Row::ForUpdate(opts) | _Row::Options(opts) => {
                        opts.bind(args);
                    },
                    _Row::ForSelect(var) | _Row::Variant(var) => {
                        // Do Nothing ... there are no bindings in a select statement
                    }
                    _ => panic!("BIND NOT IMPLEMENTED")
                };
            }
            fn bind_len(&self) -> usize {
                match &self {
                    _Row::ForInsert(opts) | _Row::ForUpdate(opts) | _Row::Options(opts) => {
                        opts.bind_len()
                    },
                    _Row::ForSelect(var) | _Row::Variant(var) => {
                        // NOTE: Variants have no bindings
                        0
                    }
                    _ => panic!("BIND NOT IMPLEMENTED")
                }
            }
        }
        impl mae::repo::builder::ToSql for _Row {
            fn sql_insert(&self) -> String {
                match &self {
                _Row::ForInsert(opts) | _Row::Options(opts) => {
                let (fields_str, values_str) = opts.sql();
                return format!("({}) VALUES ({})", fields_str, values_str);
                    },
                    _ => panic!("SQL_INSERT NOT IMPLEMENTED")
            }
            }
            fn sql_update(&self) -> String {
                match &self {
                    _Row::ForUpdate(opts) | _Row::Options(opts) => {
                        let (fields_str, values_str) = opts.sql();
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
                    },
                    _ => panic!("SQL_UPDATE NOT IMPLEMENTED")
                }
            }
            fn sql_select(&self) -> String {
                match &self {
                    _Row::ForSelect(var) | _Row::Variant(var) => {
                        let fields_str = var.sql();
                        return format!("{}", fields_str);
                    },
                    _ => panic!("SQL_SELECT NOT IMPLEMENTED")
                }
            }
        }
        impl std::fmt::Display for _Row {
            fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
                write!(f, "{}", match self {
                    Self::ForSelect(v) | Self::Variant(v) => v.to_string(),
                    _ => panic!("DISPLAY IS NOT IMPLEMENTED.")
                })
            }
        }


    }.into()
}

type Body = proc_macro2::TokenStream;
type BodyIdent = proc_macro2::TokenStream;

fn as_typed(ast: &DeriveInput) -> (Body, BodyIdent) {
    let fields = match &ast.data {
        Data::Struct(DataStruct {
            fields: Fields::Named(fields),
    ..
}) => &fields.named,
        _ => panic!("expected a struct with named fields")
    };
    let body_ident = quote! {_TypedRow};
    let typed = fields.iter().map(|f| {
        let name = &f.ident;
        let ty = &f.ty;
        quote! {#name(#ty)}
    });
    let body = quote! {
        enum _TypedRow {
            #(#typed,)*
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
        _ => panic!("expected a struct with named fields")
    };
    let mut to_string = vec![];
    let body_ident = quote! {Field};
    let variant = fields.iter().map(|f| {
        let name = &f.ident;
        let ty = &f.ty;
        let name_str = f.ident.as_ref().unwrap().to_string();
        to_string.push(quote!{
            #body_ident::#name => #name_str.to_string()
        });
        quote! {#name}
    });
    let sql_getter = fields.iter().map(|f| {
        let name = &f.ident;
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
        _ => panic!("expected a struct with named fields")
    };
    let body_ident = quote! {_OptionRow};
    let typed = fields.iter().map(|f| {
        let name = &f.ident;
        let ty = &f.ty;
        quote! {#name: Option<#ty>}
    });
    let string_some= fields.iter().map(|f| {
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
        
        impl mae::repo::builder::BindArgs for #body_ident {
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
