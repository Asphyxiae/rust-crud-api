use postgres::{Client, NoTls};
use postgres::Error as PostgresError;
use std::net::{TcpListener, TcpStream};
use std::io::{Read, Write};
use std::env;
use serde_json::Value;

#[macro_use]
extern crate serde_derive;

// Modelo: estructura User con id, name, email, edad y telefono
#[derive(Serialize, Deserialize)]
struct User {
    id: Option<i32>,
    name: String,
    email: String,
    edad: i32,          
    telefono: String,    
}

// Constantes
const OK_RESPONSE: &str = "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\n\r\n";
const NOT_FOUND: &str = "HTTP/1.1 404 NOT FOUND\r\n\r\n";
const INTERNAL_SERVER_ERROR: &str = "HTTP/1.1 500 INTERNAL SERVER ERROR\r\n\r\n";

// Función principal
fn main() {
    // Obtener la URL de la base de datos en tiempo de ejecución
    let db_url = match env::var("DATABASE_URL") {
        Ok(url) => url,
        Err(_) => {
            println!("DATABASE_URL no está configurada, se utiliza la configuración predeterminada");
            "postgres://postgres:postgres@db:5432/postgres".to_string()
        }
    };

    // Configurar la base de datos
    if let Err(e) = set_database(&db_url) {
        println!("Error: {}", e);
        return;
    }

    // Iniciar el servidor e imprimir el puerto
    let listener = TcpListener::bind("0.0.0.0:8080").unwrap();
    println!("Servidor iniciado en el puerto 8080");

    // Manejar las solicitudes de clientes
    for stream in listener.incoming() {
        match stream {
            Ok(stream) => {
                handle_client(stream, &db_url);
            }
            Err(e) => {
                println!("Error: {}", e);
            }
        }
    }
}

// Función para manejar al cliente
fn handle_client(mut stream: TcpStream, db_url: &str) {
    let mut buffer = [0; 1024];
    let mut request = String::new();

    match stream.read(&mut buffer) {
        Ok(size) => {
            request.push_str(String::from_utf8_lossy(&buffer[..size]).as_ref());

            let (status_line, content) = match &*request {
                r if r.starts_with("POST /users") => handle_post_request(r, db_url),
                r if r.starts_with("GET /users/") => handle_get_request(r, db_url),
                r if r.starts_with("GET /users") => handle_get_all_request(r, db_url),
                r if r.starts_with("PUT /users/") => handle_put_request(r, db_url),
                r if r.starts_with("DELETE /users/") => handle_delete_request(r, db_url),
                _ => (NOT_FOUND.to_string(), "404 Not Found".to_string()),
            };

            stream.write_all(format!("{}{}", status_line, content).as_bytes()).unwrap();
        }
        Err(e) => {
            println!("Error: {}", e);
        }
    }
}

// Controladores

// Función para manejar la solicitud POST
fn handle_post_request(request: &str, db_url: &str) -> (String, String) {
    match (get_user_request_body(&request), Client::connect(db_url, NoTls)) {
        (Ok(mut user), Ok(mut client)) => {
            // Obtener edad y telefono del cuerpo de la solicitud
            let body = get_request_body(&request);
            if let Some(edad) = body.get("edad").and_then(|e| e.as_i64()) {
                user.edad = edad as i32;
            }
            if let Some(telefono) = body.get("telefono").and_then(|t| t.as_str()) {
                user.telefono = telefono.to_string();
            }

            client
                .execute(
                    "INSERT INTO users (name, email, edad, telefono) VALUES ($1, $2, $3, $4)",
                    &[&user.name, &user.email, &user.edad, &user.telefono]
                )
                .unwrap();

            (OK_RESPONSE.to_string(), "Usuario creado".to_string())
        }
        _ => (INTERNAL_SERVER_ERROR.to_string(), "Error".to_string()),
    }
}

// Función para manejar la solicitud GET
fn handle_get_request(request: &str, db_url: &str) -> (String, String) {
    match (get_id(&request).parse::<i32>(), Client::connect(db_url, NoTls)) {
        (Ok(id), Ok(mut client)) =>
            match client.query_one("SELECT * FROM users WHERE id = $1", &[&id]) {
                Ok(row) => {
                    let user = User {
                        id: row.get(0),
                        name: row.get(1),
                        email: row.get(2),
                        edad: row.get(3),
                        telefono: row.get(4),
                    };

                    (OK_RESPONSE.to_string(), serde_json::to_string(&user).unwrap())
                }
                _ => (NOT_FOUND.to_string(), "Usuario no encontrado".to_string()),
            }

        _ => (INTERNAL_SERVER_ERROR.to_string(), "Error".to_string()),
    }
}

// Función para manejar la solicitud GET de todos los usuarios
fn handle_get_all_request(_request: &str, db_url: &str) -> (String, String) {
    match Client::connect(db_url, NoTls) {
        Ok(mut client) => {
            let mut users = Vec::new();

            for row in client.query("SELECT * FROM users", &[]).unwrap() {
                users.push(User {
                    id: row.get(0),
                    name: row.get(1),
                    email: row.get(2),
                    edad: row.get(3),
                    telefono: row.get(4),
                });
            }

            (OK_RESPONSE.to_string(), serde_json::to_string(&users).unwrap())
        }
        _ => (INTERNAL_SERVER_ERROR.to_string(), "Error".to_string()),
    }
}

// Función para manejar la solicitud PUT
fn handle_put_request(request: &str, db_url: &str) -> (String, String) {
    match
        (
            get_id(&request).parse::<i32>(),
            get_user_request_body(&request),
            Client::connect(db_url, NoTls),
        )
    {
        (Ok(id), Ok(mut user), Ok(mut client)) => {
            // Obtener edad y telefono del cuerpo de la solicitud
            let body = get_request_body(&request);
            if let Some(edad) = body.get("edad").and_then(|e| e.as_i64()) {
                user.edad = edad as i32;
            }
            if let Some(telefono) = body.get("telefono").and_then(|t| t.as_str()) {
                user.telefono = telefono.to_string();
            }

            client
                .execute(
                    "UPDATE users SET name = $1, email = $2, edad = $3, telefono = $4 WHERE id = $5",
                    &[&user.name, &user.email, &user.edad, &user.telefono, &id]
                )
                .unwrap();

            (OK_RESPONSE.to_string(), "Usuario actualizado".to_string())
        }
        _ => (INTERNAL_SERVER_ERROR.to_string(), "Error".to_string()),
    }
}

// Función para manejar la solicitud DELETE
fn handle_delete_request(request: &str, db_url: &str) -> (String, String) {
    match (get_id(&request).parse::<i32>(), Client::connect(db_url, NoTls)) {
        (Ok(id), Ok(mut client)) => {
            let rows_affected = client.execute("DELETE FROM users WHERE id = $1", &[&id]).unwrap();

            if rows_affected == 0 {
                return (NOT_FOUND.to_string(), "Usuario no encontrado".to_string());
            }

            (OK_RESPONSE.to_string(), "Usuario eliminado".to_string())
        }
        _ => (INTERNAL_SERVER_ERROR.to_string(), "Error".to_string()),
    }
}

// Función para configurar la base de datos
fn set_database(db_url: &str) -> Result<(), PostgresError> {
    // Conectar a la base de datos
    let mut client = Client::connect(db_url, NoTls)?;

    // Crear la tabla si no existe
    client.batch_execute(
        "CREATE TABLE IF NOT EXISTS users (
            id SERIAL PRIMARY KEY,
            name VARCHAR NOT NULL,
            email VARCHAR NOT NULL,
            edad INTEGER,
            telefono VARCHAR
        )"
    )?;
    Ok(())
}

// Función para obtener el ID de la solicitud
fn get_id(request: &str) -> &str {
    request.split("/").nth(2).unwrap_or_default().split_whitespace().next().unwrap_or_default()
}

// Función para deserializar un usuario a partir del cuerpo de la solicitud
fn get_user_request_body(request: &str) -> Result<User, serde_json::Error> {
    serde_json::from_str(request.split("\r\n\r\n").last().unwrap_or_default())
}

// Función para deserializar el cuerpo de la solicitud JSON
fn get_request_body(request: &str) -> Value {
    let body = request.split("\r\n\r\n").last().unwrap_or_default();
    serde_json::from_str(body).unwrap_or_default()
}
