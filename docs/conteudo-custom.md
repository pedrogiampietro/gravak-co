# Conteúdo custom: mapas, arte e enfeites

O caminho para criar coisas nossas que não existem no 5065 (mapas, arte,
enfeites, itens), do jeito que fizemos as **Fendas** (as criptas). Leia antes
de começar algo novo; atualize quando aprender algo.

## Como o conteúdo nosso chega ao jogo

- Tudo o que é nosso fica em `content/`, em arquivos legíveis e versionados.
  Os arquivos originais (`R:\5065`) **nunca** são alterados.
- O build (`gravak-content build`, que o `run-client.bat` e o
  `run-server.bat` rodam sozinhos quando algo mudou) gera
  `content/build/client/`, com a mesma estrutura da instalação.
- O **cliente** lê `content/build/client` antes da instalação
  (`GRAVAK_CONTENT`). Um arquivo nosso com o mesmo caminho de um original o
  substitui (por exemplo `ini/GameMap.dat` e `ani/MiniMap.Ani`, que o build
  regrava com os originais mais os nossos).
- O **servidor** lê do mesmo lugar o que precisa: `ini/itemtype.dat`, os
  `.DMap` dos mapas nossos (`map/map/`), as células bloqueadas pelos objetos
  (`gravak/maps/<id>.txt`) e o layout das fendas (`gravak/rifts/<id>.txt`).
- O pacote dos jogadores (`client/tools/package-client.ps1`) leva
  `content/build/client` inteiro.

Detalhes de cada tipo de conteúdo: [content/README.md](../content/README.md).

## A perspectiva do Conquer (vale para toda arte)

- **Isométrico 2:1.** Cada célula do mapa é um losango de **64×32 px**. Um
  passo em x anda (+32, +16) na tela e um passo em y anda (−32, +16).
- **Luz de cima e da esquerda.** Faces voltadas para a esquerda/baixo mais
  claras; para a direita, mais escuras.
- **Escala:** um personagem tem uns **100 a 140 px** de altura no jogo. Uma
  tocha fica com ~64 px, um baú com ~62, um pilar com ~170.
- O chão de um mapa é uma imagem grande cortada em blocos de **256×256**
  (`.pul` + `.ani`); paredes e enfeites são **objetos** desenhados por cima,
  ordenados pela profundidade da célula (`x + y`), então os personagens passam
  atrás e na frente deles corretamente.

## Arte com IA

O que deu certo nas criptas: **texturas planas** (vistas de cima ou de frente,
sem perspectiva) que o build projeta e monta em 3D, mais **enfeites prontos**
no ângulo isométrico. Assim tudo sai com o mesmo ângulo e a mesma luz, mesmo
vindo de gerações diferentes.

### Dicas

- Gere **uma imagem por arquivo**, em **1024×1024**. Folhas com várias peças
  juntas saem pequenas demais (cada textura com ~250 px fica borrada e
  repetida no jogo).
- Gere primeiro uma textura (o piso) e use-a como **imagem de referência** nas
  outras ("same style as the reference image"), para manter o estilo.
- Texturas: ligue **seamless/tile** se a ferramenta tiver (Midjourney:
  `--tile`). Para conferir, repita a imagem 2×2 e veja se aparece emenda.
- Enfeites: **fundo branco liso**, objeto inteiro e centralizado, sem sombra no
  fundo. Depois rode o script que tira o fundo (abaixo).
- Prompts em inglês funcionam melhor.

### Prompts: texturas (troque o tema entre colchetes)

Piso:
```
Seamless tileable texture, straight top-down view, [ancient crypt floor made of large irregular dark grey flagstones, worn edges, thin dark joints with a little green moss, subtle cracks], hand-painted fantasy MMORPG texture in the style of early 2000s Asian MMOs, muted cold colors, soft even lighting, no perspective, no objects, no shadows cast by objects, 1024x1024
```

Variações do piso (com a primeira como referência):
```
Same style as the reference image: seamless tileable top-down [crypt flagstone floor, more broken and cracked stones, some missing pieces showing dark earth, dust in the corners], muted cold colors, even lighting, no perspective, 1024x1024
```

Face da parede:
```
Seamless tileable texture, straight front view, [old dungeon wall of large rough dark stone blocks, deep mortar lines, damp stains and moss near the bottom], hand-painted fantasy MMORPG texture, muted cold colors, flat even lighting, no perspective, 1024x1024
```

Topo da parede (tem que ser **visto de cima**, sem a face aparecendo):
```
Same style as the reference image: seamless tileable texture, straight top-down view looking down at the flat top of a thick stone wall, [rough cut stone slabs fitted together, dust and small rubble], no wall face visible, no edges, no perspective, even lighting, 1024x1024
```

### Prompt: enfeites

```
Single isometric game asset, 2:1 isometric angle seen from above at about 30 degrees, light coming from the top-left, [OBJETO], hand-painted fantasy MMORPG style like Conquer Online, muted dark colors, centered, full object visible, plain pure white background, no ground, no shadow on the background, 1024x1024
```

Os `[OBJETO]` das criptas:

| Arquivo | Objeto |
|---|---|
| `tocha.png` | a wall torch in an iron holder with a warm orange flame |
| `pilar.png` | a tall cracked ancient stone pillar with carved runes |
| `pilar-quebrado.png` | a broken stone pillar, only the lower half standing, rubble at its base |
| `ossos.png` | a small pile of skulls and old bones |
| `bau.png` | an old wooden treasure chest with rusted iron bands, closed |
| `caixotes.png` | two stacked old wooden crates, one broken |
| `correntes.png` | rusty iron chains and shackles hanging from a short stone post |
| `estatua.png` | a weathered stone statue of a hooded guardian holding a sword |
| `braseiro.png` | an iron brazier with glowing coals and blue ghostly flames |
| `teia.png` | a large dusty spider web, flat, for a corner |

### Prompt: conceito (para alinhar o visual antes de gerar as peças)

```
Isometric 2:1 view of a narrow dark [crypt labyrinth] in a fantasy MMORPG, [stone flagstone floor, thick stone walls forming corridors about 3 meters wide, wall torches casting warm light, broken pillars, skull piles, an old chest, blue ghostly brazier], hand-painted style like Conquer Online, moody, 16:9
```

### Tirar o fundo dos enfeites

```
python content/art/tirar_fundo.py <pasta com as imagens> --out content/art/<tema>
```

Deixa o fundo branco transparente (com borda suave), recorta no objeto e salva
em PNG. Imagens que já vêm transparentes só são recortadas. Confira o
resultado: partes brancas **dentro** do objeto (dentes de caveira, teia) podem
ficar transparentes; se incomodar, gere de novo com o objeto sobre fundo
cinza-claro e ajuste `LIGHT` no script.

## Mapas nossos: as Fendas

`content/rifts.toml` lista as fendas; o build (`client/crates/content/src/rifts.rs`)
gera cada uma:

- **Labirinto:** um maze (busca em profundidade, com alguns laços) de
  `rooms = [largura, altura]` salas. Cada sala tem **5×5 células** de piso, as
  paredes **2** de espessura; uma passagem abre 3 células da parede. Algumas
  salas vizinhas viram câmaras (com pilares), e as 4 salas em volta da mais
  distante da entrada viram o **salão do Guardião**. A mesma `seed` gera sempre
  o mesmo mapa.
- **Piso:** as texturas `chao-*.png` projetadas em isométrico; o chão se
  repete a cada 2 blocos de 256 px, então poucos blocos cobrem qualquer mapa
  (`data/map/puzzle/gravak-cripta/`, `ani/gravak-cripta.ani`).
- **Paredes:** um bloco por célula de parede perto do piso, com face de
  `parede-face.png` e topo de `parede-topo.png` (16 variações, para a textura
  continuar de um bloco para o outro). A borda externa tem 8 células de
  parede, para esconder o piso além dela.
- **Enfeites:** pela planta do labirinto (becos sem saída guardam baú,
  caixotes ou correntes; tochas e teias nas paredes; braseiros e estátuas no
  salão). A altura de cada um no jogo e se ele bloqueia a célula estão em
  `PROPS`, no `rifts.rs`.
- **Saídas:** `.pul`, `.DMap` (células andáveis), o mapa no `GameMap.dat`, o
  minimapa (`ani/MiniMap.Ani` + `data/MiniMap/gravak-rift-<id>.png`), as
  paredes e enfeites como objetos (`gravak/maps/<id>.txt`) e as células de
  cada sala para o servidor (`gravak/rifts/<id>.txt`).

### Criar uma fenda nova

1. Acrescente um `[[rift]]` em `content/rifts.toml` com um `id` livre (as
   fendas usam 9101–9129).
2. Crie uma migration em `server/migrations/` com a linha do mapa na tabela
   `maps`. Copie de `95_rift_maps.sql`: o `path` é `gravak-rift-<id>.cmap`, o
   ponto de reviver é a entrada (13, 13) e `flags = 588` (sem gravar a posição,
   sem PK, sem teleporte, sem voo).
3. Se quiser o nome no canto da tela, acrescente o id em `map_name`
   (`client/crates/sandbox/src/main.rs`).
4. Reinicie servidor e cliente e entre com um GM: `$tele <id> 13 13`.

**Nunca altere uma migration que já rodou** (o servidor confere o conteúdo
delas): crie outra, como a `96_rift_crypts.sql` fez.

### Um tema novo (gelo, templo, esgoto...)

Hoje o gerador lê a arte de `content/art/cripta` (`ART` no `rifts.rs`). Para
outro tema:

1. Gere a arte com os prompts acima trocando o tema, com **os mesmos nomes de
   arquivo**, numa pasta `content/art/<tema>/`.
2. Faça o gerador aceitar um campo `theme = "<tema>"` por fenda (a pasta da
   arte, e um `ani`/pasta de blocos e objetos por tema em vez de
   `gravak-cripta`).
3. Ajuste em `PROPS` os enfeites do tema (nome, altura no jogo, se bloqueia).

## Monstros nossos

No Conquer, um monstro não é um sprite: é um **modelo 3D** (`.c3`: malha e
animações) vestido por uma **textura** (`.dds`, a "planta" achatada do corpo).
O 5065 faz monstros novos reaproveitando um modelo com outra pele (o
GreenDevil, 397, é o DevilKing, 197, em verde), e nós fazemos o mesmo:

- `content/monsters.toml` lista os nossos: o `look` novo (o
  `monster_types.look` do servidor), o `base` (o monstro original de onde vêm
  o modelo e as animações) e a pintura: `colour` e `strength` (a cor nova,
  mantendo a luz de cada pixel), `light` (mais claro ou escuro) e `glow` com
  `glow_from` (as partes mais claras brilham nessa cor, por cima do escuro).
- O build (`client/crates/content/src/monsters.rs`) pinta a textura do base,
  grava `c3/texture/gravak/monster-<look>.dds` e acrescenta ao `armor.ini`
  (o modelo do base com a nossa textura), ao `3dtexture.ini` e ao
  `3dmotion.ini` (as animações do base no look novo).
- No servidor, o monstro usa o look por uma migration (como a
  `99_rift_creature_looks.sql`).
- Looks livres: confira que `[<look>000000]` não existe no `armor.ini`
  original nem `0<look>000...` no `3dmotion.ini`. Os das fendas são 951–979.

Escolha o base pelo **tamanho e forma** (um chefe pede um modelo grande: o
IcyBladeDevil 329, o LightningApe 225, o NagaLord 162). Para ver os monstros
antes do jogo, dá para desenhá-los com `character::NpcModel::load_monster` e
`vertices_static`, como os testes de `marketing.rs` desenham o mundo.

Uma pele feita por IA também serve: exporte a textura do base (o teste
`texture_dump` de `showcase.rs`), peça para a IA **repintar a imagem mantendo
exatamente o desenho e as posições** e use-a no lugar da pintura por código.
Se a IA mover alguma parte, a textura fica errada no corpo.

## Criaturas novas (modelos do Meshy)

Para uma criatura que não existe no 5065 (modelo novo, não só pele nova):

1. **Conceito:** uma imagem por criatura, corpo inteiro, de frente, em
   A-pose, braços afastados do corpo, pernas visíveis até os pés, nada
   solto/flutuando, fundo liso, 1024 px ou mais. Rig automático só funciona
   com 2 braços e 2 pernas.
2. **Meshy (Agente ou Image to 3D):** anexe o conceito e peça um modelo
   game-ready, low poly (3.000 a 6.000 triângulos), uma textura só de cor
   (sem PBR), auto-rig humanoide e as animações; exporte em **GLB**. O plano
   grátis exporta andar e correr; parado, ataque e morte vêm se você
   escolher na biblioteca de animações.
3. **Arquivos:** `content/art/monstros/<nome>/walk.glb` (obrigatório: traz a
   malha, o rig, a textura e o andar) e, se tiver, `run.glb`, `idle.glb`,
   `attack.glb`, `death.glb`. Os GLBs **não vão para o git** (uns 20 MB
   cada): guarde-os também fora (Drive, por exemplo). O build pula uma
   criatura sem `walk.glb`.
4. **`content/monsters.toml`:** um `[[creature]]` com `look` (livre no
   `armor.ini`/`3dmotion.ini`/`3dobj.ini`), `folder`, `name` e `height` (a
   altura em pé; um monstro comum tem uns 200, um chefe 300+).
5. **Servidor:** uma migration que aponta o `monster_types.look` para o look
   novo (como a `100_rift_new_creatures.sql`).

O conversor (`client/crates/content/src/creatures.rs`) escreve o modelo e as
animações em `.c3` (`c3/monster/gravak-<look>/`), a textura em 512 px e as
linhas dos catálogos. Cada vértice fica com os 2 ossos mais fortes (o
cliente mistura os dois). Sem `idle`/`attack`/`death.glb`, ele cria pelo
esqueleto: parado (a pose média do andar, respirando), um golpe com o braço
direito e uma queda para trás.

Prompts que funcionaram, para o Agente do Meshy (anexe o conceito):
```
Turn the attached image into a game-ready 3D model for an isometric MMORPG: [CRIATURA]. Low poly (3,000 to 5,000 triangles), one single base-color texture (no PBR, no normal map), A-pose, faithful to the image. Then auto-rig it as a humanoid and add these animations: idle, walk, attack ([GOLPE]), hit, death. Export as GLB with all animations.
```

## Objetos num mapa original

Para pôr algo nosso num mapa que já existe (como a estátua do Gravak em Twin
City): `content/maps/<id do mapa>.toml`, pelo **Gravak Editor**
(`run-editor.bat`, aba de mapas) ou à mão. Veja o formato em
[content/README.md](../content/README.md).

## Conferir sem abrir o jogo

O cliente desenha qualquer mapa numa imagem com os mesmos códigos do jogo
(`login_scenes::sharp_view` e `map::Map::load`, com `GRAVAK_CONTENT`
apontando para `content/build/client`). As funções de
`client/crates/sandbox/src/marketing.rs` mostram como montar esse tipo de
teste. É assim que dá para ver um mapa novo inteiro antes de testar no jogo.
