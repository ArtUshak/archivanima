mod macro_utils;

use std::fmt::{Display, Formatter};

use lazy_static::__Deref;
use proc_macro::TokenStream;
use proc_macro2::{Literal, Span};
use quote::quote;
use serde::{Deserialize, Serialize};
use syn::{
    parenthesized,
    parse::{Parse, Parser},
    parse_macro_input,
    punctuated::Punctuated,
    token::{Comma, Paren},
    Attribute,
    Data::Struct,
    DeriveInput, Expr, ExprLit, Fields, FnArg, Ident, Lit, LitStr, Meta, Token, Type,
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

impl FieldType {
    fn new(type_str: &str) -> Option<FieldType> {
        match type_str {
            "Checkbox" => Some(FieldType::Checkbox),
            "Radio" => Some(FieldType::Radio),
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

    fn need_wrap_some(&self) -> bool {
        !matches!(self, Self::Checkbox)
    }

    fn need_value_list(&self) -> bool {
        matches!(self, Self::Radio)
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
                panic!("Attribute parse_form_field_type_attribute requires string literal");
            }
        } else {
            panic!("Attribute parse_form_field_type_attribute should be in form of #[parse_form_field_type_attribute = \"NAME\"]");
        }
    }
    None
}

#[proc_macro_derive(
    FormWithDefinition,
    attributes(form_submit_name, form_field_verbose_name, form_field_type)
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
                    for attribute in field.attrs {
                        // TODO!
                        if let Some(verbose_name_real) =
                            parse_form_field_verbose_name_attribute(&attribute)
                        {
                            verbose_name_string = verbose_name_real;
                        } else if let Some(field_type_real) =
                            parse_form_field_type_attribute(&attribute)
                        {
                            field_type = field_type_real;
                        }
                    }

                    field_args.push((
                        field_type.need_wrap_some(),
                        field_type.need_value_list(),
                        ident,
                        LitStr::new(&verbose_name_string, Span::call_site()),
                        LitStr::new(&name_string, Span::call_site()),
                        Ident::new(&field_type.to_string(), Span::call_site()),
                        field_type_raw,
                    ));
                }
            }
        }

        let field_expressions: Vec<proc_macro2::TokenStream> = field_args.iter()
            .map(|(
                need_wrap, need_value_list,
                field_ident, verbose_name_literal, name_literal, field_type_ident, field_type_raw
            )|
            if *need_value_list {
                quote! {
                    crate::utils::form_definition::FieldDefinition {
                        name: #name_literal.to_string(),
                        verbose_name: #verbose_name_literal.to_string(),
                        field_type: crate::utils::form_definition::FieldData::#field_type_ident(#field_type_raw::get_options(), Some(self.#field_ident.clone().get_option())),
                        errors: (*field_errors.get(#name_literal).unwrap_or(&&vec![])).clone(),
                    }
                }
            } else if *need_wrap {
                quote! {
                    crate::utils::form_definition::FieldDefinition {
                        name: #name_literal.to_string(),
                        verbose_name: #verbose_name_literal.to_string(),
                        field_type: crate::utils::form_definition::FieldData::#field_type_ident(Some(self.#field_ident.clone())),
                        errors: (*field_errors.get(#name_literal).unwrap_or(&&vec![])).clone(),
                    }
                }
            } else {
                quote! {
                    crate::utils::form_definition::FieldDefinition {
                        name: #name_literal.to_string(),
                        verbose_name: #verbose_name_literal.to_string(),
                        field_type: crate::utils::form_definition::FieldData::#field_type_ident(self.#field_ident.clone()),
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
        _ => panic!("`add_field` has to be used with structs "),
    }
}

#[proc_macro_derive(CheckCSRF)]
pub fn derive_check_csrf(form: TokenStream) -> TokenStream {
    let input = parse_macro_input!(form as DeriveInput);

    if let Struct(_) = input.data {
        let name = input.ident;
        let generics = input.generics;

        quote! {
            impl #generics crate::utils::form::CheckCSRF for #name #generics {
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
        })
    }
}

#[derive(Clone, Debug)]
enum Mode {
    Simple,
    Edit,
}

impl TryFrom<Ident> for Mode {
    type Error = ();

    fn try_from(value: Ident) -> Result<Self, Self::Error> {
        match value.to_string().as_str() {
            "simple" => Ok(Self::Simple),
            "edit" => Ok(Self::Edit),
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

    let result = match mode {
        Mode::Simple => quote!(
            #[get(#url)]
            pub fn #function_name_get<'a>(
                user: Authentication,
                csrf_token: CsrfToken,
                asset_context: &'a State<AssetContext>,
                #(#guard_args: #guard_types,)*
                #(#extra_args,)*
            ) -> #template_type_name<'a> {
                #template_type_name {
                    user: user.into(),
                    form: #form_type_name::new(&csrf_token.authenticity_token())
                        .get_definition(ValidationErrors::new()),
                    asset_context,
                    breadcrumbs: #breadcrumbs,
                }
            }

            #[post(#url, data = "<form>")]
            pub async fn #function_name_post<'a, 'b>(
                user: Authentication,
                pool: &'a State<Pool<Postgres>>,
                form: CSRFProtectedForm<#form_type_name>,
                asset_context: &'b State<AssetContext>,
                #(#guard_args: #guard_types,)*
                #(#extra_args,)*
            ) -> Result<Either<Redirect, #template_type_name<'b>>, crate::error::Error> {
                match form.validate() {
                    Ok(()) => {
                        match form.process(#(#extra_arg_names,)* pool).await? {
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
        Mode::Edit => quote!(
            #[get(#url)]
            pub async fn #function_name_get<'a, 'b>(
                user: Authentication,
                pool: &'a State<Pool<Postgres>>,
                csrf_token: CsrfToken,
                asset_context: &'b State<AssetContext>,
                #(#guard_args: #guard_types,)*
                #(#extra_args,)*
            ) -> Result<#template_type_name<'b>, crate::error::Error>{
                Ok(#template_type_name {
                    user: user.into(),
                    form: #form_type_name::load(#(#extra_arg_names,)* &csrf_token.authenticity_token(), pool).await?
                        .get_definition(ValidationErrors::new()),
                    asset_context,
                    breadcrumbs: #breadcrumbs,
                })
            }

            #[post(#url, data = "<form>")]
            pub async fn #function_name_post<'a, 'b>(
                user: Authentication,
                pool: &'a State<Pool<Postgres>>,
                form: CSRFProtectedForm<#form_type_name>,
                asset_context: &'b State<AssetContext>,
                #(#guard_args: #guard_types,)*
                #(#extra_args,)*
            ) -> Result<Either<Redirect, #template_type_name<'b>>, crate::error::Error> {
                match form.validate() {
                    Ok(()) => {
                        match form.process(#(#extra_arg_names,)* pool).await? {
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
    };

    result.into()
}
