# Guia da forja: do item Normal ao Super +12

Testado ponta a ponta no servidor (`cargo test -p game forge_journey -- --nocapture`):
uma SunBlade Normal saiu **Super +12, 2 Super DragonGems, +121 HP e -7% de dano**.

Quase todos os NPCs ficam no **Market** (mapa 1036). Para os NPCs que trabalham
com equipamento, **o item precisa estar equipado**.

| NPC | Onde | O que faz |
|---|---|---|
| ProficiencyGod | Market (189, 212) | Sobe a proficiência da arma com ExpBalls |
| MagicArtisan | Market (179, 208) | Qualidade (sempre dá certo) e nível do item |
| GodlyArtisan | Market (193, 212) | Abre sockets |
| WuxingOven | Market (320, 224) | +1 a +9, +9 a +12, encantamento de HP |
| Ethereal | Market (186, 216) | Aumenta o bless (-% de dano) |
| WeaponMaster | Market (178, 192) | Nível acima de 120 (até 130) |
| MillionaireLee | Market (242, 242) | Empacota 10 DragonBalls em 1 DBScroll |
| Alex | Ape City (436, 311) | Renascimento (abençoa um item: -1%) |

## Menu do item (Shift + botão direito) = os mesmos NPCs
O menu faz exatamente o que os NPCs fazem e cobra o mesmo, com o item na bolsa
ou equipado:

| Menu | Igual a | Custo |
|---|---|---|
| QUALITY | MagicArtisan | `1 + 100/chance` DragonBalls, nunca falha |
| LEVEL | MagicArtisan (até nível 120) / WeaponMaster (acima) | Meteors / 1 DragonBall |
| UPGRADE | WuxingOven | +1..+9: dois itens do mesmo +n da bolsa; +9→+10 12 DB, +10→+11 25 DB, +11→+12 4 DBScrolls |
| GEM SOCKET → OPEN | GodlyArtisan | arma 1 e 5 DB; outras peças 12 DB e depois 7 StarDrills (ou ToughDrill, 12%) |

O painel mostra o custo e quanto você tem antes de confirmar.

Como GM, `$go WuxingOven` (ou parte do nome de um NPC, monstro ou jogador
online) leva você até lá.

## 0. Poder equipar — requisitos e ProficiencyGod
Armas altas pedem nível, atributo **e proficiência** da arma (no tooltip:
"Weapon Skill ... Level"). A SunBlade pede nível 105, 160 de Força e
**proficiência de Blade 11**; sem isso aparece "Equip rejected: character
requirements not met".

A proficiência é do **tipo de arma** (todas as blades contam juntas) e sobe
lutando com ela. O **ProficiencyGod** adianta: escolha a arma → `Yes`. Ele
**consome** ExpBalls (não sobem o seu nível): cada nível custa **nível atual + 1
ExpBalls** (1→2 custa 2, ... 10→11 custa 11), do 1 ao 11 são **65 ExpBalls**.
O nível 1 vem de usar a arma ao menos uma vez em combate.

O tooltip do item lista os requisitos (Level, Proficiency, Strength...) e
pinta de vermelho os que você ainda não tem.

Como GM: `$proficiency 410 12` (410 blades, 420 swords, 480 clubs, 500 bows,
421 backswords, 561 wands... os 3 primeiros dígitos do item).

## 1. Qualidade até Super — MagicArtisan
`Upgrade Quality` → escolha a peça → `Upgrade it`. Nunca falha; o preço em
DragonBalls é `1 + 100 / chance`, e a chance cai com o nível do item.

Exemplo (SunBlade, nível 105): Normal→Refined **2 DB**, →Unique **4 DB**,
→Elite **5 DB**, →Super **8 DB** (19 no total).

Alternativa com sorte: Shift + botão direito no item → **QUALITY** (1 DragonBall
por tentativa, pode falhar).

## 2. Sockets — GodlyArtisan
- **Arma:** `Socket my weapon` → 1º socket **1 DB**, 2º socket **5 DB**.
- **Outras peças:** `Socket my gears` → 1º socket **12 DB**; 2º socket
  **7 StarDrills** (garantido) ou **1 ToughDrill** (pode falhar; se falhar, você
  ganha uma StarDrill).

## 3. Gemas — menu do item
Shift + botão direito no item → **GEM SOCKET** → clique no socket aberto →
clique na gema. Super DragonGem (700013) = +15% de ataque cada.
Para tirar a gema, o mesmo painel.

## 4. +1 a +9 — WuxingOven
`Composing Upgrade` → `Compose +1 to +9` → escolha a peça → escolha o par.
Precisa de **dois itens do mesmo +n na bolsa** (podem ser +n Stones):
- se o par for **maior** que o seu item, ele pega o + do par (dois +8 → +8);
- se não, sobe **+1** (dois +8 num item +8 → +9).

Caminho rápido: 4× **+8Stone** → +8 → +9.

Alternativa com sorte: Shift + botão direito → **UPGRADE** (Meteors, chance cai
a cada +; vai até +12).

## 5. +9 a +12 — WuxingOven
`Composing Upgrade` → `Compose +9 to +12` → escolha o passo e a peça:
- +9 → +10: **12 DragonBalls**
- +10 → +11: **25 DragonBalls**
- +11 → +12: **4 DBScrolls** (= 40 DragonBalls; o MillionaireLee empacota)

## 6. HP extra — WuxingOven
`Enchant HP to my gears` → peça → qualidade da gema (consome 1 gema de
qualquer tipo): Normal até +59, Refined até +109, Super até +255 HP. Só fica se
for melhor que o encantamento atual.

## 7. "Damage -7%" — Ethereal (bless)
O item precisa já estar abençoado (o renascimento no Alex abençoa uma peça com
-1%). Depois, no Ethereal: `Upgrade blessed attribute` → peça → `Yes`.
Cada passo dá **+2%** e custa tantas **Super TortoiseGems** quanto o bless
atual: -1→-3 (1 gema), -3→-5 (3), -5→-7 (5). Máximo -7%.

## Para testar como GM
```
$level 130
$item 410205                  (SunBlade Normal)
$proficiency 410 12           (proficiência de Blade no 12)
$item 1088000 --count 40      (DragonBalls)
$item 730008 --count 4        (+8 Stones)
$item 700013 --count 3        (Super DragonGems)
$item 720028 --count 4        (DBScrolls)
$item 700073 --count 9        (Super TortoiseGems)
```
A bolsa tem 40 espaços: peça os materiais na ordem em que for usar.
