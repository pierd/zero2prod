use actix_web::{post, HttpResponse, web};

#[derive(serde::Deserialize)]
pub(crate) struct FormData {
    email: String,
    name: String
}

#[post("/subscriptions")]
pub(crate) async fn subscribe(_form: web::Form<FormData>) -> HttpResponse {
    HttpResponse::Ok().finish()
}
