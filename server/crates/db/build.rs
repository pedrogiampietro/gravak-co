// `sqlx::migrate!` embeds the migrations when this crate compiles; a new or
// changed file under `migrations/` must compile it again.
fn main() {
    println!("cargo:rerun-if-changed=../../migrations");
}
