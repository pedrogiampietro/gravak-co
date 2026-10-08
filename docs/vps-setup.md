# Setup da VPS — Gravak

Roteiro para o Claude (ou quem for) montar o servidor do Gravak numa VPS
recém-comprada. Escrito em 2026-10-08, a partir do branch `launch-security`.
Leia tudo antes de começar; marque `[x]` no que for concluído.

## Contexto

- **Servidores:** dois binários Rust em `server/`:
  - `auth-server`, o login, na porta **9958** (`AUTH_PORT`);
  - `game-server`, o jogo, na porta **5816** (vem da tabela `realms`).

  Os dois escutam em `0.0.0.0`.
- **Banco:** Postgres 17 (`DATABASE_URL`). As migrações estão em `server/migrations`.
- **Cliente:** só Windows. É empacotado **na máquina Windows do Pedro**
  (`client/tools/package-client.ps1`), não na VPS, porque precisa do 5065 em
  `R:\5065` e das chaves do cliente.
- **Updates:** são arquivos estáticos (`dist\update`, feito por
  `client/tools/publish-update.ps1`) servidos por HTTP(S). A VPS pode servir
  com nginx.
- **Como roda hoje, no Windows:**
  - `run-server.bat`: auth em **debug**, game em release; sobe o Postgres com
    `server/docker-compose.yml`;
  - `build-server.ps1`: o build;
  - `build-content.ps1`: gera `content/build/client`.

  Nada disso serve para produção como está: veja abaixo.

## O que já está pronto (branch `launch-security`)

- [x] **Login selado.** A senha vai selada para a chave X25519 do auth
  (packet 9010). O RC5 do 5065 só é aceito da própria máquina.
- [x] **Bans e GMs:** `$ban <nome> <horas|0> [motivo]`, `$unban`, `$gm <nome> on|off`.
- [x] **`server/scripts/prepare-launch.ps1`:** faz backup, deixa GM só para a
  equipe e trava as contas de dev (`gravak`, `test1`, senha 123456).
- [x] **Cliente empacotado** num `.gpk` criptografado (`installer\pack.key`).
- [x] **Updates assinados** com ed25519. O cliente de release recusa manifesto
  sem `manifest.sig` válido (`client\keys\update.key`).

## Antes de começar (no PC do Pedro)

- [ ] **Publicar o branch.** O `launch-security` **ainda não foi enviado** ao
  GitHub (`origin` = github.com/pedrogiampietro/gravak-co). Fazer push, ou merge
  em `main` e push.
- [ ] **Preparar o que não está no git e precisa ir para a VPS** (por `scp`,
  `rsync` ou zip):
  - `server/data/GameMaps` (~334 MB; mapas, ignorados pelo git);
  - `server/data/Maps`, `NPCs.csv` e o que mais houver em `server/data` que
    não esteja versionado;
  - `R:\5065\ini\Shop.dat` e `R:\5065\ini\itemtype.dat` (no `.env`:
    `SHOP_DATA` e `ITEMTYPE_DATA`);
  - `content/build/client` inteiro, gerado por `build-content.ps1` no Windows.
    O servidor usa `GRAVAK_CONTENT=<pasta>` e
    `ITEMTYPE_DATA=<pasta>/ini/itemtype.dat` (veja `run-server.bat`).
- [ ] **Backup do banco atual,** se for levar contas e personagens de teste
  (`server/scripts/db-backup.ps1`). Senão, a VPS começa com o banco vazio.

## Na VPS

### 1. Sistema

- [ ] Atualizar o sistema e criar um usuário sem root para rodar o jogo (ex.: `gravak`).
- [ ] **Firewall** (ufw ou o firewall do provedor). Abrir somente:
  - 22 (SSH; de preferência só por chave, sem senha);
  - 9958/tcp (auth) e 5816/tcp (game);
  - 80 e 443 se a VPS também servir os updates.

  **Nunca abrir a 5432 (Postgres).**
- [ ] Fuso horário: o relógio do sistema pode ficar em UTC. As guerras de
  guilda usam `GUILD_WAR_UTC_OFFSET`, que por padrão já é -3 (Brasília).

### 2. Build

- [ ] Instalar o Rust (o `server/rust-toolchain.toml` fixa a versão), além de
  `build-essential`, `pkg-config` e `libssl-dev` se pedir. Existe um
  `server/flake.nix` como alternativa.
- [ ] Clonar o repositório e fazer o checkout do branch publicado.
- [ ] Compilar **tudo em release** (no Windows o auth sai em debug; aqui não):
  ```bash
  cd server
  cargo build --release --locked -p auth --bin auth-server --features server
  cargo build --release --locked -p game --bin game-server
  ```
  **O servidor nunca foi compilado em Linux.** Se algo falhar (caminhos com
  `\`, crates só de Windows, etc.), corrija no código, rode os testes e faça
  commit.
- [ ] Rodar os testes do servidor (`cargo test --release`) e confirmar que passam.

### 3. Postgres

- [ ] **Não usar o `server/docker-compose.yml` como está.** Ele é de dev: senha
  `gravak/gravak` e porta 5432 publicada em todas as interfaces. Opções:
  - criar um `docker-compose.prod.yml` com senha forte (num `.env` fora do git),
    `ports: ["127.0.0.1:5432:5432"]` ou sem porta nenhuma, e `restart: unless-stopped`;
  - ou Postgres 17 nativo, escutando só em localhost.
- [ ] Ajustar o `DATABASE_URL` com a senha nova.
- [ ] As migrações são embutidas no `game-server` (`sqlx::migrate!` em
  `server/server/game/src/state/mod.rs`) e aplicadas quando ele sobe. Subir o
  game uma vez antes de mexer na tabela `realms`.
- [ ] Se for levar o banco do PC, restaurar o backup (ver
  `server/scripts/db-restore.ps1` para o formato).

### 4. Configuração (`server/.env`, a partir de `server/.env.example`)

- [ ] `DATABASE_URL`: a senha forte do passo 3.
- [ ] `DATA_LOCATION`: o caminho absoluto de `server/data` na VPS.
- [ ] `SHOP_DATA` e `ITEMTYPE_DATA`: os arquivos copiados. **Atenção:** o `.env`
  do PC tem caminhos `R:\\5065\\...`.
- [ ] `GRAVAK_CONTENT`: a pasta `content/build/client` copiada (e
  `ITEMTYPE_DATA` apontando para o `ini/itemtype.dat` dela, como no
  `run-server.bat`).
- [ ] `TRANSFER_SECRET`: um valor aleatório **novo**, longo, igual para auth e
  game (`openssl rand -base64 32`). Não reaproveitar o do PC.
- [ ] `LOG_VERBOSITY`: 2 está bom. Para os logs, usar o journald (passo 5).
- [ ] O `.env` com `chmod 600`, dono = usuário do jogo.

### 5. Endereço do realm (CRÍTICO)

O auth manda o jogador conectar em `realms.game_ip_address:game_port`. A
migração `7_default_realms.sql` grava **`127.0.0.1`**. Sem trocar, o login
funciona e o jogo tenta conectar no PC do próprio jogador.

- [ ] Depois que as migrações rodarem:
  ```sql
  UPDATE realms SET game_ip_address = '<IP público ou domínio>' WHERE name = 'Gravak';
  ```
  O auth também usa esse endereço para falar com o game. Testar se o auth na
  própria VPS consegue conectar no IP público (alguns provedores não fazem
  hairpin NAT). Se não conseguir, o código precisa separar "endereço para o
  cliente" de "endereço interno" (ver `server/server/auth/src/linker.rs` e
  `server/packets/transfer/src/lib.rs`).
- [ ] O `game-server` mostra no log ao subir: `Realm ... (clients are sent to IP:PORT)`.
  Conferir.

### 6. Serviços (systemd)

- [ ] Criar `gravak-auth.service` e `gravak-game.service`:
  - `User=gravak`;
  - `WorkingDirectory=<repo>/server` (o auth procura `keys/login.key` relativo
    a ela, ou em `GRAVAK_LOGIN_KEY`);
  - `EnvironmentFile=<repo>/server/.env`;
  - `Restart=on-failure`;
  - o game com `After=` no Postgres (e no auth).
- [ ] **Parada limpa (IMPORTANTE).** Em `server/crates/server/src/lib.rs`, fora
  do Windows o servidor só espera `ctrl_c()`, ou seja, **SIGINT**. O systemd
  manda **SIGTERM** por padrão, e então o processo morre **sem salvar** quem
  está online. Corrigir de um destes jeitos:
  - (a) colocar `KillSignal=SIGINT` nas duas units;
  - (b) melhor: no ramo `#[cfg(not(windows))]`, esperar também
    `tokio::signal::unix::signal(SignalKind::terminate())`.

  Fazer as duas coisas é o ideal. Usar `TimeoutStopSec=60`. Testar: logar um
  personagem, mudar algo, `systemctl restart gravak-game` e conferir que salvou.
- [ ] `systemctl enable --now` nos dois e conferir os logs (`journalctl -u gravak-game -f`).

### 7. Chave de login (antes de empacotar o cliente!)

- [ ] Na **primeira** subida, o auth cria `server/keys/login.key` (secreta) e
  `login.pub`, e mostra a pública no log.
- [ ] Copiar o **`login.pub`** para o PC do Pedro, em `server\keys\login.pub`
  (ou passar no `-LoginKey`). O `package-client.ps1` grava a chave como `Key=`
  no `servers.ini` dos jogadores. Cliente empacotado com outra chave não loga.
- [ ] Guardar uma cópia do `login.key` **fora** da VPS (gerenciador de senhas
  ou pendrive). Se ela se perder, todos os clientes precisam ser refeitos.

### 8. Contas

- [ ] Criar as contas da equipe e depois rodar o equivalente do
  `server/scripts/prepare-launch.ps1`. Ele é PowerShell + `docker exec`; no
  Linux, rodar o mesmo SQL com `psql` ou portar o script:
  ```sql
  UPDATE accounts SET gm = (lower(username) IN ('pedro', ...));
  UPDATE accounts SET banned_until = 'infinity', ban_reason = 'development account'
   WHERE username IN ('gravak','test1') AND lower(username) NOT IN ('pedro', ...);
  ```
  Fazer backup antes.

### 9. Updates (nginx)

- [ ] Instalar o nginx e servir uma pasta (ex.: `/var/www/gravak-update`) em
  `https://<domínio>/update/`. Usar HTTPS com Let's Encrypt (certbot) se houver
  domínio.
- [ ] Essa URL é o `-Update` do `package-client.ps1`.
- [ ] **A `client\keys\update.key` nunca vai para a VPS.** A assinatura é feita
  no PC do Pedro (`publish-update.ps1`); a VPS só recebe `dist\update`. Enviar
  `manifest.sig` e `manifest.txt` **por último**.

### 10. Backup

- [ ] Cron diário com `pg_dump` (o `db-backup.ps1` guarda os 14 mais recentes;
  fazer o mesmo), mais uma cópia fora da VPS (outro servidor, S3/R2 ou snapshot
  do provedor).
- [ ] Testar a restauração uma vez.

## De volta no PC do Pedro: empacotar e testar

- [ ] `installer\servers.ini` (ou `-Auth`), apontando para `<IP ou domínio>:9958`
  com `Key=<login.pub>`.
- [ ] Empacotar:
  ```powershell
  .\client\tools\package-client.ps1 -Auth <ip>:9958 -Update https://<domínio>/update
  ```
  Na primeira vez são criadas `client\keys\update.key` e `installer\pack.key`.
  **Fazer backup das duas.**
- [ ] `.\client\tools\publish-update.ps1` e subir `dist\update` para a pasta do nginx.
- [ ] **Teste de ponta a ponta, num PC que não seja o do servidor:** instalar o
  `GravakSetup.exe`, criar conta, logar, entrar no mapa, andar, relogar. Depois
  publicar um update qualquer e ver o cliente baixar. Depois adulterar um byte
  do `manifest.txt` no servidor e ver o cliente recusar.
- [ ] Rodar o `prepare-launch` (passo 8) antes de abrir para os jogadores.

## Segredos: onde ficam e quem precisa

| Arquivo | Onde vive | Fazer backup | Se perder |
|---|---|---|---|
| `server/keys/login.key` | só na VPS | sim, fora da VPS | refazer e redistribuir todos os clientes |
| `client/keys/update.key` | só no PC do Pedro | sim | clientes nunca mais atualizam |
| `client/installer/pack.key` | só no PC do Pedro | sim | novos packs não abrem nos clientes antigos |
| `TRANSFER_SECRET`, senha do Postgres | `.env` da VPS | sim | trocar e reiniciar |

Nenhum deles vai para o git (todos estão no `.gitignore`).
