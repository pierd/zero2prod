use actix_web::{get, HttpResponse, Responder};

#[get("/health_check")]
pub(crate) async fn health_check() -> impl Responder {
    HttpResponse::Ok()
}
