# Gravak — lista de desenvolvimento

Checklist vivo das próximas entregas, em ordem de prioridade. Ao concluir uma
tarefa, marque `[x]` e diga como foi testada. `docs/server-backlog.md` guarda o
histórico de paridade com o Redux; `docs/feature-roadmap.md` e
`docs/development-checklist.md`, o histórico detalhado.

## P0 — segurança e dados (antes de abrir o servidor para outras pessoas)

- [x] **Login sem senha pelo game server**: o pacote `MsgTransfer` (4001) dava
      um token de login para qualquer `account_id` a quem conectasse na porta
      do game. Agora exige a chave `TRANSFER_SECRET` do auth server (ou, sem
      chave, só conexões locais). Testado com um programa que pede o token da
      conta 1: recusado e registrado no log com o IP.
- [x] **Comandos `$` liberados para todos**: teleporte, itens de teste, prata,
      `$gw start`... Coluna `accounts.gm` (as contas de desenvolvimento viram
      GM); os demais só usam `dc`, `which`, `quest-status`, `nobility-*`,
      `title-*` e `add-stat`. Os comandos também não vão mais para o chat de
      quem está perto. Teste: `only_game_masters_run_game_master_commands`.
- [x] Contas de exemplo com senha `123456` (`shekohex`, `test1`) vêm das
      migrations, que não podem mudar depois de aplicadas: o auth mostra no
      quadro de início e avisa no log enquanto elas tiverem essa senha.
- [x] Backup do Postgres: `server/scripts/db-backup.ps1` (diário e automático
      no `run-server.bat`, 14 mantidos) e `db-restore.ps1` (salva o estado
      atual antes). Testado: backup, restore e contagem igual depois.
- [x] Corrida no armazém da guild: dois membros sacando o mesmo item, ou
      guardando no mesmo slot, ao mesmo tempo. Testado com 20 rodadas
      simultâneas contra o Postgres.
- [x] Revisar os demais recursos que dois jogadores tocam ao mesmo tempo:
      casamento corrigido (dois aceites simultâneos casavam a mesma pessoa
      duas vezes; teste com 20 rodadas). Amizade já era idempotente (`ON
      CONFLICT DO NOTHING` dos dois lados); trade e banca já conferiam cada
      remoção; a criação de guild passa pelo `GuildManager` em memória.
- [x] `run-server.bat stop` matava os servidores sem salvar (até 30 s de
      progresso perdidos). Agora `server/scripts/stop-servers.ps1` manda
      Ctrl+Break para a janela de cada servidor, e o game salva todos antes de
      sair; fechar a janela no X também salva. Testado: parada em 2 s, com
      `Saved everyone online` no log.

## P1 — operação

- [x] Linha de status com a saúde do banco (`db_ms`, conexões, ociosas); erro
      em vermelho se o banco não responder.
- [x] Contagem de personagens salvos no desligamento, no log.
- [x] Tentativas de hack (portal fora da tela, NPC fora da tela) viram `warn`
      com personagem e conta, em vez de `debug` silencioso.
- [x] Limite de tentativas de login por IP no auth server: 10 por minuto
      (login e registro); a 11ª recebe o código 51 do protocolo, "muitas
      tentativas". Conexões locais não contam. Teste em `auth::limits`.

## P2 — gameplay e conteúdo

- [ ] Quests: cadeias, requisitos, objetivos além de matar monstros, recompensas
      (hoje só existe `Pheasant Hunt`).
- [ ] Banca (player shop): nome visível, persistência e volta após reconnect.
- [ ] Trade: voltar à troca após reconnect; rejeições com mensagem clara.
- [ ] Guild: ranking, cargos avançados, depósito/saque de prata da guild.
- [ ] GuildBeast (Redux 3120) num evento de guilda.
- [ ] Skills: árvore de spells, demais efeitos, buffs/debuffs e fórmulas por
      classe; demais elementos de gems e tipos de refinaria.
- [ ] Reborn: promoção e progressão por classe.
- [ ] IA: patrulha, regras especiais e drops especiais.
- [x] Chat por canal: time, guild, amigos, cônjuge e mundo chegavam só a quem
      estava na mesma região do mapa. Agora cada canal vai a quem deve, sempre
      com o nome real de quem fala, e canais do servidor (System, Center,
      Broadcast...) enviados por um cliente são ignorados. Teste:
      `world_chat_reaches_everyone_under_the_real_name`.
- [x] `$tele <mapa> <x> <y> --all`: o GM leva todos os jogadores online junto
      (antes era um TODO, e a opção nem aceitava a forma `--all`).

## P3 — cliente e interface

- [ ] Feedback visual de inventário, gems, sockets, rarity, upgrade e operações
      rejeitadas.
- [ ] Action bar e HUD sem sobreposição (cooldowns, buffs, alvo, dano, chat).
- [ ] Revisar textos, alinhamentos, clipping e hit zones de todos os painéis.
- [ ] Smoke visual com screenshots de regressão por resolução.
- [ ] Cache de vértices estáticos e medição do agrupamento de draws com mapa
      completo.

## Validação manual (no jogo)

- [ ] Pulo em pontes, árvores, obstáculos e terrenos elevados.
- [ ] Rarity, upgrade, animações e resultado do upgrade em sessão real.
- [ ] Inserir e remover gems sem perder itens.
- [ ] Tooltip do inventário, rarity e sockets com os mesmos bônus e valores.
- [ ] HUD em 1024, 1366 e 1704 px.
- [ ] Login, trade, guild e PK depois da troca para Postgres.

## Feito recentemente

- [x] Terminal do auth e do game: linhas por categoria com cores, quadro de
      início, status a cada 5 min e logs diários em `server/logs/`.
- [x] Banco migrado de SQLite para Postgres (Docker), com testes num banco novo
      por teste; dados do `coemu.db` copiados.
- [x] Restos do SQLite removidos (importadores, dumps SQL, ferramenta de cópia);
      logs de auditoria movidos para `server/logs/`.
