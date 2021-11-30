use actix_web::{get, web, App, HttpResponse, HttpRequest, HttpServer};
use mongodb::{bson::doc, options::ClientOptions, Client, Collection, bson::Document};
use actix_session::{Session, CookieSession};
use serde::{Deserialize, Serialize};
use tera::{Tera, Context};
use rand::{distributions::Alphanumeric, Rng};
use dotenv;





struct AppState {
    coll: Collection<Document>,
    tera: Tera
}

#[derive(Serialize, Deserialize, std::fmt::Debug)]
pub struct UrlFormParams {
    url: String,
    url_id: String
}

fn get_set_session(session: &Session, key: &str, alternate: &str) -> String {
    if let Some(value) = session.get::<String>(key).expect("Couldn't access session") {
        session.insert(key, alternate).expect("Couldn't access session");
        value
    } else {
        session.insert(key, alternate).expect("Couldn't access session");
        String::from(alternate)
    }
}





#[get("/{url_id}")]
async fn redirect(req: HttpRequest, data: web::Data<AppState>) -> HttpResponse {
    let url_id: String = req.match_info().query("url_id").parse().unwrap();
    let response = match data.coll.find_one(doc! {"url_id": url_id}, None).await.expect("Failed to connect to the database") {
        Some(document) => {
            let url = document.get_str("url").expect("Document doesn't contain url");
            HttpResponse::MovedPermanently()
                .append_header(("Location", url))
                .body("")
        },
        None => {
            let html = data.tera.render("404.html", &Context::default()).expect("Failed to render template 404.html");
            HttpResponse::NotFound()
                .body(html)
        }
    };
    response
}

async fn index_get(session: Session, data: web::Data<AppState>) -> HttpResponse {
    let success = get_set_session(&session, "success", "0");
    let url = get_set_session(&session, "url", "");
    let url_id = get_set_session(&session, "url_id", "");
    let mut context = Context::new();
    context.insert("success", &success);
    context.insert("url", &url);
    context.insert("url_id", &url_id);
    let html = data.tera.render("index.html", &context).expect("Failed to render template index.html");
    HttpResponse::Ok()
        .content_type("text/html")
        .body(html)
}

async fn index_post(req: HttpRequest, session: Session, data: web::Data<AppState>, params: web::Form<UrlFormParams>) -> HttpResponse {
    let mut url = String::from(&params.url);
    let mut url_id = String::from(&params.url_id);
    let response = match data.coll.find_one(doc! {"url_id": &url_id}, None).await.expect("Failed to connect to the database") {
        Some(_) => {
            session.insert("success", "-1").expect("Couldn't access session");
            session.insert("url", &url).expect("Couldn't access session");
            HttpResponse::SeeOther()
                .append_header(("Location", req.url_for_static("index_get").expect("Failed to construct url").as_str()))
                .body("")
        }
        None => {
            if url_id == "" {
                url_id = rand::thread_rng()
                    .sample_iter(&Alphanumeric)
                    .take(6)
                    .map(char::from)
                    .collect();
            }
            if &url[..4] != "http" {
                url = format!("http://{}", url);
            }
            data.coll.insert_one(doc! {"url_id": &url_id, "url": &url}, None).await.expect("Failed to connect to the database");
            session.insert("success", "1").expect("Couldn't access session");
            session.insert("url_id", &url_id).expect("Couldn't access session");
            HttpResponse::SeeOther()
                .append_header(("Location", req.url_for_static("index_get").expect("Failed to construct url").as_str()))
                .body("")
        }
    };
    response
}





#[actix_web::main]
async fn main() -> std::io::Result<()> {
    
    dotenv::dotenv().expect("Failed to load .env");
    
    let mongo_uri = dotenv::var("MONGO_URI").expect("Failed to load MONGO_URI");
    let client_options = ClientOptions::parse(mongo_uri).await.expect("Failed to create MongoDB Options");
    let client = Client::with_options(client_options).expect("Failed to create MongoDB Client");
    let coll = client.database("shortener-data").collection("link_map");

    let mut tera = Tera::default();
    tera.add_raw_template("index.html", include_str!("../templates/index.html")).expect("Failed to compile index.html");
    tera.add_raw_template("404.html", include_str!("../templates/404.html")).expect("Failed to compile 404.html");

    let address = dotenv::var("ADDRESS").expect("Failed to load ADDRESS");
    let hostname = dotenv::var("HOSTNAME").expect("Failed to load HOSTNAME")

    println!("Starting Actix App on {}", address);

    HttpServer::new(move || {
        App::new()
            .wrap(CookieSession::signed(&[0; 32]).secure(false))
            .app_data(web::Data::new(AppState {
                coll: coll.clone(),
                tera: tera.clone()
            }))
            .service(redirect)
            .service(
                web::resource("/")
                    .name("index_get")
                    .route(web::get().to(index_get))
                    .route(web::post().to(index_post))
            )
    })
    .server_hostname(hostname)
    .bind(address)?
    .run()
    .await
}