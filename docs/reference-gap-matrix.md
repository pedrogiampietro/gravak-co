# Matriz de cobertura — Gravak-Co x bases de referência

Este documento é a lista de trabalho comparativa do Gravak-Co. `ReduxConquerProject`
e `Albetros1.1` são referências de comportamento e cobertura; a implementação
continua Rust e server-authoritative.

Legenda: `[x]` implementado e testado; `[~]` existe parcialmente; `[ ]` ainda
não iniciado.

## Já coberto no Gravak-Co

| Área | Referência observada | Estado |
| --- | --- | --- |
| Login, personagem, mapa, movimento e portais | handlers de login/movement/map | [x] |
| Inventário, warehouse, lojas e item transitions | `WarehouseManager`, handlers de items | [x] servidor, limites atômicos e smoke `--warehouse` cobrindo abertura, item e silver |
| Paper doll, equipar/desequipar e aparência | `Player`/item handlers | [x] |
| Monstros, spawn, perseguição, ataque, morte e respawn | `Monster`, `CombatManager`, `SpawnManager` | [x] catálogo de referência (405 tipos, 1.005 spawns, ~8.400 monstros) na migration 39; guardas de cidade fora até existirem regras de PK; IA presa à área de spawn |
| Drops no chão, pickup e auditoria | `GroundItem`, handlers de interação | [x] servidor, ownership, ação 14, pickup 1101, remoção e ocultação pós-morte confirmados |
| Regras de drop por monstro | drop tables/handlers | loot tables/spawn data | [~] `monster_drops` com chance, quantidade mínima/máxima, faixa de nível e grupos exclusivos por `monster_type_id`, seed do Pheasant e seleção server-authoritative; catálogo completo e drops especiais pendentes |
| Skills, cooldown, mana, proficiência inicial e livros | `ConquerSkill`, `SkillEffect` | [~] sete skills verticais prontas, poder por catálogo, escala por nível/proficiência, primeira proficiência de arma persistida com limiares clássicos até o nível 20, Burning com redução de defesa/dano periódico, Stunned com bloqueio temporário de movimento/ataque/skill, Cyclone com flag clássica `1<<23`/cadência física em 1/3 por 10 s, Superman com flag `1<<18`/multiplicador físico PvE/PvP por 15 s, Stigma com flag `1<<9`/+20% de ataque por 70 s e Magic Shield com flag `1<<8`/+20% de defesa por 70 s; flags no `MsgPlayer`, indicadores HUD, interação `Poison` para dano flutuante e expiração; treinamento NPC percorre skills elegíveis por nível/custo sem duplicação e o smoke positivo confirma `1103`/cobrança de silver; árvore/status/regras clássicas pendentes |
| Fórmulas de vida por profissão | `Player.CalculateCombatStats`, `Common.GetTrojanLifeBonus` | [x] bônus de vida do Trojan (100/105/108/110/112/115%) aplicado à vida derivada e à distribuição de atributos; demais fórmulas por classe permanecem em aberto |
| Fórmulas de mana por profissão | `Player.CalculateCombatStats`, `Common.GetTaoistManaBonus` | [x] Water/Fire Taoist usam 100/100/300/400/500/600% sobre `5 * Spirit`, com fallback seguro para outras famílias |
| Durabilidade e reparo | item handlers/serviços de equipment | [x] |
| Atributos base e pontos distribuíveis | `Player`/character stats | [x] servidor, persistência, packet 1024, `$add-stat` e botões `+` |
| Performance inicial do cliente | renderer/map cache | [x] sincronização inicial sem falso level-up; índice estático do mapa preparado antes do primeiro frame; modelos remotos e paper-doll local carregados em worker; índice espacial para evitar rescans dos 9.376 sprites ao caminhar; VSync normal e caps de 60–500 FPS; benchmark Release/immediate aquecido em ~0,57–0,75 ms/frame após a correção |

## Lacunas prioritárias

| Prioridade | Sistema | Redux | Albetros | Gravak atual | Próxima entrega |
| --- | --- | --- | --- | --- | --- |
| P0 | Fixtures e validação de pacotes | packets 1014/1022/1101/1103/1105 | packets equivalentes | [~] fixtures válidos/truncados e smoke `--invalid-item` para seis rejeições de `MsgItem` | completar lojas, sockets/refinaria e confirmações visuais |
| P0 | Operação e carga | threads/managers | handlers e eventos | [~] | autosave periódico dos personagens conectados (incluindo teste de persistência do estado runtime), probe `--load` com duas sessões, mapa, monstros, ataque, morte/drop/pickup; `benchbot` validou 10/25/50 entidades remotas e reconnect/movimento sob 50 bots, além de 50 bots com 2 reconnects concorrentes por bot (150 logins); auditoria econômica cobre as principais entradas/saídas de silver e transferências de itens, e testes atômicos cobrem saldo insuficiente/overflow no warehouse e recompensas; carga além desse smoke permanece |
| P1 | Feedback de combate | action/effects/ground item | effects/arena handlers | [~] | morte, pickup confirmado, XP/dano e cadência no cliente; PvE físico/mágico agora tem vantagem de nível limitada e o multiplicador físico da família Archer, enquanto alvos personagem físicos/mágicos têm resolução server-authoritative e `--pvp` confirma duas sessões, bloqueio em `Safe`, dano em `Free`, morte, bloqueio após derrota e `Reborn`; feedback avançado ainda pendente |
| P1 | Progressão de equipamento | plus/gem/socket/compose/refinery/artifact | handlers de item | [~] | `plus`, qualidade 1009/19, sockets/gems, DragonBall, composição 15:1, bônus físicos Dragon/Tortoise, refinaria 2076 (Intensification/Penetration), artefato 2076/1 (consumo, slot, persistência e reload), resistência de fogo, dano mágico contra personagens, smoke PvP e sincronização 2077 prontos; demais elementos e efeitos de gem/refinery pendentes |
| P1 | Proficiência e magia | spell/proficiency managers | skill handlers | [~] | poder de skill, primeira proficiência de arma persistida, requisitos de equipamento por profissão/arma/nível/sexo/atributos, `MagicAttack`/`MagicDefense` de equipamentos, Burning temporário com redução de defesa + 5% de dano periódico, Stunned temporário com bloqueio server-authoritative, Cyclone clássico com redução da cadência física para 1/3, Superman clássico com multiplicadores físicos PvE/PvP de 10x/2x, Stigma clássico com +20% de ataque por 70 s e Magic Shield clássico com +20% de defesa por 70 s; flags no `MsgPlayer`, HUD e `MsgInteract/Poison` para feedback visual; fórmulas completas e status adicionais |
| P2 | Atributos base na interface | character UI | player UI | [~] | estado desabilitado, feedback de erro e atualização imediata já wired |
| P2 | Quests e NPC scripts | task handlers | event/task handlers | [~] quest `Pheasant Hunt` com NPC 425, progresso por monstro, recompensa e persistência | cadeia de quests, requisitos, objetivos e scripts adicionais |
| P2 | Serviços de cidade | teleport/service NPCs | city handlers | [~] | as cinco Conductress (Twin City, Desert City, Phoenix Castle, Ape Mountain, Bird Island) levam aos portões da própria cidade e ao Market por 100 silver (`teleports.rs`), com destino validado antes da cobrança e estorno se a troca de mapa falhar; Mark.Controller devolve ao ponto de entrada no Market; Wardens da prisão não teleportam mais; faltam prisão com pena, demais NPCs de serviço e restrições por mapa |
| P2 | Party/team | `Team`, `TeamManager` | `Team.cs` | [~] pacotes `1023`/`1026`, convite/aceite, solicitação/aceite de entrada, limite 5, saída, expulsão, dissolução, transferência de liderança, bloqueio/liberação de entrada, painel Team com ações visuais, marcador de líder e barras de HP, XP dividido e pickup compartilhado validados por smoke de duas sessões | regras avançadas de loot e smoke gráfico completo |
| P2 | Trade/player shop | `Trade.cs`, `GameServer.cs` | `Items/Trade.cs`, `VendorItem.cs` | [~] | pacote `1056`, convite/aceite, ofertas de item/silver, cancelamento, confirmação dupla com invalidação, expiração em 60s, fechamento no disconnect, troca atômica de UIDs/upgrades; player shop com ciclo de banca, limite de 10 anúncios, `1108`, consulta na mesma tela, compra atômica, cleanup no disconnect, preço configurável, painel Market, smoke de duas sessões e auditoria econômica; `--trade` usa `$trade-fixture` não destrutivo e confirmou quatro eventos de item/silver | reconnect de sessão, nome da banca, persistência, limites e regras avançadas |
| P3 | Friends/ignore/whisper | social managers | `FriendRepository` | [~] whisper `2001` com `/w`/`/whisper`, friends `1019` persistido, ignore `1019/19,18` com bloqueio/reload, histórico visual filtrável, replay persistente de 40 mensagens `Talk`/`World`/whisper por personagem e painel Friends com online/offline; painel envia adicionar/remover amigo/ignore e prepara whisper por jogador selecionado; banner operacional confirma ações e erros | feedback contextual por ação |
| P3 | Guild/warehouse/ranking | `GuildManager`, `GuildWar` | guild repositories/events | [~] | `GuildManager` com criação, convite/aceite, limite 50, liderança, saída/expulsão, ranks recarregados e cleanup de convites; migrations `30_guilds.sql`/`31_guild_warehouse_items.sql`; carga/persistência de guilds e membros no SQLite; `$create-guild`; `$guild-deposit`/`$guild-withdraw` com transferência atômica por UID preservando plus/sockets/gems; packet 2034 e modal de warehouse com drag-and-drop; depósito liberado a membros e retirada restrita ao líder; `1107` para ações sociais; `2102` para membros online; painel inicial com nome, nível, cargo e ações de aceitar/convidar/sair/expulsar | cargos avançados, smoke gráfico completo, guerra e ranking |
| P3 | Nobility/reputation/title | `NobilityManager` | nobility/title packets | [~] | donation/rank persistidos, doação atômica, comandos de status/doação/ranking, packets 2064/2065, abas Title/Reputation sincronizadas, ranking global no card, botão de doação mínima com hover, catálogo/persistência/comandos `title-status`/`title-equip`, seletor visual na aba Title e smoke online de equipar/restaurar; falta smoke gráfico completo com vários requisitos de desbloqueio |
| P3 | Arena/eventos | arena/guild war/events | arena qualifiers/events | [ ] | somente depois do loop PvE estável |
| P3 | Pets/companions | `PetManager`, `PetAI` | player/pet handlers | [ ] | entidade, summon e persistência |

## Ordem de execução adotada

1. Fechar estabilidade de protocolo, reconnect, troca de mapa e carga.
2. Completar o feedback do loop PvE no cliente.
3. Adicionar progressão de equipamento (`plus`, gems, sockets, compose e refine)
   em fatias pequenas e transacionais; sockets/gems já têm a primeira fatia validada.
4. Completar atributos base pela interface e fórmulas de classe/proficiência.
5. Implementar quests, party, trade, social e guild em ordem de dependência.
6. Fechar eventos, arena, pets, ranking e conteúdo de endgame.

Toda linha só deve virar `[x]` após existir código, teste focado e smoke test
quando houver cliente/protocolo envolvido.
