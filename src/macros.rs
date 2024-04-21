mod macro_utils;

use std::{
    fmt::{Display, Formatter},
    ops::Deref,
};

use proc_macro::TokenStream;
use proc_macro2::{Literal, Span};
use quote::quote;
use serde::{Deserialize, Serialize};
use syn::{
    parenthesized,
    parse::{Parse, Parser},
    parse_macro_input,
    punctuated::Punctuated,
    spanned::Spanned,
    token::{Comma, Paren},
    Attribute,
    Data::Struct,
    DeriveInput, Expr, ExprLit, ExprPath, Fields, FnArg, Ident, Lit, LitBool, LitStr, Meta, Token,
    Type,
};

use macro_utils::first_letter_to_uppercase;

fn parse_form_field_verbose_name_attribute(attribute: &Attribute) -> Option<String> {
    if attribute.path().is_ident("form_field_verbose_name") {
        if let Meta::NameValue(name_value) = &attribute.meta {
            if let Expr::Lit(ExprLit {
                attrs: _,
                lit: Lit::Str(value),
            }) = &name_value.value
            {
                return Some(value.value());
            } else {
                panic!("Attribute form_field_verbose_name requires string literal");
            }
        } else {
            panic!("Attribute form_field_verbose_name should be in form of #[form_field_verbose_name = \"NAME\"]");
        }
    }

    None
}

#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
enum FieldType {
    Checkbox,
    Radio,
    RadioId,
    Date,
    Number,
    EMail,
    Telephone,
    Text,
    TextArea,
    Password,
    Url,
    Hidden,
}

#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
enum FieldProcessType {
    Regular,
    WrapSome,
    ValueList,
    ValueListLoaded,
}

impl FieldType {
    fn new(type_str: &str) -> Option<FieldType> {
        match type_str {
            "Checkbox" => Some(FieldType::Checkbox),
            "Radio" => Some(FieldType::Radio),
            "RadioId" => Some(FieldType::RadioId),
            "Date" => Some(FieldType::Date),
            "Number" => Some(FieldType::Number),
            "EMail" => Some(FieldType::EMail),
            "Telephone" => Some(FieldType::Telephone),
            "Text" => Some(FieldType::Text),
            "TextArea" => Some(FieldType::TextArea),
            "Password" => Some(FieldType::Password),
            "URL" => Some(FieldType::Url),
            "Hidden" => Some(FieldType::Hidden),
            _ => None,
        }
    }

    fn get_process_type(&self, is_optional: bool) -> FieldProcessType {
        match self {
            FieldType::Radio => FieldProcessType::ValueList,
            FieldType::RadioId => FieldProcessType::ValueListLoaded,
            FieldType::Checkbox => FieldProcessType::Regular,
            _ if is_optional => FieldProcessType::Regular,
            _ => FieldProcessType::WrapSome,
        }
    }

    fn get_form_definition_field_type(&self) -> &'static str {
        match self {
            FieldType::Checkbox => "Checkbox",
            FieldType::Radio => "Radio",
            FieldType::RadioId => "Radio",
            FieldType::Date => "Date",
            FieldType::Number => "Number",
            FieldType::EMail => "EMail",
            FieldType::Telephone => "Telephone",
            FieldType::Text => "Text",
            FieldType::TextArea => "TextArea",
            FieldType::Password => "Password",
            FieldType::Url => "URL",
            FieldType::Hidden => "Hidden",
        }
    }
}

impl Display for FieldType {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "{}",
            match self {
                FieldType::Checkbox => "Checkbox",
                FieldType::Radio => "Radio",
                FieldType::RadioId => "RadioId",
                FieldType::Date => "Date",
                FieldType::Number => "Number",
                FieldType::EMail => "EMail",
                FieldType::Telephone => "Telephone",
                FieldType::Text => "Text",
                FieldType::TextArea => "TextArea",
                FieldType::Password => "Password",
                FieldType::Url => "URL",
                FieldType::Hidden => "Hidden",
            }
        )
    }
}

fn parse_form_field_type_attribute(attribute: &Attribute) -> Option<FieldType> {
    if attribute.path().is_ident("form_field_type") {
        if let Meta::NameValue(name_value) = &attribute.meta {
            if let Expr::Lit(ExprLit {
                attrs: _,
                lit: Lit::Str(value),
            }) = &name_value.value
            {
                return FieldType::new(&value.value());
            } else {
                panic!("Attribute form_field_type requires string literal");
            }
        } else {
            panic!("Attribute form_field_type should be in form of #[form_field_type = \"NAME\"]");
        }
    }
    None
}

fn parse_form_field_optiional_attribute(attribute: &Attribute) -> Option<()> {
    if let Meta::Path(path) = &attribute.meta {
        if path.is_ident("form_field_optional") {
            Some(())
        } else {
            None
        }
    } else {
        None
    }
}

#[proc_macro_derive(
    FormWithDefinition,
    attributes(
        form_submit_name,
        form_field_verbose_name,
        form_field_type,
        form_field_optional
    )
)]
pub fn derive_form_with_definition(form: TokenStream) -> TokenStream {
    let input = parse_macro_input!(form as DeriveInput);

    if let Struct(struct_data) = input.data {
        let name = input.ident;
        let generics = input.generics;

        let mut submit_name: Option<LitStr> = None;
        for attribute in input.attrs {
            if attribute.path().is_ident("form_submit_name") {
                if let Meta::NameValue(name_value) = attribute.meta {
                    if let Expr::Lit(ExprLit {
                        attrs: _,
                        lit: Lit::Str(value),
                    }) = name_value.value
                    {
                        submit_name = Some(value);
                    } else {
                        panic!("Attribute form_submit_name requires string literal");
                    }
                } else {
                    panic!("Attribute form_submit_name should be in form of #[form_submit_name = \"NAME\"]");
                }
            }
        }

        let mut field_args = Vec::new();

        match struct_data.fields {
            Fields::Unit => {}
            Fields::Unnamed(_) => {
                panic!("FormWithDefinition can not be derived for tuple structs");
            }
            Fields::Named(named_fields) => {
                for field in named_fields.named {
                    let ident = field.ident.unwrap();
                    let name_string = ident.to_string();
                    let mut verbose_name_string =
                        first_letter_to_uppercase(&str::replace(&name_string, "_", " "));
                    let field_type_raw = field.ty;
                    let mut field_type = FieldType::Text;
                    let mut is_optional = false;
                    for attribute in field.attrs {
                        if let Some(verbose_name_real) =
                            parse_form_field_verbose_name_attribute(&attribute)
                        {
                            verbose_name_string = verbose_name_real;
                        } else if let Some(field_type_real) =
                            parse_form_field_type_attribute(&attribute)
                        {
                            field_type = field_type_real;
                        } else if parse_form_field_optiional_attribute(&attribute).is_some() {
                            is_optional = true;
                        }
                    }

                    field_args.push((
                        field_type.get_process_type(is_optional),
                        ident,
                        LitStr::new(&verbose_name_string, Span::call_site()),
                        LitStr::new(&name_string, Span::call_site()),
                        Ident::new(
                            field_type.get_form_definition_field_type(),
                            Span::call_site(),
                        ),
                        field_type_raw,
                    ));
                }
            }
        }

        let field_expressions: Vec<proc_macro2::TokenStream> = field_args.iter()
            .map(|(
                process_type,
                field_ident, verbose_name_literal, name_literal, field_type_ident, field_type_raw
            )|
            match process_type {
                FieldProcessType::WrapSome => quote! {
                    crate::utils::form_definition::FieldDefinition {
                        name: #name_literal.to_string(),
                        verbose_name: #verbose_name_literal.to_string(),
                        field_type: crate::utils::form_definition::FieldData::#field_type_ident(Some(self.#field_ident.clone())),
                        errors: (*field_errors.get(#name_literal).unwrap_or(&&vec![])).clone(),
                    }
                },
                FieldProcessType::Regular => quote! {
                    crate::utils::form_definition::FieldDefinition {
                        name: #name_literal.to_string(),
                        verbose_name: #verbose_name_literal.to_string(),
                        field_type: crate::utils::form_definition::FieldData::#field_type_ident(self.#field_ident.clone()),
                        errors: (*field_errors.get(#name_literal).unwrap_or(&&vec![])).clone(),
                    }
                },
                FieldProcessType::ValueList => quote! {
                    crate::utils::form_definition::FieldDefinition {
                        name: #name_literal.to_string(),
                        verbose_name: #verbose_name_literal.to_string(),
                        field_type: crate::utils::form_definition::FieldData::#field_type_ident(#field_type_raw::get_options(), Some(self.#field_ident.clone().get_option())),
                        errors: (*field_errors.get(#name_literal).unwrap_or(&&vec![])).clone(),
                    }
                },
                FieldProcessType::ValueListLoaded => quote! {
                    crate::utils::form_definition::FieldDefinition {
                        name: #name_literal.to_string(),
                        verbose_name: #verbose_name_literal.to_string(),
                        field_type: crate::utils::form_definition::FieldData::#field_type_ident(self.#field_ident.values.clone(), self.#field_ident.value.clone()),
                        errors: (*field_errors.get(#name_literal).unwrap_or(&&vec![])).clone(),
                    }
                }
            })
            .collect();

        let result;
        if let Some(submit_name_real) = submit_name {
            result = quote! {
                impl #generics crate::utils::form_definition::FormWithDefinition for #name #generics {
                    fn get_definition(&self, errors: validator::ValidationErrors) -> crate::utils::form_definition::FormDefinition {
                        let field_errors = errors.field_errors();
                        let fields = vec![#(#field_expressions),*];
                        return FormDefinition { fields, submit_name: Some(#submit_name_real.to_string()) };
                    }
                }
            };
        } else {
            result = quote! {
                impl #generics crate::utils::form_definition::FormWithDefinition for #name #generics {
                    fn get_definition(&self, errors: validator::ValidationErrors) -> crate::utils::form_definition::FormDefinition {
                        let field_errors = errors.field_errors();
                        let fields = vec![#(#field_expressions),*];
                        return FormDefinition { fields, submit_name: None };
                    }
                }
            };
        };

        result.into()
    } else {
        panic!("FormWithDefinition can be derived for structs only");
    }
}

#[proc_macro_attribute]
pub fn form_with_csrf(_args: TokenStream, form: TokenStream) -> TokenStream {
    let mut input = parse_macro_input!(form as DeriveInput);

    match &mut input.data {
        syn::Data::Struct(ref mut struct_data) => {
            if let syn::Fields::Named(fields) = &mut struct_data.fields {
                fields.named.push(
                    syn::Field::parse_named
                        .parse2(quote! { #[form_field_type = "Hidden"] csrf_token: String })
                        .unwrap(),
                );
            }

            quote! {
                #input
            }
            .into()
        }
        _ => panic!("`form_with_csrf` has to be used with structs "),
    }
}

fn parse_extra_validated_attribute(attribute: &Attribute) -> Option<ExprPath> {
    if attribute.path().is_ident("extra_validated") {
        if let Meta::List(list) = &attribute.meta {
            Some(
                syn::parse2(list.tokens.clone())
                    .expect("Attribute extra_validated requires type path arg"),
            )
        } else {
            panic!("Attribute extra_validated should be in form of #[extra_validated(\"TYPE\")");
        }
    } else {
        None
    }
}

#[proc_macro_derive(RawForm, attributes(extra_validated))]
#[allow(clippy::redundant_clone)]
pub fn generate_raw_form(form: TokenStream) -> TokenStream {
    let input = parse_macro_input!(form as DeriveInput);

    let name = input.ident.clone();
    let name_raw = Ident::new(&(input.ident.to_string() + "Raw"), input.ident.span());
    let vis = input.vis.clone();

    match &input.data {
        syn::Data::Struct(struct_data) => {
            if let syn::Fields::Named(fields) = &struct_data.fields {
                let mut extra_validated_field_idents = Vec::new();
                let mut regular_field_idents = Vec::new();
                let mut raw_fields = Vec::new();
                let mut field_converters = Vec::new();
                let mut id_set_types = Vec::new();
                let mut id_set_args = Vec::new();

                for field in fields.named.iter() {
                    let field_copy = field.clone();
                    let vis = field_copy.vis;
                    let ident = field_copy.ident.unwrap();
                    let extra_validated_attrs: Vec<ExprPath> = field
                        .attrs
                        .iter()
                        .filter_map(parse_extra_validated_attribute)
                        .collect();
                    if extra_validated_attrs.len() > 1 {
                        panic!("Field can not have more than one extra_validated attribute");
                    }
                    let extra_validated_attr = extra_validated_attrs.first().cloned();

                    match extra_validated_attr {
                        Some(id_set_type) => {
                            extra_validated_field_idents.push(ident.clone());
                            raw_fields.push(quote! {#vis #ident: Option<String>});

                            let id_set_index = id_set_types
                                .iter()
                                .position(|id_set_at| id_set_at == &id_set_type)
                                .unwrap_or_else(|| {
                                    let index = id_set_types.len();
                                    id_set_types.push(id_set_type.clone());
                                    id_set_args.push(Ident::new(
                                        &format!("id_set_arg_{}", index),
                                        id_set_type.span(),
                                    ));
                                    index
                                });
                            let id_set_arg = Ident::new(
                                &format!("id_set_arg_{}", id_set_index),
                                id_set_type.span(),
                            );
                            let field_name = LitStr::new(&ident.to_string(), ident.span());

                            field_converters.push(quote! {
                                let (#ident, has_err) = crate::utils::form_extra_validation::IdField::load(raw_form.#ident, #id_set_arg);
                                if has_err {
                                    errors.add(
                                        #field_name,
                                        ValidationError {
                                            code: Cow::from("invalid_id"),
                                            message: Some(Cow::from("некорректное значение")),
                                            params: HashMap::new(),
                                        },
                                    )
                                }
                            });
                        }
                        None => {
                            regular_field_idents.push(ident.clone());
                            let mut field_copy = field.clone();
                            field_copy.attrs = vec![];
                            raw_fields.push(quote! {#vis #field_copy});
                            field_converters.push(quote! {
                                let #ident = raw_form.#ident;
                            });
                        }
                    }
                }

                quote!(
                    #[derive(Clone, Debug, rocket::FromForm, CheckCSRF)]
                    #vis struct #name_raw {
                        #(#raw_fields),*
                    }

                    impl #name {
                        fn try_load(
                            raw_form: #name_raw, #(#id_set_args: &#id_set_types),*
                        ) -> crate::utils::form_extra_validation::ExtraValidatedForm<Self> {
                            let mut errors = validator::ValidationErrors::new();
                            #(#field_converters)*
                            crate::utils::form_extra_validation::ExtraValidatedForm(
                                Self {
                                    #(#extra_validated_field_idents,)*
                                    #(#regular_field_idents,)*
                                },
                                errors
                            )
                        }
                    }

                    #[rocket::async_trait]
                    impl<'r> rocket::data::FromData<'r> for crate::utils::form_extra_validation::ExtraValidatedForm<#name> {
                        type Error = Option<rocket::form::Errors<'r>>;

                        async fn from_data(
                            req: &'r rocket::request::Request<'_>, data: rocket::data::Data<'r>
                        ) -> rocket::data::Outcome<'r, Self> {
                            #(
                                let #id_set_args: #id_set_types = match req.guard().await {
                                    rocket::request::Outcome::Success(id_set) => id_set,
                                    rocket::request::Outcome::Error((status, _)) => {return rocket::data::Outcome::Error((status, None));},
                                    rocket::request::Outcome::Forward(status) => {return rocket::data::Outcome::Forward((data, status));},
                                };
                            )*

                            let raw_form_result: rocket::data::Outcome<
                                'r, crate::utils::csrf::CSRFProtectedForm<#name_raw>
                            > =
                                rocket::data::FromData::from_data(req, data).await;

                            match raw_form_result {
                                rocket::data::Outcome::Success(raw_form) => {
                                    rocket::data::Outcome::Success(#name::try_load((*raw_form).clone(), #(&#id_set_args),*))
                                },
                                rocket::data::Outcome::Error((status, err)) => {
                                    rocket::data::Outcome::Error((status, Some(err)))
                                }
                                rocket::data::Outcome::Forward((data, status)) => rocket::data::Outcome::Forward((data, status)),
                            }
                        }
                    }
                ).into()
            } else {
                panic!("`generate_raw_form` has to be used with named item structs ")
            }
        }
        _ => panic!("`generate_raw_form` has to be used with structs "),
    }
}

#[proc_macro_derive(CheckCSRF)]
pub fn derive_check_csrf(form: TokenStream) -> TokenStream {
    let input = parse_macro_input!(form as DeriveInput);

    if let Struct(_) = input.data {
        let name = input.ident;
        let generics = input.generics;

        quote! {
            impl #generics crate::utils::csrf::CheckCSRF for #name #generics {
                fn check_csrf(&self, token: &crate::utils::csrf_lib::CsrfToken) -> Result<(), crate::utils::csrf_lib::VerificationFailure> {
                    token.verify(&self.csrf_token)
                }
            }
        }
        .into()
    } else {
        panic!("CheckCSRF can be derived for structs only");
    }
}

struct FormMethodInput {
    mode: Ident,
    _comma0: Comma,
    template_name: Type,
    _comma1: Comma,
    form_type: Type,
    _comma2: Comma,
    function_name: Ident,
    _comma3: Comma,
    url: Literal,
    _comma4: Comma,
    breadcrumbs: Expr,
    _comma5: Comma,
    _paren1: Paren,
    guard_types: Punctuated<Type, Comma>,
    _comma6: Comma,
    _paren2: Paren,
    extra_args: Punctuated<FnArg, Comma>,
    _comma7: Comma,
    pass_authentication: LitBool,
}

impl Parse for FormMethodInput {
    fn parse(input: syn::parse::ParseStream) -> syn::Result<Self> {
        let mode = input.parse()?;

        let _comma0 = input.parse()?;
        let template_name = input.parse()?;

        let _comma1 = input.parse()?;
        let form_type = input.parse()?;

        let _comma2 = input.parse()?;
        let function_name = input.parse()?;

        let _comma3 = input.parse()?;
        let url = input.parse()?;

        let _comma4 = input.parse()?;
        let breadcrumbs = input.parse()?;

        let _comma5 = input.parse()?;
        let content1;
        let _paren1 = parenthesized!(content1 in input);
        let guard_types = content1.parse_terminated(Type::parse, Token![,])?;

        let _comma6 = input.parse()?;
        let content2;
        let _paren2 = parenthesized!(content2 in input);
        let extra_args = content2.parse_terminated(FnArg::parse, Token![,])?;

        let _comma7 = input.parse()?;
        let pass_authentication = input.parse()?;

        Ok(FormMethodInput {
            mode,
            _comma0,
            template_name,
            _comma1,
            form_type,
            _comma2,
            function_name,
            _comma3,
            url,
            _comma4,
            breadcrumbs,
            _comma5,
            _paren1,
            guard_types,
            _comma6,
            _paren2,
            extra_args,
            _comma7,
            pass_authentication,
        })
    }
}

#[derive(Clone, Debug)]
enum Mode {
    Simple,
    Edit,
    EditExtra,
}

impl TryFrom<Ident> for Mode {
    type Error = ();

    fn try_from(value: Ident) -> Result<Self, Self::Error> {
        match value.to_string().as_str() {
            "simple" => Ok(Self::Simple),
            "edit" => Ok(Self::Edit),
            "edit_extra" => Ok(Self::EditExtra),
            _ => Err(()),
        }
    }
}

#[proc_macro]
pub fn form_get_and_post(args: TokenStream) -> TokenStream {
    let args_input = parse_macro_input!(args as FormMethodInput);

    let mode: Mode = args_input.mode.try_into().unwrap();
    let url = args_input.url;
    let function_name_get = Ident::new(
        &(args_input.function_name.to_string() + "_get"),
        args_input.function_name.span(),
    );
    let function_name_post = Ident::new(
        &(args_input.function_name.to_string() + "_post"),
        args_input.function_name.span(),
    );
    let template_type_name = args_input.template_name;
    let form_type_name = args_input.form_type;
    let breadcrumbs = args_input.breadcrumbs;
    let guard_types: Vec<Type> = args_input.guard_types.iter().cloned().collect();
    let guard_args: Vec<Ident> = (0..guard_types.len())
        .map(|i| Ident::new(&format!("_guard_{}", i), Span::call_site()))
        .collect();
    let extra_args: Vec<FnArg> = args_input.extra_args.iter().cloned().collect();
    let extra_arg_names: Vec<Ident> = extra_args
        .iter()
        .map(|arg| match arg {
            FnArg::Receiver(_) => panic!("form_get_and_post! macro does not support self args"),
            FnArg::Typed(pat_type) => match pat_type.pat.deref() {
                syn::Pat::Ident(pat_ident) => pat_ident.ident.clone(),
                _ => panic!("form_get_and_post! macro supports simple name-type extra args only"),
            },
        })
        .collect();
    let pass_authentication = args_input.pass_authentication.value();

    let load_expr = if pass_authentication {
        quote!(
            #form_type_name::load(#(#extra_arg_names,)* &user, &csrf_token.authenticity_token(), pool)
        )
    } else {
        quote!(
            #form_type_name::load(#(#extra_arg_names,)* &csrf_token.authenticity_token(), pool)
        )
    };
    let process_expr = if pass_authentication {
        quote!(
            form.process(#(#extra_arg_names,)* &user, pool)
        )
    } else {
        quote!(
            form.process(#(#extra_arg_names,)* pool)
        )
    };

    let result = match mode {
        Mode::Simple => quote!(
            #[get(#url)]
            pub fn #function_name_get<'a>(
                user: Authentication,
                csrf_token: CsrfToken,
                asset_context: &'a rocket::State<AssetContext>,
                #(#guard_args: #guard_types,)*
                #(#extra_args,)*
            ) -> #template_type_name<'a> {
                #template_type_name {
                    form: #form_type_name::new(&csrf_token.authenticity_token())
                        .get_definition(ValidationErrors::new()),
                    user: user.into(),
                    asset_context,
                    breadcrumbs: #breadcrumbs,
                }
            }

            #[post(#url, data = "<form>")]
            pub async fn #function_name_post<'a, 'b>(
                user: Authentication,
                pool: &'a rocket::State<Pool<Postgres>>,
                form: CSRFProtectedForm<#form_type_name>,
                asset_context: &'b rocket::State<AssetContext>,
                #(#guard_args: #guard_types,)*
                #(#extra_args,)*
            ) -> Result<rocket::Either<rocket::response::Redirect, #template_type_name<'b>>, crate::error::Error> {
                match form.validate() {
                    Ok(()) => {
                        match #process_expr.await? {
                            Either::Left(redirect) => Ok(Either::Left(redirect)),
                            Either::Right(errors) => Ok(Either::Right(#template_type_name {
                                user: user.into(),
                                form: form.clear_sensitive().get_definition(errors),
                                asset_context,
                                breadcrumbs: #breadcrumbs,
                            })),
                        }
                    }
                    Err(errors) => Ok(rocket::Either::Right(#template_type_name {
                        user: user.into(),
                        form: form.clear_sensitive().get_definition(errors),
                        asset_context,
                        breadcrumbs: #breadcrumbs,
                    })),
                }
            }
        ),
        Mode::Edit => quote!(
            #[get(#url)]
            pub async fn #function_name_get<'a, 'b>(
                user: Authentication,
                pool: &'a rocket::State<Pool<Postgres>>,
                csrf_token: CsrfToken,
                asset_context: &'b rocket::State<AssetContext>,
                #(#guard_args: #guard_types,)*
                #(#extra_args,)*
            ) -> Result<#template_type_name<'b>, crate::error::Error>{
                Ok(#template_type_name {
                    form: #load_expr.await?
                        .get_definition(ValidationErrors::new()),
                    user: user.into(),
                    asset_context,
                    breadcrumbs: #breadcrumbs,
                })
            }

            #[post(#url, data = "<form>")]
            pub async fn #function_name_post<'a, 'b>(
                user: Authentication,
                pool: &'a rocket::State<Pool<Postgres>>,
                form: CSRFProtectedForm<#form_type_name>,
                asset_context: &'b rocket::State<AssetContext>,
                #(#guard_args: #guard_types,)*
                #(#extra_args,)*
            ) -> Result<rocket::Either<rocket::response::Redirect, #template_type_name<'b>>, crate::error::Error> {
                match form.validate() {
                    Ok(()) => {
                        match #process_expr.await? {
                            Either::Left(redirect) => Ok(Either::Left(redirect)),
                            Either::Right(errors) => Ok(Either::Right(#template_type_name {
                                user: user.into(),
                                form: form.clear_sensitive().get_definition(errors),
                                asset_context,
                                breadcrumbs: #breadcrumbs,
                            })),
                        }
                    }
                    Err(errors) => Ok(Either::Right(#template_type_name {
                        user: user.into(),
                        form: form.clear_sensitive().get_definition(errors),
                        asset_context,
                        breadcrumbs: #breadcrumbs,
                    })),
                }
            }
        ),
        Mode::EditExtra => quote!(
            #[get(#url)]
            pub async fn #function_name_get<'a, 'b>(
                user: Authentication,
                pool: &'a rocket::State<Pool<Postgres>>,
                csrf_token: CsrfToken,
                asset_context: &'b rocket::State<AssetContext>,
                #(#guard_args: #guard_types,)*
                #(#extra_args,)*
            ) -> Result<#template_type_name<'b>, crate::error::Error>{
                Ok(#template_type_name {
                    form: #load_expr.await?
                        .get_definition(ValidationErrors::new()),
                    user: user.into(),
                    asset_context,
                    breadcrumbs: #breadcrumbs,
                })
            }

            #[post(#url, data = "<form>")]
            pub async fn #function_name_post<'a, 'b>(
                user: Authentication,
                pool: &'a rocket::State<Pool<Postgres>>,
                form: crate::utils::form_extra_validation::ExtraValidatedForm<#form_type_name>,
                asset_context: &'b rocket::State<AssetContext>,
                #(#guard_args: #guard_types,)*
                #(#extra_args,)*
            ) -> Result<rocket::Either<rocket::response::Redirect, #template_type_name<'b>>, crate::error::Error> {
                let crate::utils::form_extra_validation::ExtraValidatedForm(form, mut errors) = form;
                match form.validate() {
                    Ok(()) => {},
                    Err(new_errors) => {
                        new_errors.field_errors().iter().for_each(
                            |(key, value_errors)| value_errors.iter().for_each(|err| errors.add(key, err.clone()))
                        );
                    }
                }
                if errors.is_empty() {
                    match #process_expr.await? {
                        Either::Left(redirect) => Ok(Either::Left(redirect)),
                        Either::Right(errors) => Ok(Either::Right(#template_type_name {
                            user: user.into(),
                            form: form.clear_sensitive().get_definition(errors),
                            asset_context,
                            breadcrumbs: #breadcrumbs,
                        })),
                    }
                } else {
                    Ok(Either::Right(#template_type_name {
                        user: user.into(),
                        form: form.clear_sensitive().get_definition(errors),
                        asset_context,
                        breadcrumbs: #breadcrumbs,
                    }))
                }
            }
        ),
    };

    result.into()
}

#[proc_macro_derive(TemplateWithQuery)]
pub fn derive_template_query(form: TokenStream) -> TokenStream {
    let input = parse_macro_input!(form as DeriveInput);
    let name = input.ident;
    let generics = input.generics;

    if let Struct(struct_data) = input.data {
        match struct_data.fields {
            Fields::Unit => {
                panic!("TemplateWithQuery can not be derived for unit structs");
            }
            Fields::Unnamed(_) => {
                panic!("TemplateWithQuery can not be derived for tuple structs");
            }
            Fields::Named(named_fields) => {
                if named_fields.named.iter().any(|field| {
                    field.ident.as_ref().map(|ident| ident.to_string())
                        == Some("query_string".to_string())
                }) {
                    quote! {
                        impl #generics crate::app::templates::TemplateWithQuery for #name #generics {
                            fn query<'template_with_query_a>(&'template_with_query_a self) -> Option<&'template_with_query_a str> {
                                self.query_string.as_ref().map(|s| s.as_str())
                            }
                        }
                    }.into()
                } else {
                    quote! {
                        impl #generics crate::app::templates::TemplateWithQuery for #name #generics {
                            fn query<'template_with_query_a>(&'template_with_query_a self) -> Option<&'template_with_query_a str> {
                                None
                            }
                        }
                    }.into()
                }
            }
        }
    } else {
        panic!("TemplateWithQuery can be derived for structs only");
    }
}
