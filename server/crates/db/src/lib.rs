pub mod account;
pub mod character;
pub mod error;
pub mod map;
pub mod monster;
pub mod npc;
pub mod portal;
pub mod realm;

pub use error::Error;

/// The Postgres of `docker compose up -d`, when `DATABASE_URL` is not set.
pub const DEFAULT_DATABASE_URL: &str = "postgres://gravak:gravak@localhost:5432/gravak";

/// The database the servers use, for the logs: `DATABASE_URL` without its
/// password.
pub fn database_label() -> String {
    let url = std::env::var("DATABASE_URL").unwrap_or_else(|_| DEFAULT_DATABASE_URL.to_owned());
    match (url.split_once("://"), url.rfind('@')) {
        (Some((scheme, rest)), Some(at)) => {
            let credentials = &rest[..at - scheme.len() - 3];
            let user = credentials.split(':').next().unwrap_or_default();
            format!("{scheme}://{user}@{}", &url[at + 1..])
        },
        _ => url,
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn database_label_hides_the_password() {
        std::env::set_var("DATABASE_URL", "postgres://gravak:secret@db:5432/gravak");
        assert_eq!(super::database_label(), "postgres://gravak@db:5432/gravak");
    }
}

#[cfg(feature = "testing")]
pub mod testing;
