FROM rust:latest

WORKDIR /usr/src/cinema-score

COPY ./Cargo.toml ./Rocket.toml ./script.sh ./
COPY ./src ./src
COPY ./static ./static

RUN cargo install --path .

EXPOSE 8000

ENTRYPOINT ["./script.sh"]
