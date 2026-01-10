extern crate proc_macro;
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
            write!(f, "{}", format!("{:?}", self).to_lowercase())
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

        impl SelectBuilder<#columns_ident, #repo_ident, RequestContext> for SelectRepo<#columns_ident> {
            fn get_sys_client_field() -> #columns_ident {
                #columns_ident::sys_client
            }

            fn get_repo_name() -> String {
                #repo_ident::get_repo_name()
            }
        }

        impl WhereBuilder<#columns_ident> for SelectRepo<#columns_ident> {
            fn get_where_block(&self) -> WhereBlock<#columns_ident> {
                self.where_block.clone()
            }
            fn copy_with(&self, where_block: WhereBlock<#columns_ident>) -> SelectRepo<#columns_ident> {
                SelectRepo {
                    where_block: where_block,
                    build_string: Some(self.get_where_string(None))
                }
            }
        }

        impl UpdateBuilder<#update_columns_ident, #columns_ident, #repo_ident, RequestContext> for UpdateRepo<#update_columns_ident, #columns_ident> {
            fn get_update_block(&self) -> &Vec<#update_columns_ident> {
                return &self.update_block;
            }
            fn get_repo_name() -> String {
                #repo_ident::get_repo_name()
            }
            fn get_sys_client_field() -> #columns_ident {
                #columns_ident::sys_client
            }
        }

        impl WhereBuilder<#columns_ident> for UpdateRepo<#update_columns_ident, #columns_ident> {
            fn get_where_block(&self) -> WhereBlock<#columns_ident> {
                self.where_block.clone()
            }
            fn copy_with(&self, where_block: WhereBlock<#columns_ident>) -> UpdateRepo<#update_columns_ident, #columns_ident> {
                UpdateRepo {
                    update_block: self.update_block.clone().to_owned(),
                    where_block: where_block
                }
            }
        }
    }
    .into()
}

