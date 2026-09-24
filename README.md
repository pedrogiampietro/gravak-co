# gravak-co

Monorepo para o desenvolvimento do cliente e do servidor de um projeto inspirado em Conquer Online.

## Estrutura

- `client/` — cliente Preservation CO 5065, escrito em Rust.
- `server/` — servidor Gravak baseado no projeto [CoEmu](https://github.com/shekohex/coemu).

O cliente e o servidor ainda não são totalmente compatíveis: o cliente trabalha com assets/protocolo 5065, enquanto a base atual do servidor CoEmu é voltada ao protocolo 5017. A integração de protocolo será uma etapa de desenvolvimento.

## Assets

Os assets do cliente não fazem parte deste repositório. No ambiente local atual eles ficam em `R:\5065`.

Para validar os assets e executar o cliente:

```powershell
cd R:\gravak-co\client
cargo run --release --locked -- --assets R:\5065 --check-assets
cargo run --release --locked -- --assets R:\5065
```

Também é possível usar `client\tools\run-local.ps1`.

Para abrir o cliente conectado ao servidor local:

```powershell
cd R:\gravak-co\client
cargo run --release --locked -p conquer-sandbox -- --assets R:\5065 --online --auth 127.0.0.1:9960 --username test1 --password 123456 --realm CoEmu
```

O modo online autentica em uma thread de rede separada, carrega o personagem e usa o mapa/posição retornados pelo servidor. Neste ambiente, o auth do monorepo usa `9960` para coexistir com o auth legado em `9958`; em uma instalação limpa, use a porta configurada no `.env`. Sem `--online`, o sandbox permanece offline.

## Servidor

O servidor exige Rust nightly; a versão está fixada em `server\rust-toolchain.toml`. Para preparar o banco local:

```powershell
cd R:\gravak-co\server
sqlx migrate run --source migrations --database-url "sqlite://data/coemu.db?mode=rwc"
cargo build -p auth --features server -p game
```

Os servidores usam, por padrão, as portas `9958` (auth) e `5816` (game). O banco SQLite, logs, builds e fixtures locais são ignorados pelo Git.

## Estado atual

O cliente foi validado contra os assets locais 5065. A base do servidor também foi corrigida e validada com os testes do workspace e build dos binários de auth/game. O `gravak-probe` já autentica, cria um personagem de teste quando necessário, reconecta, carrega `MsgUserInfo`/`MsgMapInfo` e confirma o primeiro `MsgWalk`. O sandbox gráfico já possui o fluxo online inicial, envia `MsgWalk` a cada passo concluído, reconcilia correções do servidor, acompanha movimento/saída de atores remotos e desenha jogadores com seus nomes, corpos, cabelos e equipamentos básicos; NPCs usam os modelos e texturas próprios dos catálogos 5065.

## Licenças e créditos

As licenças originais permanecem nos diretórios dos componentes:

- cliente: `client/LICENSE` (Apache-2.0);
- servidor: `server/LICENSE` (GPL-3.0-only).

Os créditos e avisos dos projetos de origem devem ser preservados. Nenhum asset proprietário do jogo é distribuído neste repositório.
