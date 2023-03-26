use crate::{
    app::{
        db::{
            add_post, list_ban_reasons, try_add_ban_reason_check_exists,
            try_add_invite_check_exists, try_add_user_check_username_and_invite,
            try_edit_ban_reason_check_exists, try_edit_post_check_exists_and_permission,
            try_edit_user_check_exists, try_get_ban_reason, try_get_post, try_get_user,
            try_get_user_full, try_remove_invite_check_exists, BanReason, NewPost, NewUser,
            PostEdit, PostVisibility, User, UserStatus, UsernameAndInviteCheckError,
        },
        templates::{
            AssetContext, BanReasonListTemplate, FormTemplate, IndexTemplate, PostDetailTemplate,
            PostDetailTemplateBanned, PostDetailTemplateHidden, PostsListTemplate,
            UserDetailTemplate,
        },
    },
    auth::{Admin, Authentication, USERNAME_COOKIE_NAME},
    utils::{
        breadcrumbs::Breadcrumb,
        csrf::CSRFProtectedForm,
        csrf_lib::CsrfToken,
        form_definition::{FormDefinition, FormWithDefinition},
        pagination::PageParams,
        template_with_status::{TemplateForbidden, TemplateUnavailableForLegal},
    },
    PaginationConfig,
};
use lazy_static::lazy_static;
use peresvet12_macros::{form_get_and_post, form_with_csrf, CheckCSRF, FormWithDefinition};
use regex::Regex;
use rocket::{
    get,
    http::{Cookie, CookieJar},
    post,
    response::Redirect,
    uri, Either, FromForm, State,
};
use serde::{Deserialize, Serialize};
use sqlx::{Pool, Postgres};
use std::{borrow::Cow, collections::HashMap};
use validator::{Validate, ValidationError, ValidationErrors};

use super::db::list_posts_with_pagination;

lazy_static! {
    static ref BREADCRUMB_ROOT: Breadcrumb =
        Breadcrumb::new_with_url("peresvet12".to_string(), uri!(index_get()).to_string());
    static ref BREADCRUMBS_INDEX: Vec<Breadcrumb> =
        vec![Breadcrumb::new_without_url("peresvet12".to_string())];
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
    static ref BREADCRUMB_USERS: Breadcrumb =
        Breadcrumb::new_without_url("пользователи".to_string());
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
    // TODO: more password validation
    password: String,

    #[validate(must_match(other = "password", message = "Passwords must match"))]
    #[form_field_type = "Password"]
    #[form_field_verbose_name = "продублировать пароль"]
    password2: String,
}

impl RegistrationForm {
    fn new(csrf_token: &str) -> Self {
        Self {
            username: "".to_string(),
            invite_code: "".to_string(),
            password: "".to_string(),
            password2: "".to_string(),
            csrf_token: csrf_token.to_string(),
        }
    }

    fn clear_sensitive(&self) -> Self {
        Self {
            username: self.username.clone(),
            invite_code: "".to_string(),
            password: "".to_string(),
            password2: "".to_string(),
            csrf_token: self.csrf_token.clone(),
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
            };

            match try_add_user_check_username_and_invite(new_user, &form.invite_code, pool).await? {
                Ok(()) => {
                    cookies.add_private(
                        Cookie::build(USERNAME_COOKIE_NAME, form.username.clone()).finish(),
                    );

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
                        cookies.add_private(
                            Cookie::build(USERNAME_COOKIE_NAME, user_real.username).finish(),
                        );
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
        cookies.remove_private(Cookie::named(USERNAME_COOKIE_NAME));
    }

    cookies.remove_private(Cookie::named(crate::utils::csrf::COOKIE_NAME)); // TODO

    Redirect::to(uri!(index_get())) // TODO
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
    (username: &str)
);

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
    ()
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
    ()
);

#[get("/")]
pub fn index_get(user: Authentication, asset_context: &State<AssetContext>) -> IndexTemplate {
    IndexTemplate {
        user,
        asset_context,
        breadcrumbs: BREADCRUMBS_INDEX.clone(),
    }
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
    ()
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
    (id: &str)
);

#[get("/posts?<page_id>&<page_size>")]
pub async fn posts_list_get<'a, 'b, 'c>(
    user: Authentication,
    pool: &'a State<Pool<Postgres>>,
    asset_context: &'b State<AssetContext>,
    pagination_config: &'c State<PaginationConfig>,
    page_id: Option<u64>,
    page_size: Option<u64>,
) -> Result<PostsListTemplate<'b>, crate::error::Error> {
    let page_params = PageParams {
        page_id,
        page_size: page_size.unwrap_or(pagination_config.default_page_size),
    };
    page_params.check(pagination_config)?;

    let page_raw = list_posts_with_pagination(pool, page_params).await?;
    let page = page_raw.map(|post| (post.id, post.clone().check_visible(&user)));

    Ok(PostsListTemplate {
        user,
        asset_context,
        breadcrumbs: BREADCRUMBS_POSTS_LIST.clone(),
        page,
    })
}

#[get("/posts/by-id/<id>")]
pub async fn post_detail_get<'a, 'b>(
    user: Authentication,
    pool: &'a State<Pool<Postgres>>,
    asset_context: &'b State<AssetContext>,
    id: i64,
) -> Result<
    Either<
        PostDetailTemplate<'b>,
        Either<
            TemplateForbidden<PostDetailTemplateHidden<'b>>,
            TemplateUnavailableForLegal<PostDetailTemplateBanned<'b>>,
        >,
    >,
    crate::error::Error,
> {
    let post = try_get_post(id, pool)
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
        })),
        PostVisibility::Hidden => Ok(Either::Right(Either::Left(TemplateForbidden {
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
        }))),
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

#[form_with_csrf]
#[derive(
    Clone, Debug, Validate, FormWithDefinition, Deserialize, Serialize, FromForm, CheckCSRF,
)]
#[form_submit_name = "добавить"]
pub struct PostAddForm {
    #[validate(length(
        max = 500,
        code = "title_too_long",
        message = "название должен быть не длиннее 500 символов"
    ))]
    #[form_field_verbose_name = "название"]
    title: String,

    #[form_field_type = "TextArea"]
    #[form_field_verbose_name = "текст"]
    description: String,

    #[form_field_type = "Checkbox"]
    #[form_field_verbose_name = "скрыть пост"]
    is_hidden: bool,
}

impl PostAddForm {
    fn new(csrf_token: &str) -> Self {
        Self {
            csrf_token: csrf_token.to_string(),
            title: "".to_string(),
            description: "".to_string(),
            is_hidden: false,
        }
    }

    fn clear_sensitive(&self) -> Self {
        Self {
            csrf_token: self.csrf_token.clone(),
            title: self.title.clone(),
            description: self.description.clone(),
            is_hidden: self.is_hidden,
        }
    }

    async fn process(
        &self,
        user: User,
        pool: &State<Pool<Postgres>>,
    ) -> Result<Either<Redirect, ValidationErrors>, crate::error::Error> {
        let post = add_post(
            NewPost {
                title: self.title.clone(),
                description: self.description.clone(),
                is_hidden: self.is_hidden,
            },
            user,
            pool,
        )
        .await?;

        Ok(Either::Left(Redirect::to(uri!(post_detail_get(post.id)))))
    }
}

form_get_and_post!(
    simple,
    FormTemplate,
    PostAddForm,
    post_add,
    "/posts/add",
    BREADCRUMBS_POST_ADD.clone(),
    (),
    (_user1: User)
);

#[form_with_csrf]
#[derive(
    Clone, Debug, Validate, FormWithDefinition, Deserialize, Serialize, FromForm, CheckCSRF,
)]
#[form_submit_name = "сохранить"]
pub struct PostEditForm {
    #[validate(length(
        max = 500,
        code = "title_too_long",
        message = "название должен быть не длиннее 500 символов"
    ))]
    #[form_field_verbose_name = "название"]
    title: String,

    #[form_field_type = "TextArea"]
    #[form_field_verbose_name = "текст"]
    description: String,

    #[form_field_type = "Checkbox"]
    #[form_field_verbose_name = "скрыть пост"]
    is_hidden: bool,
}

impl PostEditForm {
    async fn load(
        id: i64,
        user: User,
        csrf_token: &str,
        pool: &State<Pool<Postgres>>,
    ) -> Result<Self, crate::error::Error> {
        match try_get_post(id, pool).await? {
            Some(post) => {
                if !post.can_edit_by_user(&user) {
                    Err(crate::error::Error::AccessDenied)
                } else {
                    Ok(Self {
                        csrf_token: csrf_token.to_string(),
                        title: post.title,
                        description: post.description.unwrap_or("".to_string()),
                        is_hidden: post.is_hidden,
                    })
                }
            }
            None => Err(crate::error::Error::DoesNotExist),
        }
    }

    fn clear_sensitive(&self) -> Self {
        Self {
            csrf_token: self.csrf_token.clone(),
            title: self.title.clone(),
            description: self.description.clone(),
            is_hidden: self.is_hidden,
        }
    }

    async fn process(
        &self,
        id: i64,
        user: User,
        pool: &State<Pool<Postgres>>,
    ) -> Result<Either<Redirect, ValidationErrors>, crate::error::Error> {
        match try_edit_post_check_exists_and_permission(
            PostEdit {
                id,
                title: self.title.clone(),
                description: Some(self.description.clone()),
                is_hidden: self.is_hidden,
            },
            &user,
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
    edit,
    FormTemplate,
    PostEditForm,
    post_edit,
    "/posts/by-id/<id>/edit",
    vec![
        BREADCRUMB_ROOT.clone(),
        BREADCRUMB_POSTS.clone(),
        Breadcrumb::new_with_url(
            format!("пост #{}", id),
            uri!(post_detail_get(id)).to_string()
        ),
        Breadcrumb::new_without_url("изменение".to_string())
    ],
    (Admin),
    (id: i64, user_real: User)
);
