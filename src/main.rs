use postgres::{Client, NoTls};
use std::net::{TcpListener, TcpStream};
use std::io::{Read, Write};
use std::env;
use serde_json::Value;

#[macro_use]
extern crate serde_derive;

// Modelo: estructura IceCream con id, flavor, size, price y quantity
#[derive(Serialize, Deserialize)]
struct IceCream {
    id: Option<i32>,
    flavor: String,
    size: String,
    price: f64,    // Precio del helado
    quantity: i32, // Cantidad de helados
}

// Constantes
const OK_RESPONSE: &str = "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\n\r\n";
const NOT_FOUND: &str = "HTTP/1.1 404 NOT FOUND\r\n\r\n";
const INTERNAL_SERVER_ERROR: &str = "HTTP/1.1 500 INTERNAL SERVER ERROR\r\n\r\n";

// Función principal
fn main() {
    // Obtener la cadena de conexión a la base de datos desde la variable de entorno
    let db_url = env::var("DATABASE_URL").unwrap_or("postgres://postgres:postgres@db:5432/postgres".to_string());

    // Configurar la base de datos
    if let Err(e) = set_database(&db_url) {
        println!("Error: {}", e);
        return;
    }

    // Iniciar el servidor e imprimir el puerto
    let listener = TcpListener::bind("0.0.0.0:8080").expect("No se pudo enlazar al puerto 8080");
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
                r if r.starts_with("POST /icecreams") => handle_post_request(r, db_url),
                r if r.starts_with("GET /icecreams/") => handle_get_request(r, db_url),
                r if r.starts_with("GET /icecreams") => handle_get_all_request(r, db_url),
                r if r.starts_with("PUT /icecreams/") => handle_put_request(r, db_url),
                r if r.starts_with("DELETE /icecreams/") => handle_delete_request(r, db_url),
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
    match (get_icecream_request_body(&request), Client::connect(db_url, NoTls)) {
        (Ok(mut icecream), Ok(mut client)) => {
            // Obtener price y quantity del cuerpo de la solicitud
            let body = get_request_body(&request);
            if let Some(price) = body.get("price").and_then(|p| p.as_f64()) {
                icecream.price = price;
            }
            if let Some(quantity) = body.get("quantity").and_then(|q| q.as_i64()) {
                icecream.quantity = quantity as i32;
            }

            client
                .execute(
                    "INSERT INTO icecreams (flavor, size, price, quantity) VALUES ($1, $2, $3, $4)",
                    &[&icecream.flavor, &icecream.size, &icecream.price, &icecream.quantity]
                )
                .unwrap();

            (OK_RESPONSE.to_string(), "Helado creado".to_string())
        }
        _ => (INTERNAL_SERVER_ERROR.to_string(), "Error".to_string()),
    }
}

// Función para manejar la solicitud GET
fn handle_get_request(request: &str, db_url: &str) -> (String, String) {
    match (get_id(&request).parse::<i32>(), Client::connect(db_url, NoTls)) {
        (Ok(id), Ok(mut client)) =>
            match client.query_one("SELECT * FROM icecreams WHERE id = $1", &[&id]) {
                Ok(row) => {
                    let icecream = IceCream {
                        id: row.get(0),
                        flavor: row.get(1),
                        size: row.get(2),
                        price: row.get(3),
                        quantity: row.get(4),
                    };

                    (OK_RESPONSE.to_string(), serde_json::to_string(&icecream).unwrap())
                }
                _ => (NOT_FOUND.to_string(), "Helado no encontrado".to_string()),
            }

        _ => (INTERNAL_SERVER_ERROR.to_string(), "Error".to_string()),
    }
}

// Función para manejar la solicitud GET de todos los helados
fn handle_get_all_request(_request: &str, db_url: &str) -> (String, String) {
    match Client::connect(db_url, NoTls) {
        Ok(mut client) => {
            let mut icecreams = Vec::new();

            for row in client.query("SELECT * FROM icecreams", &[]).unwrap() {
                icecreams.push(IceCream {
                    id: row.get(0),
                    flavor: row.get(1),
                    size: row.get(2),
                    price: row.get(3),
                    quantity: row.get(4),
                });
            }

            (OK_RESPONSE.to_string(), serde_json::to_string(&icecreams).unwrap())
        }
        _ => (INTERNAL_SERVER_ERROR.to_string(), "Error".to_string()),
    }
}

// Función para manejar la solicitud PUT
fn handle_put_request(request: &str, db_url: &str) -> (String, String) {
    match
        (
            get_id(&request).parse::<i32>(),
            get_icecream_request_body(&request),
            Client::connect(db_url, NoTls),
        )
    {
        (Ok(id), Ok(mut icecream), Ok(mut client)) => {
            // Obtener price y quantity del cuerpo de la solicitud
            let body = get_request_body(&request);
            if let Some(price) = body.get("price").and_then(|p| p.as_f64()) {
                icecream.price = price;
            }
            if let Some(quantity) = body.get("quantity").and_then(|q| q.as_i64()) {
                icecream.quantity = quantity as i32;
            }

            client
                .execute(
                    "UPDATE icecreams SET flavor = $1, size = $2, price = $3, quantity = $4 WHERE id = $5",
                    &[&icecream.flavor, &icecream.size, &icecream.price, &icecream.quantity, &id]
                )
                .unwrap();

            (OK_RESPONSE.to_string(), "Helado actualizado".to_string())
        }
        _ => (INTERNAL_SERVER_ERROR.to_string(), "Error".to_string()),
    }
}

// Función para manejar la solicitud DELETE
fn handle_delete_request(request: &str, db_url: &str) -> (String, String) {
    match (get_id(&request).parse::<i32>(), Client::connect(db_url, NoTls)) {
        (Ok(id), Ok(mut client)) => {
            let rows_affected = client.execute("DELETE FROM icecreams WHERE id = $1", &[&id]).unwrap();

            if rows_affected == 0 {
                return (NOT_FOUND.to_string(), "Helado no encontrado".to_string());
            }

            (OK_RESPONSE.to_string(), "Helado eliminado".to_string())
        }
        _ => (INTERNAL_SERVER_ERROR.to_string(), "Error".to_string()),
    }
}

// Función para configurar la base de datos
fn set_database(db_url: &str) -> Result<(), postgres::Error> {
    // Conectar a la base de datos
    let mut client = Client::connect(db_url, NoTls)?;

    // Crear la tabla si no existe
    client.batch_execute(
        "CREATE TABLE IF NOT EXISTS icecreams (
            id SERIAL PRIMARY KEY,
            flavor VARCHAR NOT NULL,
            size VARCHAR NOT NULL,
            price DOUBLE PRECISION,
            quantity INTEGER
        )"
    )?;
    Ok(())
}

// Función para obtener el ID de la solicitud
fn get_id(request: &str) -> &str {
    request.split("/").nth(2).unwrap_or_default().split_whitespace().next().unwrap_or_default()
}

// Función para deserializar un helado a partir del cuerpo de la solicitud
fn get_icecream_request_body(request: &str) -> Result<IceCream, serde_json::Error> {
    serde_json::from_str(request.split("\r\n\r\n").last().unwrap_or_default())
}

// Función para deserializar el cuerpo de la solicitud JSON
fn get_request_body(request: &str) -> Value {
    let body = request.split("\r\n\r\n").last().unwrap_or_default();
    serde_json::from_str(body).unwrap_or_default()
}
