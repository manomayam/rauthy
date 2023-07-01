use actix_web::cookie::SameSite;
use actix_web::{cookie, web, HttpRequest};
use rauthy_common::constants::{PWD_CSRF_HEADER, PWD_RESET_COOKIE};
use rauthy_common::error_response::{ErrorResponse, ErrorResponseType};
use rauthy_common::utils::get_rand;
use rauthy_models::app_state::AppState;
use rauthy_models::entity::colors::ColorEntity;
use rauthy_models::entity::magic_links::MagicLinkPassword;
use rauthy_models::entity::password::PasswordPolicy;
use rauthy_models::entity::users::User;
use rauthy_models::entity::webauthn::WebauthnServiceReq;
use rauthy_models::request::PasswordResetRequest;
use rauthy_models::templates::PwdResetHtml;
use time::OffsetDateTime;
use tracing::debug;

pub async fn handle_get_pwd_reset<'a>(
    data: &web::Data<AppState>,
    req: HttpRequest,
    user_id: String,
    reset_id: String,
) -> Result<(String, String, cookie::Cookie<'a>), ErrorResponse> {
    let mut ml = MagicLinkPassword::find(data, &reset_id).await?;
    ml.validate(&user_id, &req)?;

    // check if the user has MFA enabled
    let user = User::find(data, ml.user_id.clone()).await?;
    let email = if user.has_webauthn_enabled() {
        Some(&user.email)
    } else {
        None
    };

    // get the html and insert values
    let rules = PasswordPolicy::find(data).await?;
    let colors = ColorEntity::find_rauthy(data).await?;
    let (html, nonce) = PwdResetHtml::build(&ml.csrf_token, &rules, email, &colors);

    // generate a cookie value and save it to the magic link
    let cookie_val = get_rand(48);
    ml.cookie = Some(cookie_val);
    ml.save(data).await?;

    let age_secs = ml.exp - OffsetDateTime::now_utc().unix_timestamp();
    let max_age = cookie::time::Duration::seconds(age_secs);
    // let exp = cookie::Expiration::from(cookie::time::OffsetDateTime::from(
    //     SystemTime::now().add(std::time::Duration::from_secs(ml.exp.timestamp() as u64)),
    // ));
    let cookie = cookie::Cookie::build(PWD_RESET_COOKIE, ml.cookie.unwrap())
        .secure(true)
        .http_only(true)
        .same_site(SameSite::Lax)
        .max_age(max_age)
        // .expires(exp)
        .path("/auth")
        .finish();

    Ok((html, nonce, cookie))
}

#[tracing::instrument(level = "debug", skip_all, fields(email = req_data.email))]
pub async fn handle_put_user_password_reset<'a>(
    data: &web::Data<AppState>,
    req: HttpRequest,
    user_id: String,
    req_data: PasswordResetRequest,
) -> Result<cookie::Cookie<'a>, ErrorResponse> {
    // validate user_id / given email address
    debug!("getting user");
    let mut user = User::find(data, user_id).await?;
    if user.email != req_data.email {
        return Err(ErrorResponse::new(
            ErrorResponseType::BadRequest,
            String::from("E-Mail does not match for this user"),
        ));
    }

    // check MFA code
    if user.has_webauthn_enabled() {
        match req_data.mfa_code {
            None => {
                // TODO delete the whole ML too?
                return Err(ErrorResponse::new(
                    ErrorResponseType::BadRequest,
                    "MFA code is missing".to_string(),
                ));
            }
            Some(code) => {
                let svc_req = WebauthnServiceReq::find(data, code).await?;
                if svc_req.user_id != user.id {
                    // TODO delete the whole ML too?
                    return Err(ErrorResponse::new(
                        ErrorResponseType::Forbidden,
                        "User ID does not match".to_string(),
                    ));
                }

                svc_req.delete(data).await?;
            }
        }
    }

    debug!("getting magic link");
    let mut ml = MagicLinkPassword::find(data, &req_data.magic_link_id).await?;
    ml.validate(&user.id, &req)?;

    // validate csrf token
    match req.headers().get(PWD_CSRF_HEADER) {
        None => {
            return Err(ErrorResponse::new(
                ErrorResponseType::Unauthorized,
                String::from("CSRF Token is missing"),
            ));
        }
        Some(token) => {
            if ml.csrf_token != token.to_str().unwrap_or("") {
                return Err(ErrorResponse::new(
                    ErrorResponseType::Unauthorized,
                    String::from("Invalid CSRF Token"),
                ));
            }
        }
    }

    debug!("applying password rules");
    // validate password
    user.apply_password_rules(data, &req_data.password).await?;

    debug!("invalidating magic link pwd");
    // all good
    ml.invalidate(data).await?;
    user.email_verified = true;
    user.save(data, None, None).await?;

    // delete the cookie
    let cookie = cookie::Cookie::build(PWD_RESET_COOKIE, "")
        .secure(true)
        .http_only(true)
        .same_site(SameSite::Lax)
        .max_age(cookie::time::Duration::ZERO)
        .path("/auth")
        .finish();
    Ok(cookie)
}
