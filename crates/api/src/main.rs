use api::{AppState, build_router};

#[tokio::main]
async fn main() {
    let state = AppState::new_in_memory();
    let app = build_router(state);

    let listener = tokio::net::TcpListener::bind("0.0.0.0:3000")
        .await
        .expect("gagal bind ke port 3000");

    println!("API berjalan di http://0.0.0.0:3000 (repository: in-memory)");

    axum::serve(listener, app)
        .await
        .expect("server berhenti tidak terduga");
}
