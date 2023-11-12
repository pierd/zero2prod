use actix_web::{get, http::header::ContentType, post, web, HttpResponse};
use actix_web_flash_messages::{FlashMessage, IncomingFlashMessages};
use anyhow::Context;
use sqlx::{Executor, PgPool, Postgres, Transaction};
use std::fmt::Write;
use uuid::Uuid;

use crate::{
    idempotency::{save_response, try_processing, IdempotencyKey, NextAction},
    routes::admin::reject_anonymous_user,
    session_state::TypedSession,
    utils::{e400, e500, see_other},
};

#[get("/admin/newsletters")]
pub async fn publish_newsletter_form(
    flash_messages: IncomingFlashMessages,
    session: TypedSession,
) -> Result<HttpResponse, actix_web::Error> {
    reject_anonymous_user(session).await?;
    let mut msg_html = String::new();
    for m in flash_messages.iter() {
        writeln!(msg_html, "<p><i>{}</i></p>", m.content()).unwrap();
    }
    let idempotency_key = uuid::Uuid::new_v4();
    Ok(HttpResponse::Ok()
        .content_type(ContentType::html())
        .body(format!(
            r#"<!DOCTYPE html>
<html lang="en">
<head>
    <meta http-equiv="content-type" content="text/html; charset=utf-8">
    <title>Publish Newsletter Issue</title>
</head>
<body>
    {msg_html}
    <form action="/admin/newsletters" method="post">
        <label>Title:<br>
            <input
                type="text"
                placeholder="Enter the issue title"
                name="title"
            >
        </label>
        <br>
        <label>Plain text content:<br>
            <textarea
                placeholder="Enter the content in plain text"
                name="text_content"
                rows="20"
                cols="50"
            ></textarea>
        </label>
        <br>
        <label>HTML content:<br>
            <textarea
                placeholder="Enter the content in HTML format"
                name="html_content"
                rows="20"
                cols="50"
            ></textarea>
        </label>
        <br>
        <input hidden type="text" name="idempotency_key" value="{idempotency_key}">
        <button type="submit">Publish</button>
    </form>
    <p><a href="/admin/dashboard">&lt;- Back</a></p>
</body>
</html>"#
        )))
}

#[derive(serde::Deserialize)]
pub struct FormData {
    title: String,
    text_content: String,
    html_content: String,
    idempotency_key: String,
}

fn success_message() -> FlashMessage {
    FlashMessage::info("The newsletter issue has been published!")
}

#[tracing::instrument(name = "Publish a newsletter issue", skip_all)]
#[post("/admin/newsletters")]
pub async fn publish_newsletter(
    form: web::Form<FormData>,
    session: TypedSession,
    pool: web::Data<PgPool>,
) -> Result<HttpResponse, actix_web::Error> {
    let user_id = reject_anonymous_user(session).await?;
    // panic!("boom");
    let FormData {
        title,
        text_content,
        html_content,
        idempotency_key,
    } = form.0;
    let idempotency_key: IdempotencyKey = idempotency_key.try_into().map_err(e400)?;
    let mut transaction = match try_processing(&pool, &idempotency_key, user_id)
        .await
        .map_err(e500)?
    {
        NextAction::StartProcessing(t) => t,
        NextAction::ReturnSavedResponse(saved_response) => {
            success_message().send();
            return Ok(saved_response);
        }
    };
    let issue_id = insert_newsletter_issue(&mut transaction, &title, &text_content, &html_content)
        .await
        .context("Failed to store newsletter issue details")
        .map_err(e500)?;
    enqueue_delivery_tasks(&mut transaction, issue_id)
        .await
        .context("Failed to enqueue delivery tasks")
        .map_err(e500)?;
    let response = see_other("/admin/newsletters");
    let response = save_response(transaction, &idempotency_key, user_id, response)
        .await
        .map_err(e500)?;
    success_message().send();
    Ok(response)
}

#[tracing::instrument(skip_all)]
async fn insert_newsletter_issue(
    transaction: &mut Transaction<'_, Postgres>,
    title: &str,
    text_content: &str,
    html_content: &str,
) -> Result<Uuid, sqlx::Error> {
    let newsletter_issue_id = Uuid::new_v4();
    let query = sqlx::query!(
        r#"
        INSERT INTO newsletter_issues (
            newsletter_issue_id, 
            title, 
            text_content, 
            html_content,
            published_at
        )
        VALUES ($1, $2, $3, $4, now())
        "#,
        newsletter_issue_id,
        title,
        text_content,
        html_content
    );
    transaction.execute(query).await?;
    Ok(newsletter_issue_id)
}

#[tracing::instrument(skip_all)]
async fn enqueue_delivery_tasks(
    transaction: &mut Transaction<'_, Postgres>,
    newsletter_issue_id: Uuid,
) -> Result<(), sqlx::Error> {
    let query = sqlx::query!(
        r#"
        INSERT INTO issue_delivery_queue (
            newsletter_issue_id, 
            subscriber_email
        )
        SELECT $1, email
        FROM subscriptions
        WHERE status = 'confirmed'
        "#,
        newsletter_issue_id,
    );
    transaction.execute(query).await?;
    Ok(())
}
