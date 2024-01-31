use azure_storage::prelude::*;
use azure_storage_blobs::prelude::*;
use cookie::time::{Duration, OffsetDateTime};
use dotenv::dotenv;
use mongodb::bson::{doc, Document};
use mongodb::bson::oid::ObjectId;
use mongodb::Client;
use mongodb::Collection;
use rocket::fs::{FileServer, NamedFile};
use rocket::http::{ContentType, Cookie, CookieJar, Status};
use rocket::request::Request;
use rocket::response;
use rocket::response::Redirect;
use rocket::routes;
use rocket::serde::json::Json;
use rocket::serde::{Deserialize, Serialize};
use rocket::{get, post, delete, State, Rocket, Build, Response};
use serde_json::{json, to_string};
use std::io::Cursor;
use std::path::Path;
use uuid::Uuid;

struct ServerData {
    users: Collection<User>,
    movies: Collection<Movie>,
    blob_client_builder: ClientBuilder,
    container_name: String,
}

impl ServerData {
    async fn new() -> Self {
        dotenv().ok();
        let connection_string = std::env::var("COSMOS_CONNECTION_STRING")
            .expect("Set env variable COSMOS_PRIMARY_KEY first!");
        let client = Client::with_uri_str(connection_string).await.unwrap();
        let database_name = std::env::var("COSMOS_DB_NAME").unwrap();
        let database = client.database(&database_name);
        let users_coll_name = std::env::var("COSMOS_COLL_USERS_NAME").unwrap();
        let movies_coll_name = std::env::var("COSMOS_COLL_MOVIES_NAME").unwrap();
        let users_coll = database.collection::<User>(&users_coll_name);
        let movies_coll = database.collection::<Movie>(&movies_coll_name);
        let account = std::env::var("STORAGE_ACCOUNT").expect("missing STORAGE_ACCOUNT");
        let access_key = std::env::var("STORAGE_ACCESS_KEY").expect("missing STORAGE_ACCOUNT_KEY");
        let container_name = std::env::var("STORAGE_CONTAINER").expect("missing STORAGE_CONTAINER");
        let storage_credentials = StorageCredentials::access_key(account.clone(), access_key);
        let blob_client_builder = ClientBuilder::new(account, storage_credentials);
        ServerData {
            users: users_coll,
            movies: movies_coll,
            blob_client_builder,
            container_name,
        }
    }
}

#[derive(Default, Serialize, Deserialize, Debug, Clone)]
struct Movie {
    title: String,
    author: String,
    image_url: String,
    avg_rating: f32,
    num_ratings: u32,
}

#[derive(Default, Serialize, Deserialize, Debug, Clone)]
struct MovieResponse {
    id: String,
    title: String,
    author: String,
    image_url: String,
    avg_rating: f32,
    num_ratings: u32,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
struct MovieUploadRequest {
    title: String,
    author: String,
    image: Vec<u8>,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
struct User {
    name: String,
    password: String,
    uuid: String,
    movie_ratings: Vec<(ObjectId, f64)>,
    created_movies: Vec<ObjectId>
}

impl User {
    fn as_doc(&self) -> Document {
        doc! {
            "name": &self.name,
            "password": &self.password,
            "uuid": &self.uuid,
            "movie_ratings": self.movie_ratings.iter().map(|&(ref id, rating)| doc! {"movie_id": id, "rating": rating}).collect::<Vec<Document>>(),
            "created_movies": &self.created_movies
        }
    }
}

impl Into<Option<Document>> for User {
    fn into(self) -> Option<Document> {
        Some(doc! {
            "name": self.name,
            "password": self.password,
        })
    }
}

#[derive(Serialize, Deserialize, Debug, Clone)]
struct UserLogin {
    name: String,
    password: String,
}

impl Into<Option<Document>> for UserLogin {
    fn into(self) -> Option<Document> {
        Some(doc! {
            "name": self.name,
            "password": self.password,
        })
    }
}

#[derive(Serialize, Deserialize, Debug, Clone)]
struct UserLogout {
    name: String,
    uuid: String,
}


#[derive(Debug)]
struct GenericJsonResponse {
    json: String,
    status: Status,
    cookies: Vec<Cookie<'static>>,
}

impl<'r> response::Responder<'r, 'static> for GenericJsonResponse {
    fn respond_to(self, _: &'r Request<'_>) -> response::Result<'static> {
        let mut binding = Response::build();
        binding.status(self.status).header(ContentType::JSON);
        for cookie in &self.cookies {
            binding.header_adjoin(cookie);
        }
        binding
            .sized_body(self.json.len(), Cursor::new(self.json))
            .ok()
    }
}

#[get("/")]
fn index_page() -> Redirect {
    Redirect::to("/movies")
}

#[get("/movies")]
async fn movies_page() -> Option<NamedFile> {
    NamedFile::open(Path::new("static/").join("movies.html"))
        .await
        .ok()
}

#[get("/login")]
async fn login_page() -> Option<NamedFile> {
    NamedFile::open(Path::new("static/").join("login.html"))
        .await
        .ok()
}

#[get("/register")]
async fn register_page() -> Option<NamedFile> {
    NamedFile::open(Path::new("static/").join("register.html"))
        .await
        .ok()
}

#[get("/add-movie")]
async fn add_movie_page() -> Option<NamedFile> {
    NamedFile::open(Path::new("static/").join("add-movie.html"))
        .await
        .ok()
}

#[get("/delete-movie")]
async fn delete_movie_page() -> Option<NamedFile> {
    NamedFile::open(Path::new("static/").join("delete-movie.html"))
        .await
        .ok()
}

#[get("/api/movies")]
async fn get_movies(server_data: &State<ServerData>) -> GenericJsonResponse {
    let mut movies: Vec<MovieResponse> = Vec::new();
    match server_data.movies.find(None, None).await {
        Ok(mut cursor) => {
            while let Ok(true) = cursor.advance().await {
                let obj_id = cursor.current().get_object_id("_id").unwrap().to_string();
                let movie = cursor.deserialize_current().unwrap();
                let movie = MovieResponse {
                    id: obj_id,
                    title: movie.title,
                    author: movie.author,
                    image_url: movie.image_url,
                    avg_rating: movie.avg_rating,
                    num_ratings: movie.num_ratings
                };
                movies.push(movie);
            }
        }
        Err(error) => {
            println!("{:?}", error);
            let json = to_string(&json!({
                "error": "Database error"
            })).expect("Failed to serialize JSON");
            return GenericJsonResponse {
                json,
                status: Status::InternalServerError,
                cookies: Vec::new(),
            };
        }
    }

    let json = serde_json::to_string(&movies).expect("Failed to serialize to JSON");
    GenericJsonResponse {
        json,
        status: Status::Ok,
        cookies: Vec::new(),
    }
}

#[get("/api/movies/<username>")]
async fn get_movies_by_username(username: &str, cookies: &CookieJar<'_>, server_data: &State<ServerData>) -> GenericJsonResponse {
    //let username = cookies.get("username").unwrap().value();
    let uuid = cookies.get("id").unwrap().value();
    let user_doc = doc!{"name": username, "uuid": uuid};
    let user: User;
    match server_data.users.find_one(user_doc.clone(), None).await {
        Ok(u) => match u {
            Some(u) => {
                user = u;
            },
            None => {
                let json = to_string(&json!({
                    "error": "Unauthorized"
                }))
                .expect("Failed to serialize JSON");
                return GenericJsonResponse {
                    json,
                    status: Status::Unauthorized,
                    cookies: Vec::new(),
                };
            }
        },
        Err(_) => {
            let json = to_string(&json!({
                "error": "Database error"
            }))
            .expect("Failed to serialize JSON");
            return GenericJsonResponse {
                json,
                status: Status::InternalServerError,
                cookies: Vec::new(),
            };
        }
    }

    let query = doc! {
        "_id": {
            "$in": user.created_movies.iter().collect::<Vec<_>>()
        }
    };
    let mut movies: Vec<MovieResponse> = Vec::new();
    match server_data.movies.find(query, None).await {
        Ok(mut cursor) => {
            while let Ok(true) = cursor.advance().await {
                let obj_id = cursor.current().get_object_id("_id").unwrap().to_string();
                let movie = cursor.deserialize_current().unwrap();
                let movie = MovieResponse {
                    id: obj_id,
                    title: movie.title,
                    author: movie.author,
                    image_url: movie.image_url,
                    avg_rating: movie.avg_rating,
                    num_ratings: movie.num_ratings
                };
                movies.push(movie);
            }
        }
        Err(error) => {
            println!("{:?}", error);
            let json = to_string(&json!({
                "error": "Database error"
            })).expect("Failed to serialize JSON");
            return GenericJsonResponse {
                json,
                status: Status::InternalServerError,
                cookies: Vec::new(),
            };
        }
    }

    let json = serde_json::to_string(&movies).expect("Failed to serialize to JSON");
    GenericJsonResponse {
        json,
        status: Status::Ok,
        cookies: Vec::new(),
    }
}

#[post("/api/login", format = "json", data = "<user>")]
async fn login(user: Json<UserLogin>, server_data: &State<ServerData>) -> GenericJsonResponse {
    let uuid;
    match server_data.users.find_one(user.0.clone(), None).await {
        Ok(user) => match user {
            Some(user) => {
                uuid = user.uuid;
            }
            None => {
                let json = to_string(&json!({
                    "error": "Wrong username or password"
                }))
                .expect("Failed to serialize JSON");
                return GenericJsonResponse {
                    json,
                    status: Status::Unauthorized,
                    cookies: Vec::new(),
                };
            }
        },
        Err(_) => {
            let json = to_string(&json!({
                "error": "Database error"
            }))
            .expect("Failed to serialize JSON");
            return GenericJsonResponse {
                json,
                status: Status::InternalServerError,
                cookies: Vec::new(),
            };
        }
    }

    let json = to_string(&json!({
        "redirectPath": "/movies"
    }))
    .expect("Failed to serialize JSON");

    let mut expiration_time = OffsetDateTime::now_utc();
    expiration_time += Duration::weeks(52);
    let mut cookies = Vec::new();

    let mut cookie = Cookie::new("username", user.0.name);
    cookie.set_expires(expiration_time);
    cookie.set_path("/");
    cookies.push(cookie);

    let mut cookie = Cookie::new("id", uuid);
    cookie.set_expires(expiration_time);
    cookie.set_path("/");
    cookies.push(cookie);

    return GenericJsonResponse {
        json,
        status: Status::Ok,
        cookies,
    };
}

#[post("/api/users", format = "json", data = "<user>")]
async fn create_user(user: Json<UserLogin>, server_data: &State<ServerData>) -> GenericJsonResponse {
    match server_data.users.find_one(doc!{"name": user.0.clone().name}, None).await {
        Ok(user) => match user {
            Some(_) => {
                let json = to_string(&json!({
                    "error": "Username alredy exists"
                }))
                .expect("Failed to serialize JSON");
                return GenericJsonResponse {
                    json,
                    status: Status::Unauthorized,
                    cookies: Vec::new(),
                };
            }
            None => ()
        },
        Err(_) => {
            let json = to_string(&json!({
                "error": "Database error"
            }))
            .expect("Failed to serialize JSON");
            return GenericJsonResponse {
                json,
                status: Status::InternalServerError,
                cookies: Vec::new(),
            };
        }
    }

    let mut cookies = Vec::new();
    let mut expiration_time = OffsetDateTime::now_utc();
    expiration_time += Duration::weeks(52);
    // username cookie
    let mut cookie = Cookie::new("username", user.0.name.clone());
    cookie.set_expires(expiration_time);
    cookie.set_path("/");
    cookies.push(cookie);
    //id cookie
    let uuid = Uuid::new_v4();
    let mut cookie = Cookie::new("id", uuid.to_string());
    cookie.set_expires(expiration_time);
    cookie.set_path("/");
    cookies.push(cookie.clone());

    // uuid string to database
    let uuid = cookie.to_string();

    // new user
    let user = User {
        name: user.0.name,
        password: user.0.password,
        uuid,
        movie_ratings: Vec::new(),
        created_movies: Vec::new()
    };
    let result = server_data.users.insert_one(user.clone(), None).await;
    match result {
        Ok(_) => {
            let json = to_string(&json!({
                "redirectPath": "/movies"
            }))
            .expect("Failed to serialize JSON");
            return GenericJsonResponse {
                json,
                status: Status::Ok,
                cookies,
            };
        }
        Err(error) => {
            let error_message = format!("Failed to insert user: {}", error);
            let json = to_string(&json!({
                "error": error_message
            }))
            .expect("Failed to serialize JSON");
            return GenericJsonResponse {
                json,
                status: Status::InternalServerError,
                cookies: Vec::new(),
            };
        }
    }
}

#[post("/logout")]
async fn logout(cookies: &CookieJar<'_>) -> GenericJsonResponse {
    let user;
    if cookies.get("uuid").is_some() && cookies.get("username").is_some() {
        user = doc! {
            "name": cookies.get("username").unwrap().value_trimmed(),
            "uuid": cookies.get("uuid").unwrap().to_string(),
        };
        println!("User {:#?} logged out", user);
    }

    return GenericJsonResponse {
        json: "".to_string(),
        status: Status::Ok,
        cookies: Vec::new(),
    };
}

#[post("/api/add-movie", format = "json", data = "<movie>")]
async fn add_movie( movie: Json<MovieUploadRequest>, server_data: &State<ServerData>, cookies: &CookieJar<'_>,) -> GenericJsonResponse {
    let username = cookies.get("username").unwrap().value();
    let uuid = cookies.get("id").unwrap().value();
    let user_doc = doc!{"name": username, "uuid": uuid};
    let mut user: User;
    match server_data.users.find_one(user_doc.clone(), None).await {
        Ok(u) => match u {
            Some(u) => {
                user = u;
            },
            None => {
                let json = to_string(&json!({
                    "error": "Unauthorized"
                }))
                .expect("Failed to serialize JSON");
                return GenericJsonResponse {
                    json,
                    status: Status::Unauthorized,
                    cookies: Vec::new(),
                };
            }
        },
        Err(_) => {
            let json = to_string(&json!({
                "error": "Database error"
            }))
            .expect("Failed to serialize JSON");
            return GenericJsonResponse {
                json,
                status: Status::InternalServerError,
                cookies: Vec::new(),
            };
        }
    }
    
    let image_url = Uuid::new_v4();
    let image_url = format!("{}.png", image_url);
    let blob_client = server_data
        .blob_client_builder
        .clone()
        .blob_client(&server_data.container_name, image_url.clone());
    blob_client
        .put_block_blob(movie.0.image)
        .content_type("application/octet-stream")
        .await
        .unwrap();

    let movie_doc = doc! {
        "title": &movie.0.title,
        "author": &movie.0.author,
    };
    match server_data.movies.find_one(movie_doc, None).await {
        Ok(cursor) => {
            if cursor.is_some() {
                let json = to_string(&json!({
                    "error": "Movie already exists"
                }))
                .expect("Failed to serialize JSON");
                return GenericJsonResponse {
                    json,
                    status: Status::Conflict,
                    cookies: Vec::new(),
                };
            }
        }
        Err(_) => {
            let json = to_string(&json!({
                "error": "Database error"
            }))
            .expect("Failed to serialize JSON");
            return GenericJsonResponse {
                json,
                status: Status::InternalServerError,
                cookies: Vec::new(),
            };
        }
    }

    let movie = Movie {
        title: movie.0.title,
        author: movie.0.author,
        image_url,
        ..Default::default()
    };
    let result = server_data.movies.insert_one(movie, None).await;
    match result {
        Ok(movie) => {
            user.created_movies.push(movie.inserted_id.as_object_id().unwrap());
            let update = doc! {
                "$set": {
                    "created_movies": &user.created_movies
                }
            };
            let result = server_data.users.update_one(user_doc.clone(), update, None).await;
            match result {
                Ok(_) => (),
                Err(error) => {
                    let error_message = format!("Failed to insert user: {}", error);
                    println!("{}", error_message);
                }
            }
            let json = to_string(&json!({
                "message": "Movie added"
            }))
            .expect("Failed to serialize JSON");
            return GenericJsonResponse {
                json,
                status: Status::Ok,
                cookies: Vec::new(),
            };
        }
        Err(error) => {
            let error_message = format!("Failed to insert user: {}", error);
            let json = to_string(&json!({
                "error": error_message
            }))
            .expect("Failed to serialize JSON");
            return GenericJsonResponse {
                json,
                status: Status::InternalServerError,
                cookies: Vec::new(),
            };
        }
    }
}

#[delete("/api/movies")]
async fn delete_movie(cookies: &CookieJar<'_>, server_data: &State<ServerData>) -> GenericJsonResponse {
    let username = cookies.get("username").unwrap().value();
    let uuid = cookies.get("id").unwrap().value();
    let user_doc = doc!{"name": username, "uuid": uuid};
    let mut user: User;
    match server_data.users.find_one(user_doc.clone(), None).await {
        Ok(u) => match u {
            Some(u) => {
                user = u;
            },
            None => {
                let json = to_string(&json!({
                    "error": "Unauthorized"
                }))
                .expect("Failed to serialize JSON");
                return GenericJsonResponse {
                    json,
                    status: Status::Unauthorized,
                    cookies: Vec::new(),
                };
            }
        },
        Err(_) => {
            let json = to_string(&json!({
                "error": "Database error"
            }))
            .expect("Failed to serialize JSON");
            return GenericJsonResponse {
                json,
                status: Status::InternalServerError,
                cookies: Vec::new(),
            };
        }
    }

    //let query = doc! {
    //    "_id": {
    //        "$in": user.created_movies.iter().collect::<Vec<_>>()
    //    }
    //};
    let mut movies: Vec<MovieResponse> = Vec::new();
    //match server_data.movies.find(query, None).await {
    //    Ok(mut cursor) => {
    //        while let Ok(true) = cursor.advance().await {
    //            let obj_id = cursor.current().get_object_id("_id").unwrap().to_string();
    //            let movie = cursor.deserialize_current().unwrap();
    //            let movie = MovieResponse {
    //                id: obj_id,
    //                title: movie.title,
    //                author: movie.author,
    //                image_url: movie.image_url,
    //                avg_rating: movie.avg_rating,
    //                num_ratings: movie.num_ratings
    //            };
    //            movies.push(movie);
    //        }
    //    }
    //    Err(error) => {
    //        println!("{:?}", error);
    //        let json = to_string(&json!({
    //            "error": "Database error"
    //        })).expect("Failed to serialize JSON");
    //        return GenericJsonResponse {
    //            json,
    //            status: Status::InternalServerError,
    //            cookies: Vec::new(),
    //        };
    //    }
    //}

    let json = serde_json::to_string(&movies).expect("Failed to serialize to JSON");
    GenericJsonResponse {
        json,
        status: Status::Ok,
        cookies: Vec::new(),
    }
}

#[get("/api/thumbnail/<image_url>")]
async fn get_thumbnail(image_url: &str, server_data: &State<ServerData>) -> GenericJsonResponse {
    if image_url == "" {
        return GenericJsonResponse {
            json: "".to_string(),
            status: Status::NotFound,
            cookies: Vec::new(),
        };
    }
    let blob_client = server_data
        .blob_client_builder
        .clone()
        .blob_client(&server_data.container_name, image_url);
    let response = blob_client.get_content().await.unwrap();
    let json = to_string(&json!(response)).expect("Failed to serialize JSON");
    return GenericJsonResponse {
        json,
        status: Status::Ok,
        cookies: Vec::new(),
    };
}

async fn setup_rocket() -> Rocket<Build> {
    let server_data = ServerData::new().await;
    rocket::build()
        .manage(server_data)
        .mount("/", routes![
               index_page,
               movies_page,
               login_page,
               register_page,
               add_movie_page,
               delete_movie_page,
               get_movies,
               get_thumbnail,
               login,
               create_user,
               logout,
               add_movie,
               get_movies_by_username,
               delete_movie
        ])
        .mount("/", FileServer::from("static"))
}

#[rocket::main]
async fn main() {
    let _ = setup_rocket().await.launch().await;
}


#[cfg(test)]
mod tests {
    use super::setup_rocket;
    use rocket::{
        local::asynchronous::Client,
        http::{
            Status,
            ContentType
        },
        async_test
    };
    use std::{
        path::Path,
        fs::read
    };
    use serde_json::json;

    #[async_test]
    async fn test_index_page_redirect() {
        let rocket = setup_rocket().await;
        let client = Client::tracked(rocket).await.expect("valid rocket instance");

        let response = client.get("/").dispatch().await;

        assert_eq!(response.status(), Status::SeeOther);
        assert_eq!(response.headers().get_one("Location"), Some("/movies"));
    }

    #[async_test]
    async fn test_movies_page() {
        let rocket = setup_rocket().await;
        let client = Client::tracked(rocket)
            .await
            .expect("valid rocket instance");

        let response = client.get("/movies").dispatch().await;
        assert_eq!(response.status(), Status::Ok);

        let response_bytes = response.into_bytes().await.unwrap();
        let expected_bytes = read(Path::new("static/").join("movies.html")).unwrap();
        assert_eq!(response_bytes, expected_bytes);
    }

    #[async_test]
    async fn test_get_movies() {
        let rocket = setup_rocket().await;
        let client = Client::tracked(rocket)
            .await
            .expect("valid rocket instance");

        let response = client.get("/api/movies").dispatch().await;
        assert_eq!(response.status(), Status::Ok);
        println!("{:#?}", response.into_string().await);
    }

    #[async_test]
    async fn test_login_page() {
        let rocket = setup_rocket().await;
        let client = Client::tracked(rocket)
            .await
            .expect("valid rocket instance");

        let response = client.get("/login").dispatch().await;
        assert_eq!(response.status(), Status::Ok);

        let response_bytes = response.into_bytes().await.unwrap();
        let expected_bytes = read(Path::new("static/").join("login.html")).unwrap();
        assert_eq!(response_bytes, expected_bytes);
    }

    #[async_test]
    async fn test_register_page() {
        let rocket = setup_rocket().await;
        let client = Client::tracked(rocket)
            .await
            .expect("valid rocket instance");

        let response = client.get("/register").dispatch().await;
        assert_eq!(response.status(), Status::Ok);

        let response_bytes = response.into_bytes().await.unwrap();
        let expected_bytes = read(Path::new("static/").join("register.html")).unwrap();
        assert_eq!(response_bytes, expected_bytes);
    }


    #[async_test]
    async fn test_login() {
        let rocket = setup_rocket().await;
        let client = Client::tracked(rocket)
            .await
            .expect("valid rocket instance");

        let payload = json!({
            "name": "mariusz",
            "password": "f",
        }).to_string();
        let response = client
            .post("/api/login")
            .header(ContentType::JSON)
            .body(payload)
            .dispatch()
            .await;
        assert_eq!(response.status(), Status::Ok);
        let cookies = response.cookies();
        println!("Cookies: {:#?}", cookies);
    }
    // test post users, then login, then delete

    // test logout

    #[async_test]
    async fn test_add_movie_page() {
        let rocket = setup_rocket().await;
        let client = Client::tracked(rocket)
            .await
            .expect("valid rocket instance");

        let response = client.get("/add-movie").dispatch().await;
        assert_eq!(response.status(), Status::Ok);

        let response_bytes = response.into_bytes().await.unwrap();
        let expected_bytes = read(Path::new("static/").join("add-movie.html")).unwrap();
        assert_eq!(response_bytes, expected_bytes);
    }

    // test add movie
    // test get thumbnail
}
