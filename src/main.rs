#[macro_use] extern crate rocket;
#[macro_use] extern crate rocket_dyn_templates;
use rocket::serde::json::Json;
use rocket_dyn_templates::handlebars::JsonValue;
#[macro_use] extern crate serde_json;
extern crate base64;
use rocket::response::status;
use rocket::http::Status;
use rocket::fairing::AdHoc;
use rocket::Build;
use rocket::request::{Request,FromRequest,Outcome};
#[macro_use] extern crate diesel;
#[macro_use] extern crate diesel_migrations;
extern crate rocket_sync_db_pools;
use rocket_sync_db_pools::database;
mod models;
mod schema;
use models::{Rustacean,NewRustacean};
use schema::rustaceans;
use diesel::prelude::*;
extern crate serde;

embed_migrations!();

#[derive(Debug)]
pub struct BasicAuth {
    pub username : String,
    pub password : String
}

impl BasicAuth {
    fn from_authorization_header(header : &str) -> Option<Self> {
        let split = header.split_whitespace().collect::<Vec<_>>();
        if split.len() != 2 {
            return None;
        }
        if split[0]!="BASIC"{
            return None;
        }
        Self::from_base64_encoded(split[1])
    }

    fn from_base64_encoded(base64_string: &str) -> Option<BasicAuth> {
        let decoded = base64::decode(base64_string).ok()?;
        let decoded_str = String::from_utf8(decoded).ok()?;
        let split = decoded_str.split(":").collect::<Vec<_>>();
        if split.len()!=2 {
            return None;
        }
        let (username,password) = (split[0].to_string(),split[1].to_string());
        Some(BasicAuth{username,password})
    }
}


#[rocket::async_trait]
impl<'r> FromRequest<'r> for BasicAuth{
    type Error = ();
    async fn from_request(request: &'r Request<'_>) -> Outcome<Self,Self::Error> {
        let auth_header = request.headers().get_one("Authorization");
        if let Some(auth_header)=auth_header{
            if let Some(auth) = Self::from_authorization_header(auth_header) {
                return Outcome::Success(auth);
            }
        }
        Outcome::Failure((Status::Unauthorized,()))
    }
}


#[database("sqlite_path")]
struct DBConn (diesel::SqliteConnection);


#[get("/")]
fn hello() -> JsonValue {
    json!("Hello World!!\n")
}

#[get("/rustaceans")]
async fn get_rustaceans(auth:BasicAuth, conn:DBConn) -> Result<JsonValue,status::Custom<JsonValue>> {
    conn.run(|c|{
        rustaceans::table.limit(100).load::<Rustacean>(c).map(|all|json!(all)).map_err(|e| status::Custom(Status::InternalServerError,json!(e.to_string())) )
    }

        ).await
}

#[get("/rustaceans/<id>")]
async fn get_rustacean(id: i32, auth: BasicAuth, conn:DBConn) -> Result<JsonValue,status::Custom<JsonValue>> {
    conn.run(move |c|{
        rustaceans::table.find(id).get_result::<Rustacean>(c).map(|rustacean| json!(rustacean)).map_err(|e|
        status::Custom(Status::InternalServerError,json!(e.to_string())) )
    }

        ).await
}

#[post("/rustaceans",format="json", data="<new_rustacean>")]
async fn create_rustacean(_auth : BasicAuth, conn : DBConn, new_rustacean : Json<NewRustacean>) -> Result<JsonValue,status::Custom<JsonValue>>{
    conn.run(|c| {
            diesel::insert_into(rustaceans::table).values(new_rustacean.into_inner()).execute(c).map(|result| json!(result)).map_err(|e| status::Custom(Status::InternalServerError,json!(e.to_string())) )
    }

        ).await
}

#[put("/rustacean/<id>",format="json", data="<rustacean>")]
async fn edit_rustacean(id: i32, auth:BasicAuth, conn:DBConn, rustacean:Json<Rustacean> )-> Result<JsonValue,status::Custom<JsonValue>> {
  conn.run( move |c| {
    //let result = diesel::update(rustaceans::table.find(id)).set(rustacean.into_inner()).execute(c).expect("Cannot update rustacen"); // This needs to update created at field also, so crashes
    diesel::update(rustaceans::table.find(id)).set((
        rustaceans::name.eq(rustacean.name.to_owned()),
        rustaceans::email.eq(rustacean.email.to_owned())

    )).execute(c).map(|result| json!(result)).map_err(|e| status::Custom(Status::InternalServerError,json!(e.to_string())) )
  }
    ).await
}

#[delete("/rustacean/<id>")]
async fn del_rustacean(id: i32, conn:DBConn) -> Result<status::NoContent,status::Custom<JsonValue>> {
    conn.run( move |c| { 
        diesel::delete(rustaceans::table.find(id)).execute(c).map(|_any| status::NoContent).map_err(|e| status::Custom(Status::InternalServerError,json!(e.to_string())) )
    } ) .await
}

#[catch(404)]
fn catchnf() -> JsonValue {
    json!("Not Found!")
}

#[catch(401)]
fn catchua() -> JsonValue {
    json!("Unauthorized!")
}

#[catch(422)]
fn catchue() -> JsonValue {
    json!("Unprocessable entity")
}

async fn run_db_migrations(r:rocket::Rocket<Build>) -> rocket::Rocket<Build> {
       DBConn::get_one(&r).await.expect("Cannot run migrations!!!").run(
        |c| match embedded_migrations::run(c) {
            Ok(()) => r,
            Err(e) => {
                panic!("Cannot run migrations {:?}", e.to_string() );
            }

        }
        ).await
       
}

#[rocket::main]
async fn main() {
    let _ = rocket::build()
    .mount("/",routes![hello, get_rustaceans, get_rustacean, create_rustacean, edit_rustacean, del_rustacean ])
    .register("/",catchers![catchnf,catchua,catchue])
    .attach(DBConn::fairing())
    .attach(AdHoc::on_ignite("Database migrations",run_db_migrations))
    .launch()
    .await
    ;
}
