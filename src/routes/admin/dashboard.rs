use actix_web::{get, web, HttpResponse};

use crate::{
    routes::{admin::reject_anonymous_user, get_username},
    session_state::TypedSession,
    utils::e500,
};

#[get("/admin/dashboard")]
pub async fn admin_dashboard(
    session: TypedSession,
    pool: web::Data<sqlx::PgPool>,
) -> Result<HttpResponse, actix_web::Error> {
    let user_id = reject_anonymous_user(session).await?;
    let username = get_username(user_id, &pool).await.map_err(e500)?;
    Ok(HttpResponse::Ok()
        .content_type("text/html; charset=utf-8")
        .body(format!(
            r#"<!DOCTYPE html>
            <html lang="en">
            <head>
                <meta http-equiv="content-type" content="text/html; charset=utf-8">
                <title>Admin dashboard</title>
            </head>
            <body>
                <p>Welcome {username}</p>
                <p>Available actions:</p>
                <ol>
                    <li><a href="/admin/password">Change password</a></li>
                    <li>
                        <form name="logoutForm" action="/admin/logout" method="post">
                            <input type="submit" value="Logout">
                        </form>
                    </li>
                </ol>
            </body>
            </html>"#,
        )))
}
