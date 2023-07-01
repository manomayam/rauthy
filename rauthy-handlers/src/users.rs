use crate::build_csp_header;
use actix_web::http::StatusCode;
use actix_web::{cookie, delete, get, post, put, web, HttpRequest, HttpResponse};
use actix_web_actors::ws;
use actix_web_grants::proc_macro::{has_any_permission, has_permissions, has_roles};
use actix_web_lab::sse;
use rauthy_common::constants::{
    APP_ID_HEADER, COOKIE_MFA, HEADER_HTML, OPEN_USER_REG, PWD_RESET_COOKIE,
    USER_REG_DOMAIN_RESTRICTION,
};
use rauthy_common::error_response::{ErrorResponse, ErrorResponseType};
use rauthy_models::app_state::AppState;
use rauthy_models::entity::colors::ColorEntity;
use rauthy_models::entity::mfa_app::{MfaApp, MfaAppReg};
use rauthy_models::entity::mfa_auth_code::MfaAuthCode;
use rauthy_models::entity::principal::Principal;
use rauthy_models::entity::sessions::{get_header_value, Session, SessionState};
use rauthy_models::entity::user_attr::{UserAttrConfigEntity, UserAttrValueEntity};
use rauthy_models::entity::users::User;
use rauthy_models::entity::webauthn;
use rauthy_models::mfa::app_reg_ws::RegWebSocket;
use rauthy_models::mfa::listen_ws::{MfaListenWs, WsListenRouteReq, WsListenTx};
use rauthy_models::request::{
    MfaPurpose, NewMfaAppRequest, NewUserRegistrationRequest, NewUserRequest, PasswordResetRequest,
    RequestResetRequest, UpdateUserRequest, UpdateUserSelfRequest, UserAttrConfigRequest,
    UserAttrValuesUpdateRequest, WebauthnAuthFinishRequest, WebauthnAuthStartRequest,
    WebauthnRegFinishRequest, WebauthnRegStartRequest,
};
use rauthy_models::response::{
    MfaAppRegResponse, UserAttrConfigResponse, UserAttrValueResponse, UserAttrValuesResponse,
    UserResponse,
};
use rauthy_models::templates::UserRegisterHtml;
use rauthy_service::password_reset;
use time::OffsetDateTime;
use tracing::{debug, error, info, warn};

/// Returns all existing users
///
/// **Permissions**
/// - rauthy_admin
#[utoipa::path(
    get,
    path = "/users",
    tag = "users",
    responses(
        (status = 200, description = "Ok", body = [UserResponse]),
        (status = 401, description = "Unauthorized"),
        (status = 403, description = "Forbidden"),
    ),
)]
#[get("/users")]
#[has_roles("rauthy_admin")]
pub async fn get_users(
    data: web::Data<AppState>,
    principal: web::ReqData<Option<Principal>>,
) -> Result<HttpResponse, ErrorResponse> {
    let principal = Principal::get_from_req(principal.into_inner())?;
    principal.validate_rauthy_admin()?;

    let users = User::find_all(&data).await?;
    let mut res = Vec::new();
    users
        .into_iter()
        .for_each(|u| res.push(UserResponse::from(u)));

    Ok(HttpResponse::Ok().json(res))
}

/// Adds a new user to the database
///
/// **Permissions**
/// - rauthy_admin
#[utoipa::path(
    post,
    path = "/users",
    tag = "users",
    request_body = NewUserRequest,
    responses(
        (status = 200, description = "Ok", body = UserResponse),
        (status = 401, description = "Unauthorized"),
        (status = 403, description = "Forbidden"),
    ),
)]
#[post("/users")]
#[has_roles("rauthy_admin")]
pub async fn post_users(
    data: web::Data<AppState>,
    req: HttpRequest,
    principal: web::ReqData<Option<Principal>>,
    session_req: web::ReqData<Option<Session>>,
    user: actix_web_validator::Json<NewUserRequest>,
) -> Result<HttpResponse, ErrorResponse> {
    let principal = Principal::get_from_req(principal.into_inner())?;
    principal.validate_rauthy_admin()?;
    if session_req.is_some() {
        Session::extract_validate_csrf(session_req, &req)?;
    }

    User::create_from_new(&data, user.into_inner())
        .await
        .map(|user| HttpResponse::Ok().json(UserResponse::from(user)))
}

/// Get the configured / allowed additional custom user attribute
#[utoipa::path(
    get,
    path = "/users/attr",
    tag = "users",
    responses(
        (status = 200, description = "Ok", body = UserAttrConfigResponse),
        (status = 403, description = "Forbidden"),
    ),
)]
#[get("/users/attr")]
#[has_roles("rauthy_admin")]
pub async fn get_cust_attr(
    data: web::Data<AppState>,
    principal: web::ReqData<Option<Principal>>,
) -> Result<HttpResponse, ErrorResponse> {
    let principal = Principal::get_from_req(principal.into_inner())?;
    principal.validate_rauthy_admin()?;

    UserAttrConfigEntity::find_all(&data)
        .await
        .map(|values| HttpResponse::Ok().json(UserAttrConfigResponse { values }))
}

/// Create a new allowed additional custom user attribute
#[utoipa::path(
    post,
    path = "/users/attr",
    tag = "users",
    request_body = UserAttrConfigRequest,
    responses(
        (status = 200, description = "Ok"),
        (status = 403, description = "Forbidden"),
    ),
)]
#[post("/users/attr")]
#[has_roles("rauthy_admin")]
pub async fn post_cust_attr(
    data: web::Data<AppState>,
    req: HttpRequest,
    principal: web::ReqData<Option<Principal>>,
    session_req: web::ReqData<Option<Session>>,
    req_data: actix_web_validator::Json<UserAttrConfigRequest>,
) -> Result<HttpResponse, ErrorResponse> {
    let principal = Principal::get_from_req(principal.into_inner())?;
    principal.validate_rauthy_admin()?;
    if session_req.is_some() {
        Session::extract_validate_csrf(session_req, &req)?;
    }

    UserAttrConfigEntity::create(&data, req_data.into_inner())
        .await
        .map(|attr| HttpResponse::Ok().json(attr))
}

/// Update an additional custom user attribute
///
/// The `name` of a custom attribute cannot be updated, only the description.
#[utoipa::path(
    put,
    path = "/users/attr/{name}",
    tag = "users",
    request_body = UserAttrConfigRequest,
    responses(
        (status = 200, description = "Ok"),
        (status = 403, description = "Forbidden"),
    ),
)]
#[put("/users/attr/{name}")]
#[has_roles("rauthy_admin")]
pub async fn put_cust_attr(
    data: web::Data<AppState>,
    path: web::Path<String>,
    req: HttpRequest,
    principal: web::ReqData<Option<Principal>>,
    session_req: web::ReqData<Option<Session>>,
    req_data: actix_web_validator::Json<UserAttrConfigRequest>,
) -> Result<HttpResponse, ErrorResponse> {
    let principal = Principal::get_from_req(principal.into_inner())?;
    principal.validate_rauthy_admin()?;
    if session_req.is_some() {
        Session::extract_validate_csrf(session_req, &req)?;
    }

    UserAttrConfigEntity::update(&data, path.into_inner(), req_data.into_inner())
        .await
        .map(|a| HttpResponse::Ok().json(a))
}

/// Delete an additional custom user attribute
#[utoipa::path(
    delete,
    path = "/users/attr/{name}",
    tag = "users",
    responses(
        (status = 204, description = "NoContent"),
        (status = 403, description = "Forbidden"),
        (status = 404, description = "NotFound"),
    ),
)]
#[delete("/users/attr/{name}")]
#[has_roles("rauthy_admin")]
pub async fn delete_cust_attr(
    data: web::Data<AppState>,
    path: web::Path<String>,
    req: HttpRequest,
    principal: web::ReqData<Option<Principal>>,
    session_req: web::ReqData<Option<Session>>,
) -> Result<HttpResponse, ErrorResponse> {
    let principal = Principal::get_from_req(principal.into_inner())?;
    principal.validate_rauthy_admin()?;
    if session_req.is_some() {
        Session::extract_validate_csrf(session_req, &req)?;
    }

    UserAttrConfigEntity::delete(&data, path.into_inner()).await?;
    Ok(HttpResponse::Ok().finish())
}

/// Get the HTML Page for the User Registration
#[utoipa::path(
    get,
    path = "/users/register",
    tag = "users",
    request_body = NewUserRequest,
    responses(
        (status = 200, description = "Ok"),
        (status = 403, description = "Forbidden: Open registration may be not allowed via config"),
    ),
)]
#[get("/users/register")]
#[has_permissions("all")]
pub async fn get_users_register(data: web::Data<AppState>) -> Result<HttpResponse, ErrorResponse> {
    if !*OPEN_USER_REG {
        return Err(ErrorResponse::new(
            ErrorResponseType::Forbidden,
            "Open User Registration is not allowed".to_string(),
        ));
    }

    let colors = ColorEntity::find_rauthy(&data).await?;
    let (body, nonce) = UserRegisterHtml::build(&colors);
    Ok(HttpResponse::Ok()
        .insert_header(HEADER_HTML)
        .insert_header(build_csp_header(&nonce))
        .body(body))
}

/// Creates a new user with almost all values set to default
///
/// This is the endpoint for the possibly allowed open user registration endpoint and can be
/// accessed by anyone, if configured.<br>
/// A Proof of Work (PoW) must be computed by the client to fight automatic bots and spammers.
#[utoipa::path(
    post,
    path = "/users/register",
    tag = "users",
    request_body = NewUserRequest,
    responses(
        (status = 204, description = "NoContent"),
        (status = 403, description = "Forbidden", body = ErrorResponse),
    ),
)]
#[post("/users/register")]
#[has_permissions("all")]
pub async fn post_users_register(
    data: web::Data<AppState>,
    req_data: actix_web_validator::Json<NewUserRegistrationRequest>,
) -> Result<HttpResponse, ErrorResponse> {
    if !*OPEN_USER_REG {
        return Err(ErrorResponse::new(
            ErrorResponseType::Forbidden,
            "Open User Registration is not allowed".to_string(),
        ));
    }
    if let Some(restriction) = &*USER_REG_DOMAIN_RESTRICTION {
        if !req_data.email.ends_with(restriction) {
            return Err(ErrorResponse::new(
                ErrorResponseType::BadRequest,
                format!(
                    "Domain for the open registration are restricted to '@{}'",
                    restriction
                ),
            ));
        }
    }

    User::create_from_reg(&data, req_data.into_inner()).await?;
    Ok(HttpResponse::NoContent().finish())
}

/// Returns a single user by its *id*
#[utoipa::path(
    get,
    path = "/users/{id}",
    tag = "users",
    responses(
        (status = 200, description = "Ok", body = UserResponse),
        (status = 401, description = "Unauthorized", body = ErrorResponse),
    ),
)]
#[get("/users/{id}")]
#[has_any_permission("token-auth", "session-auth")]
pub async fn get_user_by_id(
    data: web::Data<AppState>,
    path: web::Path<String>,
    principal: web::ReqData<Option<Principal>>,
) -> Result<HttpResponse, ErrorResponse> {
    let id = path.into_inner();

    // principal must either be admin or have the same user id
    let principal = Principal::get_from_req(principal.into_inner())?;
    principal.is_user_authorized_for_id(&id)?;

    User::find(&data, id)
        .await
        .map(|user| HttpResponse::Ok().json(UserResponse::from(user)))
}

/// Returns the additional custom attributes for the given user id
#[utoipa::path(
    get,
    path = "/users/{id}/attr",
    tag = "users",
    responses(
        (status = 200, description = "Ok", body = UserAttrValuesResponse),
        (status = 401, description = "Unauthorized", body = ErrorResponse),
    ),
)]
#[get("/users/{id}/attr")]
#[has_roles("rauthy_admin")]
pub async fn get_user_attr(
    data: web::Data<AppState>,
    path: web::Path<String>,
    principal: web::ReqData<Option<Principal>>,
) -> Result<HttpResponse, ErrorResponse> {
    let principal = Principal::get_from_req(principal.into_inner())?;
    principal.validate_rauthy_admin()?;

    let values = UserAttrValueEntity::find_for_user(&data, &path.into_inner())
        .await?
        .drain(..)
        .map(UserAttrValueResponse::from)
        .collect::<Vec<UserAttrValueResponse>>();

    Ok(HttpResponse::Ok().json(UserAttrValuesResponse { values }))
}

/// Updates the additional custom attributes for the given user id
#[utoipa::path(
    put,
    path = "/users/{id}/attr",
    tag = "users",
    request_body = UserAttrValuesUpdateRequest,
    responses(
        (status = 204, description = "NoContent"),
        (status = 401, description = "Unauthorized", body = ErrorResponse),
    ),
)]
#[put("/users/{id}/attr")]
#[has_roles("rauthy_admin")]
pub async fn put_user_attr(
    data: web::Data<AppState>,
    path: web::Path<String>,
    req: HttpRequest,
    principal: web::ReqData<Option<Principal>>,
    session_req: web::ReqData<Option<Session>>,
    req_data: actix_web_validator::Json<UserAttrValuesUpdateRequest>,
) -> Result<HttpResponse, ErrorResponse> {
    let principal = Principal::get_from_req(principal.into_inner())?;
    principal.validate_rauthy_admin()?;
    if session_req.is_some() {
        Session::extract_validate_csrf(session_req, &req)?;
    }

    let values =
        UserAttrValueEntity::update_for_user(&data, &path.into_inner(), req_data.into_inner())
            .await?
            .drain(..)
            .map(UserAttrValueResponse::from)
            .collect::<Vec<UserAttrValueResponse>>();
    Ok(HttpResponse::Ok().json(UserAttrValuesResponse { values }))
}

/// Endpoint for resetting passwords
///
/// The `id` is the user id and `reset_id` is a random 64 character long string sent via E-Mail for a
/// pre-authenticated request.
#[utoipa::path(
    post,
    path = "/users/{id}/reset/{reset_id}",
    tag = "users",
    responses(
        (status = 200, description = "Ok"),
        (status = 401, description = "Unauthorized", body = ErrorResponse),
        (status = 403, description = "Forbidden", body = ErrorResponse),
    ),
)]
#[get("/users/{id}/reset/{reset_id}")]
#[has_permissions("all")]
pub async fn get_user_password_reset(
    data: web::Data<AppState>,
    path: web::Path<(String, String)>,
    req: HttpRequest,
) -> Result<HttpResponse, ErrorResponse> {
    let path_inner = path.into_inner();
    password_reset::handle_get_pwd_reset(&data, req, path_inner.0, path_inner.1)
        .await
        .map(|(html, nonce, cookie)| {
            HttpResponse::Ok()
                .cookie(cookie)
                .insert_header(HEADER_HTML)
                .insert_header(build_csp_header(&nonce))
                .body(html)
        })
}

/// Endpoint for resetting passwords
///
/// On this endpoint, a password reset can be posted. This only works with a valid
/// `PWD_RESET_COOKIE` + CSRF token.
///
/// Expects the CSRF token to be provided with an HTTP Header called `PWD_CSRF_HEADER`
///
/// **Permissions**
/// - pre-authenticated with pwd-reset cookie from `GET /auth/v1/users/{id}/reset/{reset_id}`
#[utoipa::path(
    put,
    path = "/users/{id}/reset",
    tag = "users",
    request_body = PasswordResetRequest,
    responses(
        (status = 202, description = "Accepted"),
        (status = 401, description = "Unauthorized", body = ErrorResponse),
        (status = 403, description = "Forbidden", body = ErrorResponse),
    ),
)]
#[put("/users/{id}/reset")]
#[has_permissions("all")]
pub async fn put_user_password_reset(
    data: web::Data<AppState>,
    path: web::Path<String>,
    req: HttpRequest,
    req_data: actix_web_validator::Json<PasswordResetRequest>,
) -> Result<HttpResponse, ErrorResponse> {
    password_reset::handle_put_user_password_reset(
        &data,
        req,
        path.into_inner(),
        req_data.into_inner(),
    )
    .await
    .map(|cookie| {
        HttpResponse::Ok()
            .cookie(cookie)
            .status(StatusCode::ACCEPTED)
            .finish()
    })
}

/// Starts the authentication process for a WebAuthn Device for this user
///
/// **Permissions**
/// - authenticated and logged in user for this very {id}
#[utoipa::path(
    post,
    path = "/users/{id}/webauthn/auth/start",
    tag = "mfa",
    request_body = WebauthnAuthStartRequest,
    responses(
        (status = 200, description = "Ok", body = WebauthnAuthStartResponse),
        (status = 401, description = "Unauthorized", body = ErrorResponse),
        (status = 403, description = "Forbidden", body = ErrorResponse),
    ),
)]
#[post("/users/{id}/webauthn/auth/start")]
#[has_any_permission("token-auth", "session-auth", "session-init")]
pub async fn post_webauthn_auth_start(
    data: web::Data<AppState>,
    id: web::Path<String>,
    principal: Option<web::ReqData<Option<Principal>>>,
    req: HttpRequest,
    session_req: web::ReqData<Option<Session>>,
    req_data: actix_web_validator::Json<WebauthnAuthStartRequest>,
) -> Result<HttpResponse, ErrorResponse> {
    let purpose = req_data.into_inner().purpose;

    let session_state = if session_req.is_some() {
        let session = Session::extract_validate_csrf(session_req, &req)?;
        Some(session.state)
    } else {
        None
    };

    let id = match purpose {
        // only for a Login purpose, this can be accessed without authentication (yet)
        MfaPurpose::Login(_) => id.into_inner(),

        MfaPurpose::PasswordReset => {
            let binding_cookie = match req.cookie(PWD_RESET_COOKIE) {
                None => {
                    return Err(ErrorResponse::new(
                        ErrorResponseType::BadRequest,
                        "You are not allowed to do this operation without an active binding cookie"
                            .to_string(),
                    ));
                }
                Some(c) => c,
            };
            if binding_cookie.value().len() != 48 {
                return Err(ErrorResponse::new(
                    ErrorResponseType::BadRequest,
                    "Malformed magic link binding cookie".to_string(),
                ));
            }

            id.into_inner()
        }

        _ => {
            if session_state.is_some() && session_state.unwrap() == SessionState::Init {
                return Err(ErrorResponse::new(
                    ErrorResponseType::Forbidden,
                    "You are not allowed to do this operation with a Session Init State"
                        .to_string(),
                ));
            }

            // Validate that Principal matches the user.
            // If we have no Init Session at this point, we must have a principal, which is always
            // there for an authed session or with a valid JWT token.
            let principal_opt = principal.ok_or_else(|| {
                ErrorResponse::new(
                    ErrorResponseType::Forbidden,
                    "You are not allowed to access this resource without a Login".to_string(),
                )
            })?;
            let principal = Principal::get_from_req(principal_opt.into_inner())?;
            let id = id.into_inner();
            principal.validate_id(&id)?;
            id
        }
    };

    webauthn::auth_start(&data, id, purpose)
        .await
        .map(|res| HttpResponse::Ok().json(res))
}

/// Finishes the authentication process for a WebAuthn Device for this user
///
/// **Permissions**
/// - authenticated and logged in user for this very {id}
#[utoipa::path(
    post,
    path = "/users/{id}/webauthn/auth/finish",
    tag = "mfa",
    request_body = WebauthnAuthFinishRequest,
    responses(
        (status = 200, description = "Ok", body = WebauthnAdditionalData),
        (status = 401, description = "Unauthorized", body = ErrorResponse),
        (status = 403, description = "Forbidden", body = ErrorResponse),
    ),
)]
#[post("/users/{id}/webauthn/auth/finish")]
#[has_any_permission("token-auth", "session-auth", "session-init")]
pub async fn post_webauthn_auth_finish(
    data: web::Data<AppState>,
    id: web::Path<String>,
    principal: Option<web::ReqData<Option<Principal>>>,
    req: HttpRequest,
    session_req: web::ReqData<Option<Session>>,
    req_data: actix_web_validator::Json<WebauthnAuthFinishRequest>,
) -> Result<HttpResponse, ErrorResponse> {
    let session_state = if session_req.is_some() {
        let session = Session::extract_validate_csrf(session_req, &req)?;
        Some(session.state)
    } else {
        None
    };

    let id = id.into_inner();
    let res = if session_state.is_some() && session_state.unwrap() == SessionState::Init {
        // The Session is only in init state in a very tiny window, when the /oidc/authorize page has
        // been received and until the credentials have been validated.
        // As a double check, we have the 'code' from the /start endpoint.
        webauthn::auth_finish(&data, id, req_data.into_inner()).await?
    } else {
        // For any authenticated request, validate that Principal matches the user.
        let principal_opt = principal.ok_or_else(|| {
            ErrorResponse::new(
                ErrorResponseType::Forbidden,
                "You are not allowed to access this resource without a Login".to_string(),
            )
        })?;
        let principal = Principal::get_from_req(principal_opt.into_inner())?;
        principal.validate_id(&id)?;

        webauthn::auth_finish(&data, id, req_data.into_inner()).await?
    };

    Ok(res.into_response())
}

// TODO change to DELETE instead of PUT?
/// Deletes the WebAuthn Device for this user in the given slot
///
/// **Permissions**
/// - rauthy_admin
/// - authenticated and logged in user for this very {id}
#[utoipa::path(
    post,
    path = "/users/{id}/webauthn/delete/{slot}",
    tag = "mfa",
    responses(
        (status = 200, description = "Ok"),
        (status = 401, description = "Unauthorized", body = ErrorResponse),
        (status = 403, description = "Forbidden", body = ErrorResponse),
    ),
)]
#[post("/users/{id}/webauthn/delete/{slot}")]
#[has_any_permission("token-auth", "session-auth")]
pub async fn post_webauthn_delete(
    data: web::Data<AppState>,
    path: web::Path<(String, u8)>,
    principal: web::ReqData<Option<Principal>>,
    req: HttpRequest,
    session_req: web::ReqData<Option<Session>>,
) -> Result<HttpResponse, ErrorResponse> {
    if session_req.is_some() {
        Session::extract_validate_csrf(session_req, &req)?;
    }

    let (id, slot) = path.into_inner();

    // validate that Principal matches the user or is an admin
    let principal = Principal::get_from_req(principal.into_inner())?;
    if !principal.is_admin() {
        principal.validate_id(&id)?;
        warn!("MFA reset for user {} slot {}", id, slot);
    } else {
        warn!(
            "MFA reset from admin {:?} for user {} slot {}",
            principal.email, id, slot
        );
    }

    let mut user = User::find(&data, id).await?;
    user.delete_mfa_slot(&data, slot).await?;

    // make sure to delete any existing MFA cookie when a key is deleted
    let cookie = cookie::Cookie::build(COOKIE_MFA, "")
        .secure(true)
        .http_only(true)
        .same_site(cookie::SameSite::Lax)
        .max_age(cookie::time::Duration::ZERO)
        .path("/auth")
        .finish();
    let mut resp = HttpResponse::Ok().finish();
    if let Err(err) = resp.add_cookie(&cookie) {
        error!("Error deleting MFA cookie in post_webauthn_delete: {}", err);
    }
    Ok(resp)
}

/// Starts the registration process for a new WebAuthn Device for this user
///
/// **Permissions**
/// - authenticated and logged in user for this very {id}
#[utoipa::path(
    post,
    path = "/users/{id}/webauthn/register/start",
    tag = "mfa",
    request_body = WebauthnRegStartRequest,
    responses(
        (status = 200, description = "Ok - Returns a default Webauthn CreationChallengeResponse, which cannot be serialized into OpenAPI schema currently"),
        (status = 401, description = "Unauthorized", body = ErrorResponse),
        (status = 403, description = "Forbidden", body = ErrorResponse),
    ),
)]
#[post("/users/{id}/webauthn/register/start")]
#[has_any_permission("token-auth", "session-auth")]
pub async fn post_webauthn_reg_start(
    data: web::Data<AppState>,
    id: web::Path<String>,
    principal: web::ReqData<Option<Principal>>,
    req: HttpRequest,
    session_req: web::ReqData<Option<Session>>,
    req_data: actix_web_validator::Json<WebauthnRegStartRequest>,
) -> Result<HttpResponse, ErrorResponse> {
    if session_req.is_some() {
        Session::extract_validate_csrf(session_req, &req)?;
    }

    // validate that Principal matches the user
    let principal = Principal::get_from_req(principal.into_inner())?;
    let id = id.into_inner();
    principal.validate_id(&id)?;

    webauthn::reg_start(&data, id, req_data.into_inner())
        .await
        .map(|ccr| HttpResponse::Ok().json(ccr))
}

/// Finishes the registration process for a new WebAuthn Device for this user
///
/// **Permissions**
/// - authenticated and logged in user for this very {id}
#[utoipa::path(
    post,
    path = "/users/{id}/webauthn/register/finish",
    tag = "mfa",
    request_body = WebauthnRegFinishRequest,
    responses(
        (status = 201, description = "Created"),
        (status = 401, description = "Unauthorized", body = ErrorResponse),
        (status = 403, description = "Forbidden", body = ErrorResponse),
    ),
)]
#[post("/users/{id}/webauthn/register/finish")]
#[has_any_permission("token-auth", "session-auth")]
pub async fn post_webauthn_reg_finish(
    data: web::Data<AppState>,
    id: web::Path<String>,
    principal: web::ReqData<Option<Principal>>,
    req: HttpRequest,
    session_req: web::ReqData<Option<Session>>,
    req_data: actix_web_validator::Json<WebauthnRegFinishRequest>,
) -> Result<HttpResponse, ErrorResponse> {
    if session_req.is_some() {
        Session::extract_validate_csrf(session_req, &req)?;
    }

    // validate that Principal matches the user
    let principal = Principal::get_from_req(principal.into_inner())?;
    let id = id.into_inner();
    principal.validate_id(&id)?;

    webauthn::reg_finish(&data, id, req_data.into_inner()).await?;

    Ok(HttpResponse::Created().finish())
}

/// Request a password reset
///
/// This Endpoint will always return an `OK` to not provide any additional attack surface.
/// Only if the provided E-Mail exists in the Database, a password reset E-Mail will be sent out,
/// otherwise it will just be ignored but still return an `OK`.
///
/// **Permissions**
/// - authenticated
/// - session-init
/// - session-auth
#[utoipa::path(
    post,
    path = "/users/request_reset",
    tag = "users",
    request_body = RequestResetRequest,
    responses(
        (status = 200, description = "Ok"),
        (status = 401, description = "Unauthorized", body = ErrorResponse),
        (status = 403, description = "Forbidden", body = ErrorResponse),
    ),
)]
#[post("/users/request_reset")]
#[has_any_permission("token-auth", "session-init", "session-auth")]
pub async fn post_user_password_request_reset(
    data: web::Data<AppState>,
    req: HttpRequest,
    req_data: actix_web_validator::Json<RequestResetRequest>,
    session_req: web::ReqData<Option<Session>>,
) -> Result<HttpResponse, ErrorResponse> {
    if session_req.is_some() {
        Session::extract_validate_csrf(session_req, &req)?;
    }

    let user = User::find_by_email(&data, req_data.email.clone()).await?;
    user.request_password_reset(&data, req)
        .await
        .map(|_| HttpResponse::Ok().status(StatusCode::OK).finish())
}

/// Returns a single user by its *email*
///
/// **Permissions**
/// - rauthy_admin
#[utoipa::path(
    get,
    path = "/users/email/{email}",
    tag = "users",
    responses(
        (status = 200, description = "Ok", body = UserResponse),
        (status = 401, description = "Unauthorized", body = ErrorResponse),
        (status = 403, description = "Forbidden", body = ErrorResponse),
    ),
)]
#[get("/users/email/{email}")]
#[has_roles("rauthy_admin")]
pub async fn get_user_by_email(
    data: web::Data<AppState>,
    path: web::Path<String>,
    principal: web::ReqData<Option<Principal>>,
) -> Result<HttpResponse, ErrorResponse> {
    let principal = Principal::get_from_req(principal.into_inner())?;
    principal.validate_rauthy_admin()?;

    User::find_by_email(&data, path.into_inner())
        .await
        .map(|user| HttpResponse::Ok().json(UserResponse::from(user)))
}

/// Connect to an SSE stream and wait for a MFA Request ACK from the App
///
/// **Permissions**
/// - Custom: with random code from `AwaitMfa`
///
/// **Returns**
/// - SSE Stream
#[utoipa::path(
    get,
    path = "/users/email/{email}/mfa/await/{req_id}/{code}",
    tag = "mfa",
    responses(
        (status = 200, description = "Ok"),
        (status = 401, description = "Unauthorized", body = ErrorResponse),
        (status = 403, description = "Forbidden", body = ErrorResponse),
    ),
)]
#[get("/users/email/{email}/mfa/await/{req_id}/{code}")]
#[has_permissions("all")]
pub async fn await_mfa_app(
    data: web::Data<AppState>,
    path: web::Path<(String, String, String)>,
) -> Result<sse::Sse<sse::ChannelStream>, ErrorResponse> {
    let (tx, rx) = sse::channel(1);
    let (email, req_id, code) = path.into_inner();

    let auth_code_opt = MfaAuthCode::find(&data, email).await?;
    if auth_code_opt.is_none() {
        return Err(ErrorResponse::new(
            ErrorResponseType::BadRequest,
            "No existing active MFA request".to_string(),
        ));
    }
    let auth_code = auth_code_opt.unwrap();

    if auth_code.req_id != req_id {
        return Err(ErrorResponse::new(
            ErrorResponseType::BadRequest,
            "Bad req_id".to_string(),
        ));
    }
    if auth_code.code_listen != code {
        return Err(ErrorResponse::new(
            ErrorResponseType::Unauthorized,
            "Bad credentials".to_string(),
        ));
    }

    if auth_code.exp < OffsetDateTime::now_utc().unix_timestamp() {
        return Err(ErrorResponse::new(
            ErrorResponseType::BadRequest,
            format!("The MFA Request has expired on {}", auth_code.exp),
        ));
    }

    data.caches
        .tx_ws_listen
        .send(WsListenRouteReq::SseTx {
            app_id: auth_code.app_id,
            tx: WsListenTx {
                req_id: auth_code.req_id,
                tx,
                loc: auth_code.header_loc,
            },
        })
        .await
        .map_err(|_| {
            ErrorResponse::new(
                ErrorResponseType::Internal,
                "Error caching sse::Sender channel".to_string(),
            )
        })?;

    Ok(rx)
}

/// Opens a WebSocket to listen to incoming MFA authentication requests
///
/// CAUTION: This may be removed in a future version - not clear yet.
///
/// **Permissions**
/// - Custom: Registered MFA Apps only
///
/// **Returns**
/// - active WebSocket connection listening for MFA requests
#[utoipa::path(
    get,
    path = "/users/email/{email}/mfa/listen",
    tag = "mfa",
    responses(
        (status = 200, description = "Ok"),
        (status = 401, description = "Unauthorized", body = ErrorResponse),
        (status = 403, description = "Forbidden", body = ErrorResponse),
    ),
)]
#[get("/users/email/{email}/mfa/listen")]
#[has_permissions("all")]
pub async fn listen_mfa_app(
    data: web::Data<AppState>,
    path: web::Path<String>,
    req: HttpRequest,
    stream: web::Payload,
) -> Result<HttpResponse, ErrorResponse> {
    let app_id = get_header_value(&req, APP_ID_HEADER)?
        .to_str()
        .map_err(|_| {
            ErrorResponse::new(
                ErrorResponseType::BadRequest,
                String::from("Bad app_id format"),
            )
        })?;
    let email = path.into_inner();
    debug!(
        "before getting the mfa app in ws listen - app_id: {} - email: {}",
        app_id, email,
    );
    let mfa_app = MfaApp::find(&data, app_id.to_string(), email).await?;
    debug!("got mfa app: {:?}", mfa_app);

    // start the WebSocket
    ws::start(MfaListenWs::new(data, mfa_app), &req, stream).map_err(|e| {
        let err = format!("Error with WebSocket: {:?}", e);
        error!("{}", err);
        ErrorResponse::new(ErrorResponseType::Internal, err)
    })
}

/// Creates a request to register a new MFA app
///
/// CAUTION: This may be removed in a future version - not clear yet.
///
/// **Permissions**
/// - authenticated
/// - for own email only
///
/// **Returns**
/// - active WebSocket connection listening for MFA requests
#[utoipa::path(
    get,
    path = "/users/email/{email}/mfa/new",
    tag = "deprecated",
    request_body = NewMfaAppRequest,
    responses(
        (status = 200, description = "Ok", body = MfaAppRegResponse),
        (status = 401, description = "Unauthorized", body = ErrorResponse),
        (status = 403, description = "Forbidden", body = ErrorResponse),
    ),
)]
#[post("/users/email/{email}/mfa/new")]
#[has_permissions("token-auth")]
pub async fn add_new_mfa_app(
    data: web::Data<AppState>,
    req_data: actix_web_validator::Json<NewMfaAppRequest>,
    path: web::Path<String>,
    principal: web::ReqData<Option<Principal>>,
) -> Result<HttpResponse, ErrorResponse> {
    let email = path.into_inner();

    // validate that Principal matches the path email
    let principal = Principal::get_from_req(principal.into_inner())?;
    principal.validate_email(&email, &data).await?;

    MfaAppReg::create(&data, email, req_data.into_inner())
        .await
        .map(|res| HttpResponse::Ok().json(MfaAppRegResponse::from(res)))
}

/// Starts the registration process for a new Authenticator app via WebSocket
///
/// CAUTION: This may be removed in a future version - not clear yet.
///
/// **Permissions**
/// - pre-registered app_id from `add_new_mfa_app`
///
/// **Returns**
/// - *HTTP 200 Ok* with `RegWebSocket`
///
/// Usually, I would prefer POST here, since this request modifies the backend, but React Native
/// seems to have problems with this in combination with a WebSocket
#[utoipa::path(
    get,
    path = "/users/email/{email}/mfa/register",
    tag = "deprecated",
    responses(
        (status = 200, description = "Ok"),
        (status = 401, description = "Unauthorized", body = ErrorResponse),
        (status = 403, description = "Forbidden", body = ErrorResponse),
    ),
)]
#[get("/users/email/{email}/mfa/register")]
#[has_permissions("all")]
pub async fn mfa_register_ws(
    data: web::Data<AppState>,
    path: web::Path<String>,
    req: HttpRequest,
    stream: web::Payload,
) -> Result<HttpResponse, ErrorResponse> {
    let email = path.into_inner();

    // get app_id and mfa_req
    let app_id = get_header_value(&req, APP_ID_HEADER)?
        .to_str()
        .map_err(|_| {
            ErrorResponse::new(
                ErrorResponseType::BadRequest,
                String::from("Bad app_id format"),
            )
        })?;
    info!("New MFA App Register for App ID: {}", app_id);
    let mfa_req = MfaAppReg::find(&data, app_id.to_string()).await?;

    // start the WebSocket
    ws::start(RegWebSocket::new(data, email, mfa_req), &req, stream).map_err(|e| {
        let err = format!("Error with WebSocket: {:?}", e);
        error!("{}", err);
        ErrorResponse::new(ErrorResponseType::Internal, err)
    })
}

/// Modifies a user
///
/// **Permissions**
/// - rauthy_admin
#[utoipa::path(
    put,
    path = "/users/{id}",
    tag = "users",
    request_body = UpdateUserRequest,
    responses(
        (status = 200, description = "Ok", body = UserResponse),
        (status = 401, description = "Unauthorized", body = ErrorResponse),
        (status = 403, description = "Forbidden", body = ErrorResponse),
    ),
)]
#[put("/users/{id}")]
#[has_roles("rauthy_admin")]
pub async fn put_user_by_id(
    data: web::Data<AppState>,
    id: web::Path<String>,
    req: HttpRequest,
    principal: web::ReqData<Option<Principal>>,
    session_req: web::ReqData<Option<Session>>,
    user: actix_web_validator::Json<UpdateUserRequest>,
) -> Result<HttpResponse, ErrorResponse> {
    let principal = Principal::get_from_req(principal.into_inner())?;
    principal.validate_rauthy_admin()?;
    if session_req.is_some() {
        Session::extract_validate_csrf(session_req, &req)?;
    }

    User::update(&data, id.into_inner(), user.into_inner(), None)
        .await
        .map(|user| HttpResponse::Ok().json(UserResponse::from(user)))
}

/// Allows modification of specific user values from the user himself
///
/// **Permissions**
/// - authenticated user
#[utoipa::path(
    put,
    path = "/users/{id}/self",
    tag = "users",
    request_body = UpdateUserSelfRequest,
    responses(
        (status = 200, description = "Ok", body = UserResponse),
        (status = 401, description = "Unauthorized", body = ErrorResponse),
        (status = 403, description = "Forbidden", body = ErrorResponse),
    ),
)]
#[put("/users/{id}/self")]
#[has_permissions("session-auth")] // TODO correct role? note authenticated? --> double check!
pub async fn put_user_self(
    data: web::Data<AppState>,
    id: web::Path<String>,
    req: HttpRequest,
    principal: web::ReqData<Option<Principal>>,
    session_req: web::ReqData<Option<Session>>,
    user: actix_web_validator::Json<UpdateUserSelfRequest>,
) -> Result<HttpResponse, ErrorResponse> {
    if session_req.is_some() {
        Session::extract_validate_csrf(session_req, &req)?;
    }

    // make sure the logged in user can only update itself
    let principal = Principal::get_from_req(principal.into_inner())?;
    let id = id.into_inner();
    if principal.user_id != id {
        return Err(ErrorResponse::new(
            ErrorResponseType::Forbidden,
            "You are not allowed to update another users values".to_string(),
        ));
    }

    User::update_self_req(&data, id, user.into_inner())
        .await
        .map(|user| HttpResponse::Ok().json(UserResponse::from(user)))
}

/// Deletes a user
///
/// **Permissions**
/// - rauthy_admin
#[utoipa::path(
    delete,
    path = "/users/{id}",
    tag = "users",
    responses(
        (status = 204, description = "NoContent"),
        (status = 401, description = "Unauthorized", body = ErrorResponse),
        (status = 403, description = "Forbidden", body = ErrorResponse),
    ),
)]
#[delete("/users/{id}")]
#[has_roles("rauthy_admin")]
pub async fn delete_user_by_id(
    data: web::Data<AppState>,
    path: web::Path<String>,
    req: HttpRequest,
    principal: web::ReqData<Option<Principal>>,
    session_req: web::ReqData<Option<Session>>,
) -> Result<HttpResponse, ErrorResponse> {
    let principal = Principal::get_from_req(principal.into_inner())?;
    principal.validate_rauthy_admin()?;
    if session_req.is_some() {
        Session::extract_validate_csrf(session_req, &req)?;
    }

    let user = User::find(&data, path.into_inner()).await?;
    user.delete(&data).await?;
    Ok(HttpResponse::NoContent().finish())
}
