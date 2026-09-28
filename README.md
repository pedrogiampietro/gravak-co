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

Para compilar o cliente Release, execute `build-client.bat` na raiz do projeto
com duplo clique. No PowerShell, use
`powershell -ExecutionPolicy Bypass -File .\build-client.ps1`. O executável será
gerado em `client\target\release\conquer-sandbox.exe`.

Para abrir o cliente, execute `run-client.bat` na raiz. Por padrão ele conecta
ao servidor local (`127.0.0.1:9958`); passe `offline` ao script para abrir sem
conexão com o servidor (`run-client.bat offline`). Se o executável ainda não
existir, o atalho compila o cliente antes de iniciá-lo.

Para abrir o cliente conectado ao servidor local:

```powershell
cd R:\gravak-co\client
cargo run --release --locked -p conquer-sandbox -- --assets R:\5065 --online --auth 127.0.0.1:9958 --username test1 --password 123456 --realm CoEmu
```

### Diagnóstico de performance

O cliente normal usa a apresentação sincronizada ao monitor (`AutoVsync`), que
evita tearing e impede que a GPU fique trabalhando sem necessidade. Para testar
um teto fixo, use `GRAVAK_MAX_FPS=60`, `100`, `200`, `300`, `400` ou `500`;
`GRAVAK_MAX_FPS=0` volta ao ritmo do monitor. O modo imediato fica reservado
para benchmark: `GRAVAK_PRESENT_MODE=immediate`. Para ativar o diagnóstico, use
`GRAVAK_PERF=1`; a tecla `F11` também abre o overlay durante o jogo. O log
separa construção do frame, upload, aquisição da superfície, encode e
apresentação. O orçamento padrão é de uma nova textura por frame; ele pode ser
ajustado com `GRAVAK_ASSET_LOADS_PER_FRAME=2` ou `8`.

Para repetir a carga online, mantenha auth/game server ativos e execute no
PowerShell:

```powershell
cd R:\gravak-co\server
$env:GRAVAK_BENCHBOT_COUNT = '50'
$env:GRAVAK_BENCHBOT_MAP = '1002'
cargo run -p benchbot
```

O `benchbot` aceita de 1 a 1200 contas e também `GRAVAK_BENCHBOT_X`/
`GRAVAK_BENCHBOT_Y` para concentrar os personagens em uma posição. A medição
validada no checklist cobre 10, 25 e 50 entidades remotas; o probe
`gravak-probe ... --reconnect` valida reconnect e movimento durante a carga.
Contas novas do benchbot criam o personagem e reconectam automaticamente na
mesma execução.

Para medir a cena completa do cliente, sem abrir uma sessão manual:

```powershell
cd R:\gravak-co\client
powershell -ExecutionPolicy Bypass -File .\tools\benchmark-gameplay.ps1 -DurationSeconds 8
powershell -ExecutionPolicy Bypass -File .\tools\benchmark-gameplay.ps1 -DurationSeconds 8 -Online
```

O primeiro comando mede o gameplay offline; o segundo usa a conta fixture
`test1/123456` e exige auth/game server ativos. Ambos executam o build Release
em modo imediato apenas para medir o renderer, mantendo o VSync no uso normal.
Para validar um teto específico, acrescente `-MaxFps 60`, `100`, `200`, `300`,
`400` ou `500`; sem esse parâmetro o benchmark mede o modo sem limite.

O modo online autentica em uma thread de rede separada, carrega o personagem e usa o mapa/posição retornados pelo servidor. Neste ambiente, auth usa `9958` e game usa `5816`; em uma instalação limpa, use as portas configuradas no `.env`. Sem `--online`, o sandbox permanece offline. Para validar uma nova autenticação sem duplicar o personagem, o probe aceita `--reconnect`; `--map-change` valida uma troca 1002→1000→1002 e restaura o personagem; `--combat` valida um ataque físico contra um monstro visível; `--load <usuário> <senha>` abre duas sessões simultâneas e valida mapa, monstros, combate e coleta; `--invalid-item` envia transições inválidas ao `MsgItem` e confirma as rejeições do handler; `--pickup` valida criação de drop, coleta 1101 e remoção; `--upgrade` cria um item/Meteor e valida a melhoria 1009/20; `--npc-upgrade` valida o diálogo do ArtisanWind e a melhoria por slot; `--assign-attribute vitality` valida o pacote clássico 1024; `--quest` valida a primeira quest completa, do diálogo do NPC 425 à recompensa após derrotar um Pheasant; `--team <usuário> <senha>` valida party em duas sessões; `--trade <usuário> <senha>` valida convite, ofertas, silver, confirmação dupla e troca atômica em duas sessões; `--whisper <usuário> <senha>` valida entrega privada, replay de whisper/Talk após reconexão e destinatário inexistente; `--friends <usuário> <senha>` valida amizade persistente; `--ignore <usuário> <senha>` valida bloqueio, recarga e remoção de ignore. O chat do cliente mantém histórico filtrável da sessão e o botão Clear limpa as mensagens exibidas; os painéis Friends, Team e Trade usam ações server-authoritative, e confirmações/erros operacionais aparecem em um banner dentro do jogo.

Nos limites fixos, o pacing usa uma janela híbrida: aguarda até 2 ms antes do próximo frame e faz polling apenas no trecho final. Assim os caps altos mantêm precisão sem deixar o cliente em polling contínuo durante todo o intervalo.

## Servidor

O servidor exige Rust nightly; a versão está fixada em `server\rust-toolchain.toml`. Para preparar o banco local:

```powershell
cd R:\gravak-co\server
sqlx migrate run --source migrations --database-url "sqlite://data/coemu.db?mode=rwc"
cargo build -p auth --features server -p game
```

Os servidores usam, por padrão, as portas `9958` (auth) e `5816` (game). O banco SQLite, logs, builds e fixtures locais são ignorados pelo Git. As regras das skills ficam em `server/server/game/data/skills.dat`; é possível apontar outro catálogo com a variável `SKILLS_DATA`. Os comandos `$test-health` e `$test-mana` restauram recursos apenas em memória para testes; `$test-skill-book` cria o livro `723100`, que ensina Whirlwind (`1002`) pelo fluxo transacional de uso de item. `$test-durability` cria uma arma com 10/100 de durabilidade para validação manual de combate/reparo; `$test-quality` cria uma arma normal `410005` e um DragonBall para testar qualidade; `$test-refinery` cria um helmet e o material compatível para testar a refinaria; `$test-artifact` cria um helmet, um FlameHat (`820001`) e um Meteor para testar o artefato. O treinamento pelo NPC exige nível 10 e 500 silver para Whirlwind. Armas perdem durabilidade ao atacar e podem ser reparadas pelo ferreiro. O comando `$add-stat vitality 2` (também aceita strength/agility/spirit) gasta pontos disponíveis de forma autoritativa; cada level-up concede 3 pontos. `$upgrade-item <uid>` aumenta em um nível o `plus` de um item próprio; a ação clássica `1009/19` melhora a qualidade com DragonBall, consumindo o catalisador e penalizando a durabilidade em caso de falha. Sockets/gems, composição, refinaria, artefatos e social possuem fluxos equivalentes no menu/NPC/probe. O comando `$quest-status` lista o progresso persistido das quests, e o probe aceita `--skill-book`, `--combat`, `--quest`, `--quest-status`, `--team <usuário> <senha>`, `--friends <usuário> <senha>` e `--pvp <usuário> <senha>` para validar os fluxos ponta a ponta. O mapa de gaps e prioridades está em `docs/reference-gap-matrix.md`.

## Acompanhamento do projeto

O backlog vivo das próximas tarefas está em
[`docs/project-backlog.md`](docs/project-backlog.md). As decisões de escopo e
o status técnico detalhado continuam registrados nos checklists em `docs/`.

## Estado atual

O cliente foi validado contra os assets locais 5065. A base do servidor também foi corrigida e validada com build dos binários de auth/game. O `gravak-probe` já autentica, cria um personagem de teste quando necessário, reconecta, carrega `MsgUserInfo`/`MsgMapInfo` e confirma o primeiro `MsgWalk`. O sandbox gráfico já possui o fluxo online inicial, envia `MsgWalk` a cada passo concluído, reconcilia correções do servidor sem manter segmentos rejeitados, acompanha movimento/saída, direção, salto e `ChangeAction` de atores remotos e desenha jogadores e NPCs com seus nomes. NPCs usam os modelos e texturas próprios dos catálogos 5065; clicar sobre um NPC envia `MsgNpc` e recebe o diálogo de teste do servidor, enquanto cliques no restante do mapa iniciam movimento. A tecla `C` envia uma ação de teste (`RunLeft`) quando o cliente está conectado. O servidor já possui o loop autoritativo de combate PvE, com seleção e validação de alvo, dano, morte, XP, respawn, drops e IA básica de monstros, além de Basic Fire, Flame Burst e Whirlwind com catálogo externo de regras. Ataques físicos e mágicos também aceitam personagens como alvos, com validação de mapa/tela/alcance, modo `Safe`/`Free`, stats autoritários, bloqueio após derrota, `Reborn` e sincronização do HP no mundo e no HUD; o probe confirma o ciclo físico/mágico completo entre duas sessões. Armas perdem durabilidade por ataque, itens quebrados deixam de contribuir para os stats e ferreiros oferecem reparo atômico. O Character/inventário mostram a condição do equipamento e atualizam o tooltip quando a durabilidade muda. O cliente já exibe feedback, cooldowns, progresso recebido pelo 1103, efeitos 1105 multi-alvo, morte visual, ground loot com pickup e menu de contexto com `EQUIP`, `UNEQUIP`, `UPGRADE`, `QUALITY`, `REFINERY`, `ARTIFACT` e sockets; o ArtisanWind também permite melhorar nível ou qualidade de um equipamento selecionado pelo diálogo. A primeira fatia de artefatos persiste/aplica o material ao item correto, calcula resistência de fogo para equipamentos ativos e anuncia os efeitos pelo `2077`; ainda faltam demais elementos, proficiências completas, buffs/debuffs e a cobertura completa de skills.

## Licenças e créditos

As licenças originais permanecem nos diretórios dos componentes:

- cliente: `client/LICENSE` (Apache-2.0);
- servidor: `server/LICENSE` (GPL-3.0-only).

Os créditos e avisos dos projetos de origem devem ser preservados. Nenhum asset proprietário do jogo é distribuído neste repositório.
