# Backlog do servidor (paridade com ReduxConquer / Albetros)

Itens marcados `[x]` estão implementados e aguardando teste no jogo.

## Feito
- [x] Morte, revive, PK (pontos, nomes vermelho/preto/azul, prisão)
- [x] Stamina, círculo de XP, skills de XP, arcos e flechas, armas de duas mãos
- [x] NPCs do Market: ProficiencyGod, GodlyArtisan, WeaponMaster, MillionaireLee, LadyLuck, Alex (1º renascimento)
- [x] Labirinto (Simon, Old Explorers) e Moon Box Quest
- [x] Dis City (`dis_city.rs`, migração 72): Solar Saint em Ape City (nível 110, uma vez por semana), Hell Gate
      (5 SoulStones), Hell Hall (Slayer Points 400 / Archer 600 / Taoist 300), Hell Cloister (9 Hell Wraiths em
      ordem), Battle Formation (4 Syrens = Pluto Token, Ultimate Pluto, tesouro); Dark Horn e Star Sword trocados
      no Solar Saint
- [x] Recompensas de promoção do guia oficial (PrayingStone, ExpPotion, sets Unique/Elite/Super 40 e 70, gemas,
      ExpBalls, DragonBall)
- [x] Guild War: pólo e portões, placar, vencedora, portões, GateKeeper, conductresses, Warden, horário (sábado 20h–22h), `$gw start|end`
- [x] EXP real: tabela `levexp` do Redux, EXP por golpe, taxa ×5, ExpBall (723700) usável, virtude ao upar novatos
- [x] NPCs das cidades: GeneralPeace, Barber, ArenaGuard, Captain, CaptainLi, assistentes das minas + MineAdmin,
      CPAdmin, SurgeonMiracle, ShopBoy, CelestialTao, UnknownMan, Celestine, Shelby, WuxingOven (+10..+12)
- [x] Mineração (tecla M com PickAxe nas minas) e sentar (tecla X, regenera stamina mais rápido)
- [x] Pedidos de login 76/77/78/97 (amigos, proficiências, magias, guilda)
- [x] Catálogo de skills ampliado (curas, FastBlade, Hercules, Volcano, FireRing, RapidFire...), skills de XP
      apagadas sem o círculo, efeitos de status (escudo, Superman, Cyclone...)
- [x] MeteorScroll / DBScroll abrem em 10 Meteors / DragonBalls
- [x] Bless nos itens: Ethereal (+2 por Super TortoiseGems), renascimento abençoado, -N% de dano, "-N%" no tooltip
- [x] Casamento (LoveStone, com o par ao lado) e divórcio (Starlit, com MeteorTear)
- [x] Portais dos mapas (o cliente avisa ao pisar; 166 portais do banco)
- [x] Atributos automáticos pela tabela `stats` antes do renascimento; HP/MP recalculados por nível, classe,
      renascimento e redistribuição
- [x] Outros jogadores veem você sentado e morto
- [x] Revive / Pray em jogadores mortos; curas em aliados (mire o jogador com o cursor)
- [x] Skills de suporte: Accuracy, StarOfAccuracy, Invisibility (monstros não veem), Fly (monstros não alcançam),
      Intensify (próximo golpe ×2), Poison (flecha envenenada)
- [x] Cônjuge na aba de Atributos do Status
- [x] Guardas de cidade (30, do Redux): atacam só nomes azuis piscando e pretos
- [x] Pets: Golem, WaterElf, NightDevil (skills de XP) seguem o dono, lutam e somem no fim do tempo
- [x] Áreas originais: linha (FastBlade, ScentSword) e meio círculo (Scatter), como no Redux
- [x] Roar (enche o XP do time), Dash (golpe de escudo 200%), Meditation (recupera mana)
- [x] GuildDirector completo: dissolver, promover/rebaixar vice-líder, aliados e inimigos (aliados e
      colegas de guilda/time protegidos no modo de PK Team)
- [x] Offline TG pelo NPC TrainingGuide (Twin City) e OfflineTGAdmin (mapa 601)
- [x] Chefes de campo (Satan, BeastSatan, FurySatan, ElfKing, BirdKing, HillMonsterKing, WingedSnakeKing,
      BloodyBat) nos pontos mais fortes de cada mapa, 2 horas para voltar — local e drops definidos por nós
- [x] Encantar HP no WuxingOven (gema Normal/Refinada/Super → +1..59 / 60..109 / 110..255 HP)
- [x] Tinter na loja de tingir (mapa 1008): capacete, armadura e escudo em 5 cores, visíveis para todos
- [x] Chefes do Labirinto: Gibbon, NagaLord, Talon e Syren, um por andar, 1 hora para voltar
- [x] Campo de texto nos diálogos de NPC; GuildDirector cria guilda (nível 90, 1.000.000 de silver)
- [x] Efeitos das gemas (Redux `AddGemByID`): Phoenix +dano mágico, Fury +acerto, Rainbow +EXP, Kylin poupa
      durabilidade, Violet +proficiência, Moon +EXP de skill
- [x] Chance de acerto: 90% base, +Fury e +20% com o status Accuracy; o cliente mostra "Miss"
- [x] Arena (mapa 1005): morrer lá não derruba itens, não dá PK nem prisão, e revive no lugar
- [x] IA dos monstros por tipo (Redux `cq_monstertype`): alcance de visão e de ataque próprios; quem não caça
      (bit 2 do AttackMode, ex. GuildBeast) só revida; todo monstro persegue quem bateu nele por 30 s
- [x] Cliente: animação de cooldown na barra de atalhos; atalhos e painéis abertos salvos por personagem
      (`settings/hud_<id>.ini` ao lado do executável)

- [x] Compor +1..+9 no WuxingOven (Redux `Process_ComposePacket`): a peça vestida e dois itens +n iguais na
      bolsa; acima dela, ela fica +n, senão ganha +1, até +9. Pelo diálogo, porque o cliente não tem a janela
- [x] Resistência mágica dos monstros (Redux `MagicDefence`, em %); SnakeKing na Bird Island (1015): golpes
      tiram 1, magia passa 10%, só revida, volta em 2 horas — local, temperamento e drops escolhidos por nós

- [x] GuildBeast (Redux 3120): aparece no mapa da Guild War (1038, 95,95) a cada 2–4 horas e paga 1.000.000
      ao fundo da guilda de quem o mata (`bosses.rs`; confirmado nos logs do servidor)
- [x] Skills do renascimento puro (CruelShade, Reflect, Dodge), refinaria completa (301–319), elementos das
      magias, 2º renascimento, quests de recado e coleta com cadeias até o 125, aba Quests, IA com magia,
      regra de nível, fuga, matilha, ronda e chefes furiosos — detalhes em `project-backlog.md`

## A fazer
- [ ] Nada pendente desta lista.
