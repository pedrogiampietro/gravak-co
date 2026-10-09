# Fendas do Gravak — dungeon em time

Uma dungeon nossa, que nenhum jogador de Conquer conhece: labirintos novos a
cada entrada, criaturas e chefes próprios, e o contador de criaturas na tela.

## A ideia

A relíquia do Gravak abriu **fendas** para um mundo partido. Um time entra
por uma fenda, limpa o labirinto do outro lado (o contador mostra
`Criaturas 2/300`), e quando a última criatura cai, o **Guardião da Fenda**
desperta no coração do labirinto. Quem vence o Guardião volta com o saque.

## Decisões

| | |
|---|---|
| Time | Até 5 jogadores, no mesmo time, todos perto do líder (sozinho também entra). O líder abre a fenda. |
| Níveis | Três fendas: **Menor** (nível 70+), **Maior** (100+) e **Abissal** (120+). O nível é checado em todos os membros. |
| Entradas | 2 por dia por personagem, contadas ao entrar (dia UTC do servidor). |
| Tempo | 25 minutos. Esgotado o tempo, a fenda se fecha e todos voltam à cidade. |
| Morte | Revive na entrada da cópia da fenda (o reviver normal), com o progresso do time mantido. Se todo o time estiver morto ao mesmo tempo, a fenda falha. |
| Saída | O RiftExit na entrada leva de volta a Twin City; desconectar também tira o jogador (não volta). Os outros continuam. |
| Prêmio | **Saque pessoal**: cada membro recebe o seu na bolsa (sem disputa). Bolsa cheia: cai no chão, só para o dono. |
| Primeira vitória do dia | Um bônus a mais no saque. |

## O que faz ser diferente

1. **Criptas geradas**: o build monta labirintos de corredores de 5 células,
   célula a célula, com arte nossa (`content/art/cripta`): piso, paredes em
   blocos isométricos, tochas, pilares, baús, braseiros, estátuas. Cada fenda
   tem um conjunto de labirintos, sorteado a cada entrada: não dá para
   decorar o caminho.
2. **Criaturas próprias**: famílias novas (nome, cor, tamanho, status, IA e
   drops próprios) feitas sobre modelos originais com texturas nossas.
3. **Criaturas de elite** com modificadores, como as raridades dos itens:
   *Furiosa* (dano maior), *Blindada* (absorve dano), *Vampírica* (cura ao
   bater), *Explosiva* (explode ao morrer). Uma aura colorida mostra qual.
4. **Guardião com mecânicas**: fases em 66% e 33% da vida, invoca criaturas,
   marca uma área no chão antes de um golpe em área (dá para fugir), e se
   enfurece no fim do tempo.
5. **Restantes no minimapa**: quando faltam 10 criaturas ou menos, elas
   aparecem no minimapa, para ninguém ficar caçando a última.
6. **Ranking de tempo** no site e no jogo: o time mais rápido de cada fenda.

## Saque (por membro)

| Fenda | Sempre | Chance |
|---|---|---|
| Menor | Meteors, Exp Ball | DragonBall, gema normal, item Elite |
| Maior | Meteors, DragonBall, Exp Ball | gema refinada, item Super, DB Scroll |
| Abissal | DragonBalls, Exp Balls | gema Super, item Super nível alto, raridade (Rare a Legendary) |

Os valores exatos ficam numa tabela de drop do servidor, fácil de ajustar.

## Fases de construção

1. **Gerador de criptas** (`gravak-content`, `content/rifts.toml`): `.pul`,
   `.DMap`, paredes e enfeites, minimapa. *Feito.*
2. **Instância por time** no servidor (`server/game/src/rifts.rs`,
   migration 97): o RiftKeeper em Twin City, as checagens do time, 8 cópias
   de cada cripta, as criaturas (3 por sala), o tempo, o Guardião, o saque
   pessoal e a saída. *Feito.*
3. **HUD no cliente** (`rift_tracker.rs`, pacote 9180): contador, barra e
   tempo. *Feito;* falta mostrar as últimas criaturas no minimapa.
4. **Criaturas**: peles próprias na Fenda Menor (`content/monsters.toml`),
   modelos novos do Meshy na Maior e na Abissal (`[[creature]]`). *Feito;*
   faltam o golem de carne, as elites e as auras.
5. **Guardião**: as fases, a marca no chão (pacote novo), o saque e o
   ranking de tempo.
