use gravak_protocol::{AuthResponse, authenticate, connect_game};

#[tokio::main]
async fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let address = args.first().map(String::as_str).unwrap_or("127.0.0.1:9958");
    let username = args.get(1).map(String::as_str).unwrap_or("test1");
    let password = args.get(2).map(String::as_str).unwrap_or("123456");
    let realm = args.get(3).map(String::as_str).unwrap_or("CoEmu");

    println!("Connecting to auth {address} as {username} on realm {realm}...");
    match authenticate(address, username, password, realm).await {
        Ok(AuthResponse::Forwarded(server)) => {
            println!(
                "Auth OK: token={}, game_server={}:{}",
                server.token, server.address, server.port
            );
            let game_address = format!("{}:{}", server.address, server.port);
            match connect_game(&game_address, server.token, 5065, "English").await {
                Ok(talk) => println!(
                    "Game OK: channel={}, sender={}, message={}",
                    talk.channel, talk.sender_name, talk.message
                ),
                Err(error) => {
                    eprintln!("Game connect failed: {error}");
                    std::process::exit(1);
                }
            }
        }
        Ok(AuthResponse::Rejected(rejection)) => {
            println!(
                "Auth rejected: code={}, {}",
                rejection.code, rejection.message
            )
        }
        Err(error) => {
            eprintln!("Auth probe failed: {error}");
            std::process::exit(1);
        }
    }
}
