# Gravak — lista de desenvolvimento

Checklist vivo das próximas entregas, em ordem de prioridade. Ao concluir uma
tarefa, marque `[x]` e diga como foi testada. `docs/server-backlog.md` guarda o
histórico de paridade com o Redux; `docs/feature-roadmap.md` e
`docs/development-checklist.md`, o histórico detalhado.

## Rodada atual (em ordem)

1. [x] Drops de equipamento (20%, do nível do monstro) e de poção (4%), como no
       Redux: família, nível e qualidade (Refined 10%, Unique 6%, Elite 1%,
       Super 0,3%); só itens que existem no itemtype.dat (~99% dos sorteios).
2. [x] Proteção ao reviver: 5 s sem receber dano depois de levantar.
3. [x] Bounty: listas de criaturas preferidas e evitadas (pagas com bounty
       points) e confirmação antes de gastar token ao abandonar.
4. [x] Store: histórico de compras e novos itens (poções de 3x/5x de exp,
       garments na aba Cosmetics no lugar do Cosmetic Token, que não existe no 5065).
5. [x] VIP: teleporte para qualquer cidade (1 min de espera) e filtro de loot
       (aba VIP Services da Store; o filtro escolhe o que vai direto para a bolsa).
6. [x] Botões ornamentados (`ornate.rs`) no resto da UI: Bounty, Revive,
       confirmações e janelas.
7. [x] Comando de GM `$monster` para criar monstros (reorganizar como o mapa
       guarda os monstros). `$monster <nome|id> [qtd]` e `$monster-clear`;
       somem 5 s depois de mortos, sem respawn.
8. [x] Mais quests (outras cidades, outros objetivos), banca com nome e prata
       da guild. Quatro cadeias novas (CoachLin, OldGeneral, GeneralShou,
       GossiperWang) com caça e entrega de itens; banca com nome, anúncios
       guardados; fundo da guild com doação e saque do líder.
9. [ ] Pagamento real dos CPs por um site (projeto à parte; decisões do Pedro).
10. [x] Validar skills, magias, effects, XP skills e o "super especial" contra
        o original. Referências que o Pedro mandou (imagens no chat de
        2026-10-03):
        - anel verde no chão com raios amarelos e números grandes azulados
          (990, 1337) em vários monstros: skill de área;
        - explosões roxas/brancas em anéis sobre vários monstros: magia de
          área;
        - pentagrama dourado no chão com raios brancos sob o personagem:
          o efeito "super especial".
        Feito: teste que carrega o effect de todas as skills do MagicType.dat
        (132 effects; só `hitlight`, de skills de monstro, não existe); nomes
        "1－bomb4" do Snow corrigidos; status effects lidos do
        statuseffect.ini (Reflect, GodBelieve, Luck*, veneno...). Falta o Pedro
        olhar no jogo e apontar o que ainda estiver diferente das imagens.
11. [x] Stamina enchendo (sentado e parado) e o efeito especial quando enche.
        Referência: personagem com raios azul-claros em volta dos pés e um
        brilho branco (a confirmar se é esse o efeito da stamina cheia).
        Stamina já enche (3/s, 11/s sentado com X) e só gasta em FastBlade,
        ScentSword, Hercules (20-30) e XP skills (100). O "especial" do
        original é o CircleUp do círculo de XP cheio (Conquer.exe), agora
        desenhado enquanto o XP está pronto.
12. [x] Effects de itens: os encantos de arma (club, wand, Snow etc.).
        Referências: lâminas de gelo azul-claro subindo do chão perto do
        personagem (Snow); ícones de arma (arco, cetro e outro).
        Feito: as 12 weapon skills do Redux (Snow no club, Phoenix nas
        lâminas 420/421, WideStrike, Boreas, Halt, SpeedGun, Penetration,
        StrandedMonster, Seizer, Rage, Celestial, Roamer) saem no ataque
        normal pela chance do MagicType; aprendidas pelos livros 7250xx.
        Earthquake, Boom e FreezingArrow (dano fixo + status) ficaram de fora.
13. [x] Contador na tela para Cyclone, Superman, Speed e outros buffs.
        Feito com o 16: barra de ícones (skill/item do original) com o tempo
        restante; o servidor manda o fim de cada status (pacote 9120).
14. [x] Blade (Fast Blade / Scent Sword) com o effect certo. Referência:
        rastros verdes longos atravessando a tela, vários acertos de
        50.000+.
        Feito: o MagicType guarda um rastro .tme (FastBlade windblade4.tme,
        ScentSword swordrain4.tme); o cliente desenha os effects dele um
        após o outro na direção do golpe, para toda skill que tiver um.
15. [x] Ao matar uma criatura, a alma/luz que sai dela e voa até o
        personagem. Referência: figura azul translúcida (a alma) subindo
        perto dos personagens numa caverna.
        Feito: o effect `soul` do original (Conquer.exe, MagicEffect.cpp) sobe
        do monstro morto e voa num arco até quem o matou.
16. [x] Ícones dos bônus ativos (VIP, exp dupla, Heaven Blessing etc.) na
        HUD. Referência: fileira de ícones pequenos (verde, escuro, roxo,
        verde) no canto da tela.

## Rodada de paridade (Redux + Albetros, em ordem)

Levantado em 2026-10-04 comparando NPCs, itens, pacotes, eventos e bosses do
`ReduxConquerProject` e do `Albetros1.1` com o Gravak.

1. [x] Itens do Redux que faltam: pergaminhos de cidade (1060020-24), HairDye,
       Fireworks, NinjaAmulet, BlackTulip e as poções 721331/721333.
2. [x] NPCs do Redux que faltam: a carta de amor de Milly, Minner e Joe
       (600000-600002, recompensa MeteorTear). Os condutores do mapa da guild
       (815-818), o Warden da prisão da guerra (140) e o Mark.Controller já
       existiam; as Guild Conductresses das cidades e o Welcomer são NPCs
       próprios do Redux, que não existem no banco do 5065.
3. [x] Weapon skills que faltaram: Earthquake, Boom e FreezingArrow (golpe + atordoar; FreezingArrow ensinada pelo Archer God).
4. [x] Lucky time e a skill Bless (segunda reencarnação): LuckDiffuse,
       LuckAbsorb, golpe duplo e sorte no upgrade.
5. [x] Broadcast (2050): anúncio pago para o servidor inteiro (/bc no chat, nível 50, 5 CPs, um por minuto).
6. [x] Trade: quem cai no meio da troca é avisado ao voltar (2 min) e o parceiro na hora; pedidos com o nome do personagem.
7. [x] Guild: ranking (top 10 por fundo no GuildDirector) e poderes do vice-líder (convidar e expulsar membros).
8. [x] Bosses: os 19 reis de monstro do 5065 (GhostKing ... BullKing, 8300-8318)
       aparecem a cada 2-4 h onde a sua espécie vive, com aviso ao servidor,
       golpe em área e drop de DB, Meteors e equipamento Elite/Super;
       GuildBeast no mapa da guild paga 1M ao fundo da guild de quem o mata.
       `$boss <id>` para testar. (Os bosses do Albetros eram só esboços.)
9. [x] Eventos (Albetros): Team Deathmatch, Capture the Flag, Freeze War,
       Infection e Last Man Standing (no lugar do Skill Tournament, que no
       Albetros era só um esboço) na arena de eventos (mapa 7002). Inscrição
       de 1 min com `$join`, times equilibrados por nível, meia ExpBall aos
       vencedores; `$event <tdm|freeze|infection|ctf|lms>` e um automático a
       cada 3 h.
10. [x] HUD: action bar, buffs, alvo, dano e chat sem sobreposição em 1024,
        1366 e 1704 px; feedback de operações recusadas.
11. [x] Desempenho: menos draws (vizinhos de mesma textura, pipeline e
        recorte viram uma chamada, mesmo com profundidades diferentes) e o
        encoder só troca estado quando ele muda. O cache de vértices
        estáticos ficou de fora: o quadro já leva ~0,6 ms.
12. [ ] Validação no jogo (Pedro): pulo, gems, upgrade, tooltips, trade, guild
        e PK no Postgres.

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

- [x] Quests em cadeia: o Old General Yang oferece a próxima tarefa disponível
      (quest anterior feita + nível mínimo). Seis etapas pelos monstros de Twin
      City (Pheasant → Turtledove → Robin → Apparition → Poltergeist →
      WarGhost), com prata, experiência e Meteor/DragonBall; a recompensa em
      item espera espaço na bolsa em vez de se perder. Testado no servidor real
      com o probe `--quest`.
- [x] Quests com outros objetivos (entregar itens) e cadeias de outros NPCs e
      cidades (rodada atual, item 8).
- [x] Quests de recado (falar com um NPC: `target_npc`) e de coleta (o monstro
      da quest deixa o item na bolsa com `drop_chance`% enquanto ela corre),
      migração 79. Cadeias acima do 95: VillageHead (Desert City, 95–110,
      Mystic Castle) e Mike (Desert City, 110–125, labirinto), com BatWing,
      DevilHorn e FiendFang (`content/items/quest.toml`). Aba Quests no painel
      QUESTS (pacote 9102): as quests em andamento, a próxima de cada NPC e
      quantas foram feitas. Testes: `messages_and_collections_past_level_95`,
      `quest_log` do cliente.
- [x] Banca (player shop): nome visível, persistência e volta após reconnect.
- [x] Trade: voltar à troca após reconnect; rejeições com mensagem clara.
- [x] Guild: depósito/saque de prata da guild.
- [x] Guild: ranking e cargos avançados.
- [x] GuildBeast (Redux 3120) num evento de guilda.
- [x] Skills do renascimento puro: CruelShade (3050, Trojan: 10/14/18/20% da
      vida restante do alvo por 100 de stamina, no máximo 5 golpes do herói),
      Reflect (3060, Warrior, passiva: devolve 1 golpe físico em 10, sem matar)
      e Dodge (3080, Fire Taoist: +10/20/30 de esquiva por 60 s). Nenhuma das
      referências as implementa; os números vêm do MagicType.
- [x] Refinaria completa: os 275 materiais do Albetros (tipos 301–319),
      encaixe por tipo de equipamento, usados da bolsa (clique direito) no
      item vestido que servem, por 7 dias. Efeitos: Intensification (+vida),
      Detoxication (−veneno), Critical/Skill Critical (×1,5), Immunity,
      Breakthrough, Counteraction, Penetration, Block (para o golpe físico,
      de jogador ou monstro), resistências a Metal/Madeira/Água/Fogo/Terra e
      Magic Defense; chances e resistências limitadas a 60%. Itens em
      `content/items/refinery.toml`; caem só de chefes (um de nível 3–5) e,
      raramente, de monstros fortes (`special_drops.rs`).
- [x] Elementos das magias (`Element=` no `skills.dat`): só a resistência ao
      elemento da magia a reduz (antes a resistência a fogo valia para toda
      magia).
- [x] Dano por classe no PvP: decidido manter a paridade com a referência
      (só o arco muda, ÷3); a identidade de cada classe vem das skills do
      renascimento e da refinaria.
- [x] Reborn: 2º renascimento com regra própria (as referências não têm): os
      pontos somam o bônus do nível do 2º e de novo o do 1º (guardado em
      `first_rebirth_level`, migração 78), e skill ensinada pelo 1º nunca é
      esquecida. Teste de ponta a ponta pelo Alex: `rebirth_journey`.
- [x] Pontos do renascido gastos logo após upar (antes o `+` recusava até o
      autosave). Teste: `reborn_level_up_points_can_be_spent_at_once`.
- [x] IA (`systems/monster_rules.rs`): 102 monstros conjuradores (o `SkillType`
      do Redux, migração 80) atacam com magia de até 7 células; monstro não
      caça herói 25+ níveis acima (revida se apanhar) e bate mais fraco em
      quem está acima (−3%/nível, mínimo 30%) e mais forte em quem está
      abaixo (+2%/nível, máximo 150%); monstro fraco (até nível 70) foge uma
      vez com 15% de vida; a matilha do mesmo tipo a até 6 células responde
      quando um apanha; guardas fazem ronda em volta do posto; chefes (de
      mundo, de campo e do labirinto) ficam furiosos abaixo de 30% (golpes 2×
      mais rápidos), chamam 3 da sua espécie abaixo de 50% e lançam magia em
      área (5 células) a cada 10 s. Teste no mapa real: `monster_ai_journey`.
- [x] Janela Guild System (tecla/botão GUILD; pacotes 9170–9172, migração 81):
      sem guild, abas Guild System (convites recebidos e pedidos enviados),
      Buscar (pedir para entrar com mensagem) e Crie (nome, título, lema,
      descrição; nível 90 e 1.000.000 de silver); com guild, cabeçalho com
      brasão, título, lema, membros, online e level da guild (1% da
      experiência dos membros) e abas Geral, Membros (promover, rebaixar,
      expulsar, passar liderança), Pedidos (convidar por nome, aceitar ou
      recusar pedidos; convites e pedidos expiram em 7 dias), Histórico
      (filtros e busca, 90 dias), Banco (depósito, saque do líder, taxa de
      entrada), Guerra (declarar, aceitar, recusar, encerrar, placar e
      mortes), Mortes e frags (filtros, busca e resumo 12H/24H/3D/7D, 30 dias)
      e Ajustes (do líder; dissolver com confirmação). O nome da guild e o
      cargo aparecem sobre o nome de cada jogador para todos (1014 leva a
      guild). Corrigido: uma saída rebaixava os vice-líderes no banco.
      Testes: `guild_window_journey`, `guild_panel` do cliente.
- [x] Chat por canal: time, guild, amigos, cônjuge e mundo chegavam só a quem
      estava na mesma região do mapa. Agora cada canal vai a quem deve, sempre
      com o nome real de quem fala, e canais do servidor (System, Center,
      Broadcast...) enviados por um cliente são ignorados. Teste:
      `world_chat_reaches_everyone_under_the_real_name`.
- [x] `$tele <mapa> <x> <y> --all`: o GM leva todos os jogadores online junto
      (antes era um TODO, e a opção nem aceitava a forma `--all`).

## Bounty Tasks (como no Tibia)

- [x] Servidor: quadro com 3 monstros por dificuldade (Beginner, Adept, Expert,
      Master por faixa de nível), tarefas Normal/Silver/Gold, reroll tokens (10
      no máximo, 1 grátis por dia), bounty points e talismã com dano, roubo de
      vida, loot extra e abate em dobro. Conta para o time inteiro.
- [x] Cliente: Task Board no painel QUESTS (botão do HUD ou tecla **B**),
      contador sob o minimapa e nome dourado com losango nos monstros da tarefa.
- [x] Testado no servidor real com o probe `--bounty` (quadro, escolha,
      abandono) e um abate que concluiu a tarefa.
- [x] Listas de criaturas preferidas e evitadas (pagas com bounty points).
- [x] Confirmação antes de gastar um token ao abandonar.
- [ ] Validação visual no jogo (Pedro).

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
- [x] HUD em 1024, 1366 e 1704 px.
- [ ] Login, trade, guild e PK depois da troca para Postgres.

## Feito recentemente

- [x] Terminal do auth e do game: linhas por categoria com cores, quadro de
      início, status a cada 5 min e logs diários em `server/logs/`.
- [x] Banco migrado de SQLite para Postgres (Docker), com testes num banco novo
      por teste; dados do `coemu.db` copiados.
- [x] Restos do SQLite removidos (importadores, dumps SQL, ferramenta de cópia);
      logs de auditoria movidos para `server/logs/`.
