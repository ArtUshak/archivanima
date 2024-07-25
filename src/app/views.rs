use crate::{
    app::{
        db::{
            change_user_password, list_ban_reasons, list_posts_with_pagination,
            list_users_with_pagination, search_posts_with_pagination,
            try_add_ban_reason_check_exists, try_add_invite_check_exists,
            try_add_user_check_username_and_invite, try_ban_post_check_exists,
            try_edit_ban_reason_check_exists, try_edit_user_check_exists, try_get_ban_reason,
            try_get_post, try_get_user, try_get_user_full, try_remove_invite_check_exists,
            try_unban_post_check_exists, BanReason, BanReasonIdSet, NewUser, PostVisibility, User,
            UserStatus, UsernameAndInviteCheckError,
        },
        templates::{
            AssetContext, BanReasonListTemplate, FormTemplate, IndexTemplate, PostAddTemplate,
            PostDetailTemplate, PostDetailTemplateAgeRestricted, PostDetailTemplateBanned,
            PostDetailTemplateHidden, PostEditTemplate, PostsListTemplate, PostsSearchTemplate,
            UserDetailTemplate, UsersListTemplate,
        },
    },
    auth::{Admin, Authentication, Uploader, USERNAME_COOKIE_NAME},
    utils::{
        breadcrumbs::Breadcrumb,
        csrf::CSRFProtectedForm,
        csrf_lib::CsrfToken,
        date_to_offset_date_time,
        form_definition::{FormDefinition, FormWithDefinition},
        form_extra_validation::IdField,
        pagination::PageParams,
        template_with_status::{TemplateForbidden, TemplateUnavailableForLegal},
        url_query::UrlQuery,
    },
    PaginationConfig, UploadConfig,
};
use archivanima_macros::{
    form_get_and_post, form_with_csrf, CheckCSRF, FormWithDefinition, RawForm,
};
use lazy_static::lazy_static;
use regex::Regex;
use rocket::{
    get,
    http::{Cookie, CookieJar},
    post,
    response::Redirect,
    time::Date,
    uri, Either, FromForm, State,
};
use serde::{Deserialize, Serialize};
use sqlx::{Pool, Postgres};
use std::{borrow::Cow, collections::HashMap};
use validator::{Validate, ValidationError, ValidationErrors};

use super::db::list_latest_pinned_posts;

lazy_static! {
    static ref BREADCRUMB_ROOT: Breadcrumb =
        Breadcrumb::new_with_url("archivanima".to_string(), uri!(index_get()).to_string());
    static ref BREADCRUMBS_INDEX: Vec<Breadcrumb> =
        vec![Breadcrumb::new_without_url("archivanima".to_string())];
    static ref BREADCRUMBS_REGISTRATION: Vec<Breadcrumb> = vec![
        BREADCRUMB_ROOT.clone(),
        Breadcrumb::new_without_url("регистрация".to_string()),
    ];
    static ref BREADCRUMBS_LOGIN: Vec<Breadcrumb> = vec![
        BREADCRUMB_ROOT.clone(),
        Breadcrumb::new_without_url("вход".to_string()),
    ];
    static ref BREADCRUMBS_LOGOUT: Vec<Breadcrumb> = vec![
        BREADCRUMB_ROOT.clone(),
        Breadcrumb::new_without_url("выход".to_string()),
    ];
    static ref BREADCRUMBS_CHANGE_PASSWORD: Vec<Breadcrumb> = vec![
        BREADCRUMB_ROOT.clone(),
        Breadcrumb::new_without_url("смена пароля".to_string()),
    ];
    static ref BREADCRUMB_USERS: Breadcrumb =
        Breadcrumb::new_without_url("пользователи".to_string());
    static ref BREADCRUMB_USERS_LIST: Vec<Breadcrumb> =
        vec![BREADCRUMB_ROOT.clone(), BREADCRUMB_USERS.clone()];
    static ref BREADCRUMBS_INVITE_ADD: Vec<Breadcrumb> = vec![
        BREADCRUMB_ROOT.clone(),
        Breadcrumb::new_without_url("инвайты".to_string()),
        Breadcrumb::new_without_url("добавление".to_string()),
    ];
    static ref BREADCRUMBS_INVITE_REMOVE: Vec<Breadcrumb> = vec![
        BREADCRUMB_ROOT.clone(),
        Breadcrumb::new_without_url("инвайты".to_string()),
        Breadcrumb::new_without_url("удаление".to_string()),
    ];
    static ref BREADCRUMBS_BAN_REASONS_LIST: Vec<Breadcrumb> = vec![
        BREADCRUMB_ROOT.clone(),
        Breadcrumb::new_without_url("причины бана".to_string())
    ];
    static ref BREADCRUMB_BAN_REASONS: Breadcrumb = Breadcrumb::new_with_url(
        "причины бана".to_string(),
        uri!(ban_reasons_list_get()).to_string()
    );
    static ref BREADCRUMBS_BAN_REASON_ADD: Vec<Breadcrumb> = vec![
        BREADCRUMB_ROOT.clone(),
        BREADCRUMB_BAN_REASONS.clone(),
        Breadcrumb::new_without_url("добавление".to_string())
    ];
    static ref BREADCRUMB_BAN_REASON_EDIT: Breadcrumb =
        Breadcrumb::new_without_url("изменение".to_string());
    static ref BREADCRUMB_POSTS: Breadcrumb = Breadcrumb::new_with_url(
        "посты".to_string(),
        uri!(posts_list_get(None as Option<u64>, None as Option<u64>)).to_string()
    );
    static ref BREADCRUMBS_POSTS_LIST: Vec<Breadcrumb> = vec![
        BREADCRUMB_ROOT.clone(),
        Breadcrumb::new_without_url("посты".to_string())
    ];
    static ref BREADCRUMBS_POST_ADD: Vec<Breadcrumb> = vec![
        BREADCRUMB_ROOT.clone(),
        BREADCRUMB_POSTS.clone(),
        Breadcrumb::new_without_url("добавление".to_string()),
    ];
    static ref BREADCRUMB_UPLOADS: Breadcrumb = Breadcrumb::new_without_url("файлы".to_string(),);
    static ref BREADCRUMBS_UPLOAD_ADD: Vec<Breadcrumb> = vec![
        BREADCRUMB_ROOT.clone(),
        BREADCRUMB_UPLOADS.clone(),
        Breadcrumb::new_without_url("загрузка".to_string()),
    ];
}

lazy_static! {
    static ref USERNAME_CHARACTERS_REGEX: Regex = Regex::new(r"^[a-zA-Z0-9_]+$").unwrap();
    static ref PASSWORD_CHARACTERS_REGEX: Regex = Regex::new(r"^[a-zA-Z0-9_\-/!\+=]+$").unwrap();
    static ref PASSWORD_LETTER_REGEX: Regex = Regex::new(r"[a-z]").unwrap();
    static ref PASSWORD_DIGIT_REGEX: Regex = Regex::new(r"[0-9]").unwrap();
}

#[form_with_csrf]
#[derive(
    Clone, Debug, Validate, FormWithDefinition, Deserialize, Serialize, FromForm, CheckCSRF,
)]
#[form_submit_name = "зарегистрироваться"]
pub struct RegistrationForm {
    #[validate(regex(
        path = "USERNAME_CHARACTERS_REGEX",
        code = "username_wrong_characters",
        message = "имя пользователя может содержать только латинские буквы, цифры и нижние подчёркивания (_)"
    ))]
    #[validate(length(
        min = 2,
        code = "username_too_short",
        message = "имя пользователя должно быть не короче 2 символов"
    ))]
    #[validate(length(
        max = 64,
        code = "username_too_long",
        message = "имя пользователя должно быть не длиннее 64 символов"
    ))]
    #[form_field_verbose_name = "имя пользователя"]
    username: String,

    #[form_field_type = "Password"]
    #[form_field_verbose_name = "инвайт-код"]
    invite_code: String,

    #[validate(length(
        min = 8,
        code = "password_too_short",
        message = "пароль должен быть не короче 8 символов"
    ))]
    #[validate(regex(
        path = "PASSWORD_CHARACTERS_REGEX",
        code = "password_wrong_characters",
        message = "пароль может содержать лишь латинские буквы, цифры, и символы _, -, /, +, = и !"
    ))]
    #[validate(regex(
        path = "PASSWORD_LETTER_REGEX",
        code = "password_missing_letter",
        message = "пароль должен содержать по меньшей мере одну латинскую букву"
    ))]
    #[validate(regex(
        path = "PASSWORD_DIGIT_REGEX",
        code = "password_missing_digit",
        message = "пароль должен содержать по меньшей мере одну цифру"
    ))]
    #[form_field_type = "Password"]
    #[form_field_verbose_name = "пароль"]
    password: String,

    #[validate(must_match(other = "password", message = "пароли должны совпадать"))]
    #[form_field_type = "Password"]
    #[form_field_verbose_name = "продублировать пароль"]
    password2: String,

    #[form_field_type = "Date"]
    #[form_field_optional]
    #[form_field_verbose_name = "дата рождения"]
    birth_date: Option<Date>,
}

impl RegistrationForm {
    fn new(csrf_token: &str) -> Self {
        Self {
            username: "".to_string(),
            invite_code: "".to_string(),
            password: "".to_string(),
            password2: "".to_string(),
            csrf_token: csrf_token.to_string(),
            birth_date: None,
        }
    }

    fn clear_sensitive(&self) -> Self {
        Self {
            username: self.username.clone(),
            invite_code: "".to_string(),
            password: "".to_string(),
            password2: "".to_string(),
            csrf_token: self.csrf_token.clone(),
            birth_date: self.birth_date,
        }
    }
}

#[get("/auth/register")]
pub fn registration_get(
    user: Authentication,
    csrf_token: CsrfToken,
    asset_context: &State<AssetContext>,
) -> Either<FormTemplate, Redirect> {
    if !user.is_anonymous() {
        Either::Right(Redirect::to(uri!(index_get()))) // TODO
    } else {
        Either::Left(FormTemplate {
            user,
            form: RegistrationForm::new(&csrf_token.authenticity_token())
                .get_definition(ValidationErrors::new()),
            asset_context,
            breadcrumbs: BREADCRUMBS_REGISTRATION.clone(),
        })
    }
}

#[post("/auth/register", data = "<form>")]
pub async fn registration_post<'a, 'b, 'c>(
    cookies: &'a CookieJar<'_>,
    form: CSRFProtectedForm<RegistrationForm>,
    pool: &'b State<Pool<Postgres>>,
    user: Authentication,
    asset_context: &'c State<AssetContext>,
) -> Result<Either<Redirect, FormTemplate<'c>>, crate::error::Error> {
    if !user.is_anonymous() {
        return Ok(Either::Left(Redirect::to(uri!(index_get())))); // TODO
    }

    match form.validate() {
        Ok(()) => {
            let new_user = NewUser {
                username: &form.username,
                password: &form.password,
                is_active: true,
                is_admin: false,
                is_uploader: false,
                birth_date: form.birth_date.map(date_to_offset_date_time),
            };

            match try_add_user_check_username_and_invite(new_user, &form.invite_code, pool).await? {
                Ok(()) => {
                    cookies
                        .add_private(Cookie::build((USERNAME_COOKIE_NAME, form.username.clone())));

                    Ok(Either::Left(Redirect::to(uri!(index_get())))) // TODO
                }
                Err(UsernameAndInviteCheckError::UserAlreadyExists) => {
                    let mut errors = ValidationErrors::new();
                    errors.add(
                        "username",
                        ValidationError {
                            code: Cow::from("username_already_in_use"),
                            message: Some(Cow::from("имя пользователя уже занято")),
                            params: HashMap::new(),
                        },
                    );
                    Ok(Either::Right(FormTemplate {
                        user,
                        form: form.clear_sensitive().get_definition(errors),
                        asset_context,
                        breadcrumbs: BREADCRUMBS_REGISTRATION.clone(),
                    }))
                }
                Err(UsernameAndInviteCheckError::InvalidInviteCode) => {
                    let mut errors = ValidationErrors::new();
                    errors.add(
                        "invite_code",
                        ValidationError {
                            code: Cow::from("invite_code_invalid"),
                            message: Some(Cow::from("инвайт-код недействителен")),
                            params: HashMap::new(),
                        },
                    );
                    Ok(Either::Right(FormTemplate {
                        user,
                        form: form.clear_sensitive().get_definition(errors),
                        asset_context,
                        breadcrumbs: BREADCRUMBS_REGISTRATION.clone(),
                    }))
                }
            }
        }
        Err(errors) => Ok(Either::Right(FormTemplate {
            user,
            form: form.clear_sensitive().get_definition(errors),
            asset_context,
            breadcrumbs: BREADCRUMBS_REGISTRATION.clone(),
        })),
    }
}

#[form_with_csrf]
#[derive(
    Clone, Debug, Validate, FormWithDefinition, Deserialize, Serialize, FromForm, CheckCSRF,
)]
#[form_submit_name = "войти"]
pub struct LoginForm {
    #[form_field_verbose_name = "имя пользователя"]
    username: String,
    #[form_field_type = "Password"]
    #[form_field_verbose_name = "пароль"]
    password: String,
}

impl LoginForm {
    fn new(csrf_token: &str) -> Self {
        Self {
            username: "".to_string(),
            password: "".to_string(),
            csrf_token: csrf_token.to_string(),
        }
    }

    fn clear_sensitive(&self) -> Self {
        Self {
            username: self.username.clone(),
            password: "".to_string(),
            csrf_token: self.csrf_token.clone(),
        }
    }
}

#[get("/auth/login")]
pub fn login_get(
    user: Authentication,
    csrf_token: CsrfToken,
    asset_context: &State<AssetContext>,
) -> Either<FormTemplate, Redirect> {
    if !user.is_anonymous() {
        Either::Right(Redirect::to(uri!(index_get()))) // TODO
    } else {
        Either::Left(FormTemplate {
            user,
            form: LoginForm::new(&csrf_token.authenticity_token())
                .get_definition(ValidationErrors::new()),
            asset_context,
            breadcrumbs: BREADCRUMBS_LOGIN.clone(),
        })
    }
}

#[post("/auth/login", data = "<form>")]
pub async fn login_post<'a, 'b, 'c>(
    cookies: &'a CookieJar<'_>,
    form: CSRFProtectedForm<LoginForm>,
    pool: &'b State<Pool<Postgres>>,
    user: Authentication,
    asset_context: &'c State<AssetContext>,
) -> Result<Either<Redirect, FormTemplate<'c>>, crate::error::Error> {
    if user.is_authenticated() {
        return Ok(Either::Left(Redirect::to(uri!(index_get())))); // TODO
    }

    match form.validate() {
        Ok(()) => {
            match try_get_user_full(&form.username, pool).await? {
                Some(user_real) => {
                    let verification_result = user_real.check_password(&form.password)?;
                    if verification_result {
                        cookies
                            .add_private(Cookie::build((USERNAME_COOKIE_NAME, user_real.username)));
                        Ok(Either::Left(Redirect::to(uri!(index_get()))))
                        // TODO
                    } else {
                        let mut errors = ValidationErrors::new();
                        errors.add(
                            "password",
                            ValidationError {
                                code: Cow::from("password_invalid"),
                                message: Some(Cow::from("неверный пароль")),
                                params: HashMap::new(),
                            },
                        );
                        Ok(Either::Right(FormTemplate {
                            user,
                            form: form.clear_sensitive().get_definition(errors),
                            asset_context,
                            breadcrumbs: BREADCRUMBS_LOGIN.clone(),
                        }))
                    }
                }
                None => {
                    let mut errors = ValidationErrors::new();
                    errors.add(
                        "username",
                        ValidationError {
                            code: Cow::from("username_not_found"),
                            message: Some(Cow::from("неверное имя пользователя")),
                            params: HashMap::new(),
                        },
                    );
                    Ok(Either::Right(FormTemplate {
                        user,
                        form: form.clear_sensitive().get_definition(errors),
                        asset_context,
                        breadcrumbs: BREADCRUMBS_LOGIN.clone(),
                    }))
                }
            }
        }
        Err(errors) => Ok(Either::Right(FormTemplate {
            user,
            form: form.clear_sensitive().get_definition(errors),
            asset_context,
            breadcrumbs: BREADCRUMBS_LOGIN.clone(),
        })),
    }
}

#[form_with_csrf]
#[derive(Clone, Debug, FormWithDefinition, Deserialize, Serialize, FromForm, CheckCSRF)]
#[form_submit_name = "выйти"]
pub struct LogoutForm {}

impl LogoutForm {
    fn new(csrf_token: &str) -> Self {
        Self {
            csrf_token: csrf_token.to_string(),
        }
    }
}

#[get("/auth/logout")]
pub fn logout_get(
    user: Authentication,
    csrf_token: CsrfToken,
    asset_context: &State<AssetContext>,
) -> Either<FormTemplate, Redirect> {
    if user.is_anonymous() {
        Either::Right(Redirect::to(uri!(index_get()))) // TODO
    } else {
        Either::Left(FormTemplate {
            user,
            form: LogoutForm::new(&csrf_token.authenticity_token())
                .get_definition(ValidationErrors::new()),
            asset_context,
            breadcrumbs: BREADCRUMBS_LOGOUT.clone(),
        })
    }
}

#[post("/auth/logout", data = "<_form>")]
pub async fn logout_post(
    cookies: &CookieJar<'_>,
    _form: CSRFProtectedForm<LogoutForm>,
    user: Authentication,
) -> Redirect {
    if !user.is_anonymous() {
        cookies.remove_private(Cookie::build(USERNAME_COOKIE_NAME));
    }

    cookies.remove_private(Cookie::build(crate::utils::csrf::COOKIE_NAME));

    Redirect::to(uri!(index_get())) // TODO
}

#[form_with_csrf]
#[derive(
    Clone, Debug, Validate, FormWithDefinition, Deserialize, Serialize, FromForm, CheckCSRF,
)]
#[form_submit_name = "сменить"]
pub struct ChangePasswordForm {
    #[form_field_type = "Password"]
    #[form_field_verbose_name = "старый пароль"]
    old_password: String,

    #[validate(length(
        min = 8,
        code = "password_too_short",
        message = "пароль должен быть не короче 8 символов"
    ))]
    #[validate(regex(
        path = "PASSWORD_CHARACTERS_REGEX",
        code = "password_wrong_characters",
        message = "пароль может содержать лишь латинские буквы, цифры, и символы _, -, /, +, = и !"
    ))]
    #[validate(regex(
        path = "PASSWORD_LETTER_REGEX",
        code = "password_missing_letter",
        message = "пароль должен содержать по меньшей мере одну латинскую букву"
    ))]
    #[validate(regex(
        path = "PASSWORD_DIGIT_REGEX",
        code = "password_missing_digit",
        message = "пароль должен содержать по меньшей мере одну цифру"
    ))]
    #[form_field_type = "Password"]
    #[form_field_verbose_name = "новый пароль"]
    new_password: String,
}

impl ChangePasswordForm {
    fn new(csrf_token: &str) -> Self {
        Self {
            old_password: "".to_string(),
            new_password: "".to_string(),
            csrf_token: csrf_token.to_string(),
        }
    }

    fn clear_sensitive(&self) -> Self {
        Self {
            old_password: "".to_string(),
            new_password: "".to_string(),
            csrf_token: self.csrf_token.clone(),
        }
    }
}

#[get("/auth/change-password")]
pub fn change_password_get(
    user: User,
    csrf_token: CsrfToken,
    asset_context: &State<AssetContext>,
) -> FormTemplate {
    FormTemplate {
        user: Authentication::Authenticated(user),
        form: ChangePasswordForm::new(&csrf_token.authenticity_token())
            .get_definition(ValidationErrors::new()),
        asset_context,
        breadcrumbs: BREADCRUMBS_CHANGE_PASSWORD.clone(),
    }
}

#[post("/auth/change-password", data = "<form>")]
pub async fn change_password_post<'a, 'b, 'c>(
    form: CSRFProtectedForm<ChangePasswordForm>,
    pool: &'b State<Pool<Postgres>>,
    user: User,
    asset_context: &'c State<AssetContext>,
) -> Result<Either<Redirect, FormTemplate<'c>>, crate::error::Error> {
    match form.validate() {
        Ok(()) => {
            let user_full = try_get_user_full(&user.username, pool).await?.unwrap();
            let verification_result = user_full.check_password(&form.old_password)?;
            if verification_result {
                change_user_password(&user.username, &form.new_password, pool).await?;
                Ok(Either::Left(Redirect::to(uri!(index_get()))))
                // TODO
            } else {
                let mut errors = ValidationErrors::new();
                errors.add(
                    "old_password",
                    ValidationError {
                        code: Cow::from("old_password_invalid"),
                        message: Some(Cow::from("неверный старый пароль")),
                        params: HashMap::new(),
                    },
                );
                Ok(Either::Right(FormTemplate {
                    user: Authentication::Authenticated(user),
                    form: form.clear_sensitive().get_definition(errors),
                    asset_context,
                    breadcrumbs: BREADCRUMBS_LOGIN.clone(),
                }))
            }
        }
        Err(errors) => Ok(Either::Right(FormTemplate {
            user: Authentication::Authenticated(user),
            form: form.clear_sensitive().get_definition(errors),
            asset_context,
            breadcrumbs: BREADCRUMBS_LOGIN.clone(),
        })),
    }
}

#[get("/user/by-username/<username>")]
pub async fn user_detail_get<'a, 'b, 'c>(
    user: Authentication,
    pool: &'a State<Pool<Postgres>>,
    asset_context: &'b State<AssetContext>,
    username: &'c str,
) -> Result<UserDetailTemplate<'b>, crate::error::Error> {
    let item = match &user {
        Authentication::Authenticated(user_real) if user_real.username == username => {
            user_real.clone()
        }
        _ => try_get_user(username, pool)
            .await?
            .ok_or(crate::error::Error::DoesNotExist)?,
    };

    Ok(UserDetailTemplate {
        user,
        asset_context,
        breadcrumbs: vec![
            BREADCRUMB_ROOT.clone(),
            BREADCRUMB_USERS.clone(),
            Breadcrumb::new_without_url(item.username.clone()),
        ],
        item,
    })
}

#[form_with_csrf]
#[derive(
    Clone, Debug, Validate, FormWithDefinition, Deserialize, Serialize, FromForm, CheckCSRF,
)]
#[form_submit_name = "сохранить"]
pub struct UserEditForm {
    #[form_field_type = "Radio"]
    #[form_field_verbose_name = "статус"]
    status: UserStatus,
}

impl UserEditForm {
    async fn load(
        username: &str,
        csrf_token: &str,
        pool: &State<Pool<Postgres>>,
    ) -> Result<Self, crate::error::Error> {
        match try_get_user(username, pool).await? {
            Some(user) => Ok(Self {
                status: user.into(),
                csrf_token: csrf_token.to_string(),
            }),
            None => Err(crate::error::Error::DoesNotExist),
        }
    }

    fn clear_sensitive(&self) -> Self {
        Self {
            csrf_token: self.csrf_token.clone(),
            status: self.status,
        }
    }

    async fn process(
        &self,
        username: &str,
        pool: &State<Pool<Postgres>>,
    ) -> Result<Either<Redirect, ValidationErrors>, crate::error::Error> {
        match try_edit_user_check_exists(username, self.status, pool).await? {
            Some(()) => Ok(Either::Left(Redirect::to(uri!(user_detail_get(username))))),
            None => Err(crate::error::Error::DoesNotExist),
        }
    }
}

form_get_and_post!(
    edit,
    FormTemplate,
    UserEditForm,
    user_edit,
    "/users/by-username/<username>/edit",
    vec![
        BREADCRUMB_ROOT.clone(),
        BREADCRUMB_USERS.clone(),
        Breadcrumb::new_with_url(
            username.to_string(),
            uri!(user_detail_get(username)).to_string()
        ),
        Breadcrumb::new_without_url("управление".to_string())
    ],
    (Admin),
    (username: &str),
    false
);

#[get("/users?<page_id>&<page_size>")]
pub async fn users_list_get<'a, 'b, 'c>(
    user: Authentication,
    pool: &'a State<Pool<Postgres>>,
    asset_context: &'b State<AssetContext>,
    pagination_config: &'c State<PaginationConfig>,
    page_id: Option<u64>,
    page_size: Option<u64>,
    _admin: Admin,
) -> Result<UsersListTemplate<'b>, crate::error::Error> {
    let page_params = PageParams {
        page_id,
        page_size: page_size.unwrap_or(pagination_config.default_page_size),
    };
    page_params.check(pagination_config)?;

    let page = list_users_with_pagination(pool, page_params).await?;

    Ok(UsersListTemplate {
        user,
        asset_context,
        breadcrumbs: BREADCRUMB_USERS_LIST.clone(),
        page,
        page_base: UrlQuery::new(),
    })
}

#[form_with_csrf]
#[derive(
    Clone, Debug, Validate, FormWithDefinition, Deserialize, Serialize, FromForm, CheckCSRF,
)]
#[form_submit_name = "добавить"]
pub struct InviteAddForm {
    #[validate(regex(
        path = "USERNAME_CHARACTERS_REGEX",
        code = "username_wrong_characters",
        message = "инвайт-код может содержать только латинские буквы, цифры и нижние подчёркивания (_)"
    ))]
    #[validate(length(
        min = 1,
        code = "username_too_short",
        message = "инвайт-код должен быть не короче 1 символов"
    ))]
    #[validate(length(
        max = 64,
        code = "username_too_long",
        message = "инвайт-код должен быть не длиннее 64 символов"
    ))]
    #[form_field_type = "Password"]
    #[form_field_verbose_name = "инвайт-код"]
    invite_code: String,
}

impl InviteAddForm {
    fn new(csrf_token: &str) -> Self {
        Self {
            csrf_token: csrf_token.to_string(),
            invite_code: "".to_string(),
        }
    }

    fn clear_sensitive(&self) -> Self {
        Self {
            invite_code: "".to_string(),
            csrf_token: self.csrf_token.clone(),
        }
    }

    async fn process(
        &self,
        pool: &State<Pool<Postgres>>,
    ) -> Result<Either<Redirect, ValidationErrors>, crate::error::Error> {
        match try_add_invite_check_exists(&self.invite_code, pool).await? {
            Some(()) => Ok(Either::Left(Redirect::to(uri!(index_get())))),
            None => {
                let mut errors = ValidationErrors::new();
                errors.add(
                    "invite_code",
                    ValidationError {
                        code: Cow::from("invite_already_exists"),
                        message: Some(Cow::from("инвайт-код уже существует")),
                        params: HashMap::new(),
                    },
                );
                Ok(Either::Right(errors))
            }
        }
    }
}

form_get_and_post!(
    simple,
    FormTemplate,
    InviteAddForm,
    invite_add,
    "/invites/add",
    BREADCRUMBS_INVITE_ADD.clone(),
    (Admin),
    (),
    false
);

#[form_with_csrf]
#[derive(
    Clone, Debug, Validate, FormWithDefinition, Deserialize, Serialize, FromForm, CheckCSRF,
)]
#[form_submit_name = "удалить"]
pub struct InviteRemoveForm {
    #[form_field_type = "Password"]
    #[form_field_verbose_name = "инвайт-код"]
    invite_code: String,
}

impl InviteRemoveForm {
    fn new(csrf_token: &str) -> Self {
        Self {
            csrf_token: csrf_token.to_string(),
            invite_code: "".to_string(),
        }
    }

    fn clear_sensitive(&self) -> Self {
        Self {
            invite_code: "".to_string(),
            csrf_token: self.csrf_token.clone(),
        }
    }

    async fn process(
        &self,
        pool: &State<Pool<Postgres>>,
    ) -> Result<Either<Redirect, ValidationErrors>, crate::error::Error> {
        match try_remove_invite_check_exists(&self.invite_code, pool).await? {
            Some(()) => Ok(Either::Left(Redirect::to(uri!(index_get())))),
            None => {
                let mut errors = ValidationErrors::new();
                errors.add(
                    "invite_code",
                    ValidationError {
                        code: Cow::from("invite_does_not_exist"),
                        message: Some(Cow::from("инвайт-код не существует")),
                        params: HashMap::new(),
                    },
                );
                Ok(Either::Right(errors))
            }
        }
    }
}

form_get_and_post!(
    simple,
    FormTemplate,
    InviteRemoveForm,
    invite_remove,
    "/invites/remove",
    BREADCRUMBS_INVITE_REMOVE.to_vec(),
    (Admin),
    (),
    false
);

#[get("/")]
pub async fn index_get<'a, 'b, 'c, 'd>(
    user: Authentication,
    pool: &'a State<Pool<Postgres>>,
    asset_context: &'b State<AssetContext>,
    pagination_config: &'c State<PaginationConfig>,
    upload_config: &'d State<UploadConfig>,
) -> Result<IndexTemplate<'b, 'd>, crate::error::Error> {
    let pinned_posts = list_latest_pinned_posts(pool, pagination_config.default_page_size, &user)
        .await?
        .into_iter()
        .map(|post| (post.id, post.check_visible(&user)))
        .collect();
    Ok(IndexTemplate {
        user,
        asset_context,
        breadcrumbs: BREADCRUMBS_INDEX.clone(),
        pinned_posts,
        storage: &upload_config.storage,
    })
}

#[get("/ban-reasons")]
pub async fn ban_reasons_list_get<'a, 'b>(
    user: Authentication,
    pool: &'a State<Pool<Postgres>>,
    asset_context: &'b State<AssetContext>,
    _admin: Admin,
) -> Result<BanReasonListTemplate<'b>, crate::error::Error> {
    Ok(BanReasonListTemplate {
        user,
        asset_context,
        breadcrumbs: BREADCRUMBS_BAN_REASONS_LIST.clone(),
        items: list_ban_reasons(pool).await?,
    })
}

#[form_with_csrf]
#[derive(
    Clone, Debug, Validate, FormWithDefinition, Deserialize, Serialize, FromForm, CheckCSRF,
)]
#[form_submit_name = "добавить"]
pub struct BanReasonAddForm {
    #[validate(regex(
        path = "USERNAME_CHARACTERS_REGEX",
        code = "id_wrong_characters",
        message = "ID может содержать только латинские буквы, цифры и нижние подчёркивания (_)"
    ))]
    #[validate(length(
        max = 64,
        code = "id_too_long",
        message = "ID должно быть не длиннее 64 символов"
    ))]
    #[validate(length(
        min = 1,
        code = "id_too_short",
        message = "ID должно быть не короче 1 символов"
    ))]
    #[form_field_verbose_name = "ID"]
    id: String,

    #[form_field_verbose_name = "описание"]
    description: String,
}

impl BanReasonAddForm {
    fn new(csrf_token: &str) -> Self {
        Self {
            csrf_token: csrf_token.to_string(),
            id: "".to_string(),
            description: "".to_string(),
        }
    }

    fn clear_sensitive(&self) -> Self {
        Self {
            csrf_token: self.csrf_token.clone(),
            id: self.id.clone(),
            description: self.description.clone(),
        }
    }

    async fn process(
        &self,
        pool: &State<Pool<Postgres>>,
    ) -> Result<Either<Redirect, ValidationErrors>, crate::error::Error> {
        match try_add_ban_reason_check_exists(
            BanReason {
                id: self.id.clone(),
                description: Some(self.description.clone()),
            },
            pool,
        )
        .await?
        {
            Some(()) => Ok(Either::Left(Redirect::to(uri!(ban_reasons_list_get())))),
            None => {
                let mut errors = ValidationErrors::new();
                errors.add(
                    "id",
                    ValidationError {
                        code: Cow::from("id_already_exists"),
                        message: Some(Cow::from("причина бана с таким ID уже существует")),
                        params: HashMap::new(),
                    },
                );
                Ok(Either::Right(errors))
            }
        }
    }
}

form_get_and_post!(
    simple,
    FormTemplate,
    BanReasonAddForm,
    ban_reason_add,
    "/ban-reasons/add",
    BREADCRUMBS_BAN_REASON_ADD.to_vec(),
    (Admin),
    (),
    false
);

#[form_with_csrf]
#[derive(
    Clone, Debug, Validate, FormWithDefinition, Deserialize, Serialize, FromForm, CheckCSRF,
)]
#[form_submit_name = "сохранить"]
pub struct BanReasonEditForm {
    #[form_field_verbose_name = "описание"]
    description: String,
}

impl BanReasonEditForm {
    async fn load(
        id: &str,
        csrf_token: &str,
        pool: &State<Pool<Postgres>>,
    ) -> Result<Self, crate::error::Error> {
        match try_get_ban_reason(id, pool).await? {
            Some(ban_reason) => Ok(Self {
                csrf_token: csrf_token.to_string(),
                description: ban_reason.description.unwrap_or("".to_string()),
            }),
            None => Err(crate::error::Error::DoesNotExist),
        }
    }

    fn clear_sensitive(&self) -> Self {
        Self {
            csrf_token: self.csrf_token.clone(),
            description: self.description.clone(),
        }
    }

    async fn process(
        &self,
        id: &str,
        pool: &State<Pool<Postgres>>,
    ) -> Result<Either<Redirect, ValidationErrors>, crate::error::Error> {
        match try_edit_ban_reason_check_exists(
            BanReason {
                id: id.to_string(),
                description: Some(self.description.clone()),
            },
            pool,
        )
        .await?
        {
            Some(()) => Ok(Either::Left(Redirect::to(uri!(ban_reasons_list_get())))),
            None => Err(crate::error::Error::DoesNotExist),
        }
    }
}

form_get_and_post!(
    edit,
    FormTemplate,
    BanReasonEditForm,
    ban_reason_edit,
    "/ban-reasons/by-id/<id>/edit",
    vec![
        BREADCRUMB_ROOT.clone(),
        BREADCRUMB_BAN_REASONS.clone(),
        Breadcrumb::new_without_url(format!("изменение ({})", id))
    ],
    (Admin),
    (id: &str),
    false
);

#[get("/posts?<page_id>&<page_size>")]
pub async fn posts_list_get<'a, 'b, 'c>(
    user: Authentication,
    pool: &'a State<Pool<Postgres>>,
    asset_context: &'b State<AssetContext>,
    pagination_config: &'c State<PaginationConfig>,
    page_id: Option<u64>,
    page_size: Option<u64>,
    upload_config: &'c State<UploadConfig>,
) -> Result<PostsListTemplate<'b, 'c>, crate::error::Error> {
    let page_params = PageParams {
        page_id,
        page_size: page_size.unwrap_or(pagination_config.default_page_size),
    };
    page_params.check(pagination_config)?;

    let page_raw = list_posts_with_pagination(pool, page_params, &user).await?;
    let page = page_raw.map(|post| (post.id, post.clone().check_visible(&user)));

    Ok(PostsListTemplate {
        user,
        asset_context,
        breadcrumbs: BREADCRUMBS_POSTS_LIST.clone(),
        page,
        storage: &upload_config.storage,
        page_base: UrlQuery::new(),
    })
}

#[get("/posts/by-id/<id>")]
pub async fn post_detail_get<'a, 'b, 'c>(
    user: Authentication,
    pool: &'a State<Pool<Postgres>>,
    asset_context: &'b State<AssetContext>,
    id: i64,
    upload_config: &'c State<UploadConfig>,
) -> Result<
    Either<
        PostDetailTemplate<'b, 'c>,
        Either<
            Either<
                TemplateForbidden<PostDetailTemplateHidden<'b>>,
                TemplateForbidden<PostDetailTemplateAgeRestricted<'b>>,
            >,
            TemplateUnavailableForLegal<PostDetailTemplateBanned<'b>>,
        >,
    >,
    crate::error::Error,
> {
    let post = try_get_post(id, pool, &user)
        .await?
        .ok_or(crate::error::Error::DoesNotExist)?;
    let post_id = post.id;

    match post.check_visible(&user) {
        PostVisibility::Visible(post) => Ok(Either::Left(PostDetailTemplate {
            user,
            asset_context,
            breadcrumbs: vec![
                BREADCRUMB_ROOT.clone(),
                BREADCRUMB_POSTS.clone(),
                Breadcrumb::new_without_url(format!("#{}: {}", post.id, &post.title)),
            ],
            item: post,
            storage: &upload_config.storage,
        })),
        PostVisibility::Hidden => Ok(Either::Right(Either::Left(Either::Left(
            TemplateForbidden {
                template: PostDetailTemplateHidden {
                    user,
                    asset_context,
                    breadcrumbs: vec![
                        BREADCRUMB_ROOT.clone(),
                        BREADCRUMB_POSTS.clone(),
                        Breadcrumb::new_without_url(format!("#{}", post_id)),
                    ],
                    item_id: post_id,
                },
            },
        )))),
        PostVisibility::AgeRestricted(min_age) => Ok(Either::Right(Either::Left(Either::Right(
            TemplateForbidden {
                template: PostDetailTemplateAgeRestricted {
                    user,
                    asset_context,
                    breadcrumbs: vec![
                        BREADCRUMB_ROOT.clone(),
                        BREADCRUMB_POSTS.clone(),
                        Breadcrumb::new_without_url(format!("#{}", post_id)),
                    ],
                    item_id: post_id,
                    min_age,
                },
            },
        )))),
        PostVisibility::Banned(ban_reason, ban_reason_text) => {
            Ok(Either::Right(Either::Right(TemplateUnavailableForLegal {
                template: PostDetailTemplateBanned {
                    user,
                    asset_context,
                    breadcrumbs: vec![
                        BREADCRUMB_ROOT.clone(),
                        BREADCRUMB_POSTS.clone(),
                        Breadcrumb::new_without_url(format!("#{}", post_id)),
                    ],
                    item_id: post_id,
                    ban_reason,
                    ban_reason_text,
                },
            })))
        }
    }
}

#[get("/posts/add")]
pub fn post_add_get(
    user: User,
    csrf_token: CsrfToken,
    asset_context: &State<AssetContext>,
    _uploader: Uploader,
) -> PostAddTemplate {
    PostAddTemplate {
        user: Authentication::Authenticated(user),
        asset_context,
        breadcrumbs: BREADCRUMBS_POST_ADD.clone(),
        csrf_token: csrf_token.authenticity_token(),
    }
}

#[get("/posts/by-id/<id>/edit")]
#[allow(clippy::too_many_arguments)]
pub async fn post_edit_get<'a, 'b, 'c>(
    id: i64,
    user: User,
    authentication: Authentication,
    csrf_token: CsrfToken,
    asset_context: &'a State<AssetContext>,
    pool: &'b State<Pool<Postgres>>,
    _uploader: Uploader,
    upload_config: &'c State<UploadConfig>,
) -> Result<PostEditTemplate<'a, 'c>, crate::error::Error> {
    let post = try_get_post(id, pool, &authentication)
        .await?
        .ok_or(crate::error::Error::DoesNotExist)?;

    if !post.can_edit_by_user(&user) {
        return Err(crate::error::Error::AccessDenied);
    }

    Ok(PostEditTemplate {
        user: Authentication::Authenticated(user),
        asset_context,
        breadcrumbs: vec![
            BREADCRUMB_ROOT.clone(),
            BREADCRUMB_POSTS.clone(),
            Breadcrumb::new_with_url(format!("#{}", id), uri!(post_detail_get(id)).to_string()),
            Breadcrumb::new_without_url("изменение".to_string()),
        ],
        csrf_token: csrf_token.authenticity_token(),
        item: post,
        storage: &upload_config.storage,
    })
}

#[form_with_csrf]
#[derive(RawForm, Clone, Debug, Validate, FormWithDefinition)]
#[form_submit_name = "забанить"]
pub struct PostBanForm {
    #[extra_validated(crate::app::db::BanReasonIdSet)]
    #[form_field_type = "RadioId"]
    #[form_field_verbose_name = "причина бана"]
    ban_reason_id: IdField,

    #[validate(length(
        max = 500,
        code = "ban_reason_text_too_long",
        message = "описание должно быть не длиннее 500 символов"
    ))]
    #[form_field_verbose_name = "описание причины бана"]
    ban_reason_text: String,
}

impl PostBanForm {
    async fn load(
        id: i64,
        ban_reason_id_set: BanReasonIdSet,
        user: &Authentication,
        csrf_token: &str,
        pool: &State<Pool<Postgres>>,
    ) -> Result<Self, crate::error::Error> {
        match try_get_post(id, pool, user).await? {
            Some(post) => match post.ban {
                Some((ban_reason, ban_reason_text)) => Ok(Self {
                    csrf_token: csrf_token.to_string(),
                    ban_reason_id: IdField::load(
                        ban_reason.map(|ban_reason| ban_reason.id),
                        &ban_reason_id_set,
                    )
                    .0,
                    ban_reason_text: ban_reason_text.unwrap_or("".to_string()),
                }),
                None => Ok(Self {
                    csrf_token: csrf_token.to_string(),
                    ban_reason_id: IdField::load(None, &ban_reason_id_set).0,
                    ban_reason_text: "".to_string(),
                }),
            },
            None => Err(crate::error::Error::DoesNotExist),
        }
    }

    fn clear_sensitive(&self) -> Self {
        Self {
            csrf_token: self.csrf_token.clone(),
            ban_reason_id: self.ban_reason_id.clone(),
            ban_reason_text: self.ban_reason_text.clone(),
        }
    }

    async fn process(
        &self,
        id: i64,
        _ban_reason_id_set: BanReasonIdSet,
        _user: &Authentication,
        pool: &State<Pool<Postgres>>,
    ) -> Result<Either<Redirect, ValidationErrors>, crate::error::Error> {
        match try_ban_post_check_exists(
            id,
            self.ban_reason_id.value.clone(),
            if self.ban_reason_text.is_empty() {
                None
            } else {
                Some(self.ban_reason_text.clone())
            },
            pool,
        )
        .await?
        {
            Some(()) => Ok(Either::Left(Redirect::to(uri!(post_detail_get(id))))),
            None => Err(crate::error::Error::DoesNotExist),
        }
    }
}

form_get_and_post!(
    edit_extra,
    FormTemplate,
    PostBanForm,
    post_ban,
    "/posts/by-id/<id>/ban",
    vec![
        BREADCRUMB_ROOT.clone(),
        BREADCRUMB_POSTS.clone(),
        Breadcrumb::new_with_url(
            format!("#{}", id),
            uri!(post_detail_get(id)).to_string()
        ),
        Breadcrumb::new_without_url("бан".to_string())
    ],
    (Admin),
    (id: i64, ban_reason_id_set: BanReasonIdSet),
    true
);

#[form_with_csrf]
#[derive(
    Clone, Debug, Validate, FormWithDefinition, Deserialize, Serialize, FromForm, CheckCSRF,
)]
#[form_submit_name = "разбанить"]
pub struct PostUnbanForm {}

impl PostUnbanForm {
    async fn load(
        id: i64,
        user: &Authentication,
        csrf_token: &str,
        pool: &State<Pool<Postgres>>,
    ) -> Result<Self, crate::error::Error> {
        if try_get_post(id, pool, user).await?.is_some() {
            Ok(Self {
                csrf_token: csrf_token.to_string(),
            })
        } else {
            Err(crate::error::Error::DoesNotExist)
        }
    }

    fn clear_sensitive(&self) -> Self {
        Self {
            csrf_token: self.csrf_token.clone(),
        }
    }

    async fn process(
        &self,
        id: i64,
        _user: &Authentication,
        pool: &State<Pool<Postgres>>,
    ) -> Result<Either<Redirect, ValidationErrors>, crate::error::Error> {
        match try_unban_post_check_exists(id, pool).await? {
            Some(()) => Ok(Either::Left(Redirect::to(uri!(post_detail_get(id))))),
            None => Err(crate::error::Error::DoesNotExist),
        }
    }
}

form_get_and_post!(
    edit,
    FormTemplate,
    PostUnbanForm,
    post_unban,
    "/posts/by-id/<id>/unban",
    vec![
        BREADCRUMB_ROOT.clone(),
        BREADCRUMB_POSTS.clone(),
        Breadcrumb::new_with_url(
            format!("#{}", id),
            uri!(post_detail_get(id)).to_string()
        ),
        Breadcrumb::new_without_url("разбан".to_string())
    ],
    (Admin),
    (id: i64),
    true
);

#[get("/posts/search?<query>&<page_id>&<page_size>")]
#[allow(clippy::too_many_arguments)]
pub async fn posts_search_get<'a, 'b, 'c>(
    user: Authentication,
    pool: &'a State<Pool<Postgres>>,
    asset_context: &'b State<AssetContext>,
    pagination_config: &'c State<PaginationConfig>,
    query: Option<String>,
    page_id: Option<u64>,
    page_size: Option<u64>,
    upload_config: &'c State<UploadConfig>,
) -> Result<PostsSearchTemplate<'b, 'c>, crate::error::Error> {
    let page_params = PageParams {
        page_id,
        page_size: page_size.unwrap_or(pagination_config.default_page_size),
    };
    page_params.check(pagination_config)?;

    let page_raw = search_posts_with_pagination(pool, query.as_deref(), page_params, &user).await?;
    let page = page_raw.map(|post| (post.id, post.clone().check_visible(&user)));

    let query_string = query.clone().unwrap_or_default();

    let page_base: UrlQuery = vec![("query".to_string(), query_string.clone())]
        .into_iter()
        .collect();

    Ok(PostsSearchTemplate {
        user,
        asset_context,
        breadcrumbs: vec![
            BREADCRUMB_ROOT.clone(),
            BREADCRUMB_POSTS.clone(),
            Breadcrumb::new_without_url(if query_string.is_empty() {
                "поиск".to_string()
            } else {
                format!("поиск: {}", query_string)
            }),
        ],
        page,
        storage: &upload_config.storage,
        query_string: query,
        page_base,
    })
}
