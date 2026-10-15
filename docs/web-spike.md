# Gravak no navegador

**Estado (15/10/2026):** o cliente Rust (`client/crates/sandbox`) roda no
navegador em https://gravak.fun/pt/play, para contas logadas no site. Ele faz
login nos servidores reais, entra no mundo e se joga: andar, pular
(Ctrl+clique), atacar, reviver, inventário, lojas, Sell All e Helper, com som e
com as configurações salvas na conta. O build nativo (`cargo build -p
conquer-sandbox`) continua o mesmo: tudo o que é do navegador fica atrás de
`cfg(target_arch = "wasm32")`.

![Tela de login no navegador](web-spike-login.jpg)

Começou como um spike de viabilidade (só a tela de login, sem rede). Este
documento diz como ficou e o que ainda falta.

## Como funciona

```
navegador (gravak.fun/pt/play, iframe de /play/index.html)
  ├─ gravak.js + gravak_bg.wasm ........... public/play do site (Next.js)
  ├─ wss://gravak.fun/ws/9958, /ws/5816 ... nginx → gravak-ws-bridge :9960 → auth / jogo (TCP)
  ├─ POST /game-files ..................... nginx → gravak-web-files :9961 → gravak.gpk decifrado
  └─ /api/settings ........................ site (Next.js) → tabela account_settings
```

### Rede: `crates/ws-bridge`

O navegador não abre TCP. `gravak-ws-bridge` aceita um WebSocket em `/<porta>`
e repassa os bytes, sem ler nada, para a porta TCP do servidor (só 9958 e 5816).
Ele conecta no servidor antes de aceitar o WebSocket, então um socket aberto
quer dizer servidor no ar (o status do mundo na tela de login usa isso). A
cifra TQ continua no cliente: `gravak-protocol` roda sobre `transport::Stream`,
TCP no nativo e WebSocket no navegador. Os servidores veem todo jogador do
navegador como `127.0.0.1`; a ponte registra o IP real (`X-Forwarded-For`).

### Arquivos do jogo: `crates/web-files` e `web_files.rs`

`gravak-web-files` abre o `gravak.gpk` com a chave de release (lida de um
arquivo, nunca no wasm) e entrega as entradas decifradas, em lotes, só para quem
tem o cookie de sessão do site (`gravak_session`, assinado com o
`SESSION_SECRET`). Um `POST /game-files` leva os caminhos, um por linha. A
resposta traz, para cada caminho na mesma ordem, um u32 e os bytes, ou
`MISSING` (o pack não tem) ou `LATER` (resposta cheia, pedir de novo). O
cabeçalho `X-Gravak-Pack` (tamanho e data do pack) é a versão do pack. O
`gravak-publish-update` reinicia o serviço quando uma release traz pack novo.

No cliente, todo `Assets` do navegador lê um só armazém (`web_files.rs`). A
leitura continua síncrona: um arquivo que ainda não chegou é pedido e a leitura
falha como pendente (`Downloading game file: ...`). Quem leu tenta de novo
depois:

- **Entrar no mundo:** `web.rs` monta o mundo (`build_world`, a metade de
  `run()` depois do login) de novo e de novo até nada do que ele lê estar a
  caminho, e então troca o canvas da tela de login pelo do mundo. Um canvas
  pintado em 2D (softbuffer) não pode ir para a GPU.
- **Loaders** (texturas, interface, geometria, modelos): `background.rs`. No
  nativo é a mesma thread de antes. No navegador os trabalhos rodam no loop da
  página, até 6 ms por callback, e um trabalho que esbarrou num arquivo a
  caminho roda de novo quando os arquivos chegam. A janela de tempo começa no
  primeiro trabalho de cada callback (o desenho, onde as texturas são lidas,
  vem um frame depois do `about_to_wait`).
- **O desenho:** um frame que esbarrou num arquivo a caminho é pulado, não é
  erro. `Renderer::upload` não guarda uma textura feita enquanto faltava um
  arquivo (seria o quadrado com o número do item, o texto no lugar do botão
  Revive); o frame seguinte a faz de novo. Loja (`ShopState::reload_icons`) e
  Sell All (`SellAllView::reload`) refazem ícones e preços montados na
  abertura.

Tudo o que chega fica no **Cache Storage** do navegador, num cache com a versão
do pack no nome: a segunda visita quase não pede nada, e um pack novo começa um
cache novo e apaga o antigo.

**Pré-carregamento.** Descobrir um arquivo por vez custava ~115 idas e voltas
na primeira visita. Duas listas, gravadas de sessões reais, vão num pedido cada:

- `client/web/world.txt`: o que entrar em qualquer mundo lê (interface, HUD,
  ícones, efeitos, `ini/`, `ani/`, das quatro profissões) e o Birth Village,
  onde todo personagem novo nasce. ~2 MB com gzip, pedidos enquanto a tela de
  login está aberta.
- `client/web/body1.txt` .. `body4.txt`: os modelos de cada corpo
  (`c3/000N`, `network::body_from_mesh`), ~3,4 MB, pedidos quando o servidor
  diz o corpo do herói que entra.

Primeira visita de um personagem novo: 3 pedidos, o mundo aberto ~2 s depois de
"Enter the World". Para gravar as listas de novo: subir um `gravak-web-files`
com `GRAVAK_LOG_PATHS=1` (ele imprime `PATH <caminho>` de cada pedido), jogar
uma sessão de cada profissão, e tirar dos caminhos o que é de um mapa
(`map/`, `data/map/`, `gravak/maps/`, `data/minimap/`, `c3/monster/`), menos o
Birth Village (`newbie`, `sky`).

### Desenho

- wgpu com WebGPU onde há adaptador, senão WebGL2
  (`new_instance_with_webgpu_detection`), pedindo ao WebGL2 os limites dele
  (`downlevel_webgl2_defaults`).
- O canvas tem o tamanho que a página dá (`client/web/index.html`); o jogo não
  restaura nem impõe tamanho de janela no navegador.
- A fonte embutida (KaTeX SansSerif, SIL OFL) responde por toda família
  genérica, inclusive monospace (a fonte do HUD).
- Um clique no canvas dá a ele o foco do teclado (e com ele Ctrl, Alt e Shift);
  o canvas do mundo recebe o foco ao abrir.

### Som: `audio_web.rs`

A mesma API do `audio.rs`, pela Web Audio da página: o navegador decodifica os
MP3 e WAV (`decodeAudioData`), sem decodificador no wasm e sem thread. Tema da
tela de login, cliques, música da região em loop com fade de 0,8 s, passos e
golpes, nos volumes da janela Options. O `AudioContext` só toca depois do
primeiro clique ou tecla, como os navegadores exigem.

### Configurações: `saved.rs` e `/api/settings`

Opções, HUD, atalhos, Helper, listas do Sell All e escolhas da tela de login
passam por `saved.rs`. No nativo são os mesmos `settings/*.ini` ao lado do
executável. No navegador cada arquivo é uma chave do `localStorage`
(`gravak/settings/<nome>`) e vai para a conta no site: a página baixa os
arquivos da conta ao abrir (os do site ganham), envia os que só ela tinha, e
manda um arquivo 2 s depois da última mudança ou quando a página fecha
(`keepalive`). O site guarda em `account_settings` (migration 106), até 200
arquivos de 64 KB por conta. Senha nunca: o `vault` só sela no Windows.

### Site

`/pt/play` (`web/src/app/[lang]/play`) pede o login do site, porque os
arquivos do jogo só vão para uma sessão, e mostra o jogo sobre a janela inteira,
com uma barra fina para voltar ao site e pôr em tela cheia. O link "Jogar no
navegador" está no menu.

## Publicar na VPS

Pré-requisitos, uma vez: `rustup target add wasm32-unknown-unknown` e
`cargo install wasm-bindgen-cli --version 0.2.128 --locked` (a versão do
`wasm-bindgen` do `Cargo.lock`). No Windows, `client\tools\build-web.ps1` faz o
mesmo que os passos abaixo.

```bash
cd client
CARGO_TARGET_DIR=target/web cargo build --release -p conquer-sandbox --target wasm32-unknown-unknown
wasm-bindgen --target web --no-typescript --out-name gravak --out-dir dist/web \
  target/web/wasm32-unknown-unknown/release/conquer-sandbox.wasm
cp web/index.html dist/web/
cp /var/www/gravak-update/servers.ini dist/web/          # os mundos e suas chaves de login
mkdir -p dist/web/assets && cp web/world.txt web/body?.txt dist/web/assets/
# Os arquivos da tela de login (web/assets.txt), tirados do pack pelo cliente
# nativo compilado com a chave de release (o da Armory serve):
/opt/gravak/armory/conquer-sandbox --export-assets /var/www/gravak-update/game \
  dist/web/assets data/main/GeneralBG.dds
printf 'data/main/GeneralBG.dds\n' > dist/web/assets/manifest.txt
rsync -a --delete dist/web/ /srv/gravak/web/public/play/
chown -R gravak:gravak /srv/gravak/web/public/play
systemctl restart gravak-web     # o Next só serve de public/ o que existia ao subir
```

Serviços: `gravak-ws-bridge` (127.0.0.1:9960) e `gravak-web-files`
(127.0.0.1:9961, `EnvironmentFile` = o `.env.production.local` do site, para o
`SESSION_SECRET`; chave em `/srv/gravak/keys/pack.key`). No nginx de
`gravak.fun`: `location /ws/` (WebSocket para a ponte) e
`location = /game-files` (com gzip). Para testar sem o site:
`http://localhost:8099/?bridge=ws://127.0.0.1:9960`.

## Testar sem um navegador de verdade

O login do site tem Turnstile, então um navegador automatizado não passa por
ele. Na VPS:

- um `gravak-web-files` local com um `SESSION_SECRET` de teste, um servidor
  estático para `client/dist/web` que repassa `/game-files` para ele, e um
  cookie assinado com esse segredo;
- Chromium do Playwright em `index.html?bridge=ws://127.0.0.1:9960`, com
  WebGL2 por software (SwiftShader, 7–15 fps). O headless shell não tem
  WebGPU; o Chromium completo tem (SwiftShader), mas a captura de tela sai
  branca;
- a cada 5 s o console diz o que do mundo está na página
  (`Gravak world: N actors; models: ...`), e os erros de carregamento de modelo
  e de som vão para o console (`report!`; o `eprintln!` não chega lá).

O Playwright fecha o navegador num SIGTERM mas o processo continua vivo: use
`kill -9`.

## Medidas

| | |
|---|---|
| wasm (release, sem `wasm-opt`) | 9,3 MB, 2,9 MB com gzip |
| primeira visita, personagem novo | 3 pedidos de arquivos, mundo ~2 s após "Enter the World" |
| segunda visita | quase tudo do Cache Storage |
| tela de login | ~55–60 fps, 7–9 ms de CPU por frame (pintada na CPU) |

## O que falta

1. **Outros mapas na primeira visita.** Fora do Birth Village os arquivos de
   um mapa chegam aos poucos, e modelos e texturas aparecem com atraso na
   primeira vez. Listas por mapa (como `bodyN.txt`) resolveriam.
2. **Threads.** Os loaders rodam no loop da página; decodificar DDS e C3 custa
   frames enquanto um mapa carrega. Web Workers (`wasm-bindgen-rayon`) pedem
   `+atomics`, nightly e os cabeçalhos COOP/COEP no site.
3. **Tela de login.** Sem os heróis da criação de personagem e sem as vistas do
   mundo ao fundo (`showcase`, `login_scenes` usam threads). Ela pinta tudo na
   CPU todo frame: em 1920×1080 cai abaixo de 60 fps.
4. **Plataforma.** Clipboard (evento `paste`), cursores próprios não testados,
   botão "Exit" (no navegador não fecha nada), fonte sem os glyphs de acentos
   estendidos (trocar por uma Noto/Liberation Sans OFL), toque no celular.
5. **Tamanho.** `wasm-opt -Oz` e `opt-level = "z"` devem tirar 10–20%.
6. **Configurações do cliente do PC** continuam nos arquivos dele: ele não tem a
   sessão do site para falar com `/api/settings`.

## Notas do ambiente

- No worktree (`.claude\worktrees\…`) o `cl.exe` do MSVC falha com D8050 ao
  compilar o C do `ring` quando o `target` fica dentro do worktree. Com
  `CARGO_TARGET_DIR` fora dele compila normalmente. O `build-web.ps1` aceita
  `GRAVAK_WEB_TARGET` para isso. No checkout normal não acontece.
- Para testar digitação por automação, mande teclas (`keydown`) e não
  `insertText`: o canvas do winit só escuta eventos de tecla.
