use actix_web::http::header::ContentType;
use actix_web::{get, post, web, HttpResponse};
use actix_web_flash_messages::{FlashMessage, IncomingFlashMessages};
use secrecy::{ExposeSecret, Secret};
use std::fmt::Write;

use crate::authentication::{validate_credentials, AuthError, Credentials};
use crate::routes::admin::reject_anonymous_user;
use crate::routes::get_username;
use crate::session_state::TypedSession;
use crate::utils::{e500, see_other};

#[get("/admin/password")]
pub async fn change_password_form(
    session: TypedSession,
    flash_messages: IncomingFlashMessages,
) -> Result<HttpResponse, actix_web::Error> {
    if session.get_user_id().map_err(e500)?.is_none() {
        return Ok(see_other("/login"));
    }

    let mut msg_html = String::new();
    for m in flash_messages.iter() {
        writeln!(msg_html, "<p><i>{}</i></p>", m.content()).unwrap();
    }

    Ok(HttpResponse::Ok()
        .content_type(ContentType::html())
        .body(format!(
            r#"<!DOCTYPE html>
<html lang="en">
<head>
    <meta http-equiv="content-type" content="text/html; charset=utf-8">
    <title>Change Password</title>
</head>
<body>
    {msg_html}
    <form action="/admin/password" method="post">
        <label>Current password
            <input
                type="password"
                placeholder="Enter current password"
                name="current_password"
            >
        </label>
        <br>
        <label>New password
            <input
                type="password"
                placeholder="Enter new password"
                name="new_password"
            >
        </label>
        <br>
        <label>Confirm new password
            <input
                type="password"
                placeholder="Type the new password again"
                name="new_password_check"
            >
        </label>
        <br>
        <button type="submit">Change password</button>
</form>
    <p><a href="/admin/dashboard">&lt;- Back</a></p>
</body>
</html>"#
        )))
}

#[derive(serde::Deserialize)]
pub struct FormData {
    current_password: Secret<String>,
    new_password: Secret<String>,
    new_password_check: Secret<String>,
}

#[post("/admin/password")]
pub async fn change_password(
    web::Form(form): web::Form<FormData>,
    session: TypedSession,
    pool: web::Data<sqlx::PgPool>,
) -> Result<HttpResponse, actix_web::Error> {
    let user_id = reject_anonymous_user(session).await?;

    if form.new_password.expose_secret() != form.new_password_check.expose_secret() {
        FlashMessage::error(
            "You entered two different new passwords - the field values must match.",
        )
        .send();
        return Ok(see_other("/admin/password"));
    }

    let username = get_username(user_id, &pool).await.map_err(e500)?;
    let credentials = Credentials {
        username,
        password: form.current_password,
    };
    if let Err(e) = validate_credentials(credentials, &pool).await {
        return match e {
            AuthError::InvalidCredentials(_) => {
                FlashMessage::error("The current password is incorrect.").send();
                Ok(see_other("/admin/password"))
            }
            AuthError::UnexpectedError(_) => Err(e500(e)),
        };
    }

    crate::authentication::change_password(user_id, form.new_password, &pool)
        .await
        .map_err(e500)?;
    FlashMessage::error("Your password has been changed.").send();
    Ok(see_other("/admin/password"))
}
