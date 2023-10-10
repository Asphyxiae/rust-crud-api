# Stage de compilación
FROM rust:1.69-buster as builder

# Establece el directorio de trabajo dentro del contenedor
WORKDIR /app

# Acepta el argumento de construcción DATABASE_URL
ARG DATABASE_URL

# Define la variable de entorno DATABASE_URL
ENV DATABASE_URL=$DATABASE_URL

# Copia todo el contenido del directorio actual (donde se encuentra el Dockerfile) al directorio de trabajo en el contenedor
COPY . .

# Ejecuta la compilación de la aplicación Rust en modo de lanzamiento
RUN cargo build --release

# Etapa de producción
FROM debian:buster-slim

# Establece el directorio de trabajo dentro del contenedor
WORKDIR /usr/local/bin

# Copia el binario de la aplicación compilada desde la etapa de compilación al directorio de trabajo en la etapa de producción
COPY --from=builder /app/target/release/rust-crud-api .

# Define el comando que se ejecutará cuando se inicie el contenedor
CMD ["./rust-crud-api"]
