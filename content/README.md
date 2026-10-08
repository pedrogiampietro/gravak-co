# content/

O conteúdo **nosso** do jogo, em arquivos legíveis e versionados. Os arquivos
originais do cliente 5065 (`R:\5065`) não são alterados: o build junta este
conteúdo aos originais e grava o resultado em `content/build/`, que o cliente e o
servidor leem antes da instalação.

```
content/
  items/      um .toml por item (novo, ou original alterado)
  icons/      ícones dos itens (PNG; o build gera os .dds)
  maps/       objetos nossos em cada mapa: <id do mapa>.toml
  objects/    imagens desses objetos (PNG)
  garments/   garments nossos: o corte de um garment original com textura nossa
  accessories/ acessórios de arma: o modelo de uma arma original com textura
              e efeito nossos, num slot próprio
  textures/   texturas dos garments e acessórios (PNG) e os scripts que as pintam
  build/      gerado pelo build; não editar, não versionar
```

## Fluxo

1. `run-editor.bat` abre o **Gravak Editor**. Também dá para editar os `.toml` à
   mão.
2. No editor: escolha um item, ou crie um com **➕ Novo item** / **Duplicar**,
   altere o que quiser e clique em **💾 Salvar**. Isso grava
   `content/items/<id>.toml`.
3. Ícone: arraste uma imagem para a janela, ou informe o caminho e clique em
   **Importar**. Com "Tirar o fundo da imagem" marcado, o fundo (branco, ou o
   xadrez de um print) fica transparente. A imagem é recortada e salva em
   `content/icons/<id>.png` com 64×64.
4. **🔨 Build** (ou só abrir o `run-server.bat`/`run-client.bat`, que fazem o
   build sozinhos quando algo mudou) gera `content/build/client/`:
   - `ini/itemtype.dat`, os itens originais com os nossos;
   - `data/ItemMinIcon/<id>.dds` e `data/MapItemIcon/<id>.dds`, os ícones.
5. Reinicie o servidor e o cliente:
   - `run-server.bat` usa `content\build\client\ini\itemtype.dat` como
     `ITEMTYPE_DATA`;
   - `run-client.bat` define `GRAVAK_CONTENT=content\build\client`, e o cliente
     lê essa pasta antes de `R:\5065`.
6. No jogo, um GM cria o item digitando `$item <id>` no chat.

Sem o editor, pela linha de comando (a partir de `client/`):

```powershell
cargo run -p gravak-content -- build           # ou: check, items <texto>, item <id>
cargo run -p gravak-content -- decrypt R:\5065\ini\itemtype.dat itemtype.txt
```

## Formato de `items/*.toml`

```toml
[[item]]
id = 721000              # ID novo, ou um existente para alterá-lo
base = 723017            # opcional: copia todas as colunas desse item
icon = "icons/721000.png"
name = "Gravak Statue"
price = 5000
description = "Uma estátua de Gravak."
```

- As colunas que você não escrever mantêm o valor do próprio item, ou da `base`,
  ou ficam em 0.
- Um arquivo pode ter vários `[[item]]`, e uma `base` pode ser outro item nosso.
- Item novo sem `icon` usa o ícone da `base`.
- As chaves são as 39 colunas do `itemtype.dat`: `name`, `price`, `cps_price`,
  `req_level`, `attack_min`, `attack_max`, `defense`, `health_add`,
  `type_name`, `description` e as demais. A lista completa, com a posição de
  cada coluna, está em `client/crates/content/src/itemtype.rs` e aparece no
  editor ao passar o mouse sobre cada campo.

### Escolhendo o ID

O ID diz ao jogo que tipo de item ele é:

- **família:** os milhares definem o slot de equipamento. Por exemplo, `410xxx`
  é arma de uma mão, `500xxx`–`599xxx` é de duas mãos, `130xxx` é armadura e
  `900xxx` é escudo. Itens fora dessas faixas ficam na bolsa.
- **qualidade:** o último dígito (`…3` Normal, `…6` Refined, `…7` Unique,
  `…8` Elite, `…9` Super).

O botão **Sugerir** do editor procura o primeiro ID livre na mesma família e
qualidade da base. Não use um ID que já existe na instalação para um item novo:
isso substitui o item original.

## Acessórios de arma (`accessories/*.toml`)

Um acessório é um item que vai num slot próprio da janela de equipamento
(coluna da esquerda, acima do garment) e veste a arma do mesmo tipo com o
visual dele. Todos o veem assim; sem a arma certa, ele não muda nada.

- `350xxx`: arma de duas mãos (slot direito);
- `360xxx`: arma de uma mão (slot direito ou esquerdo);
- `370xxx`: arco (slot direito);
- `380xxx`: escudo (slot esquerdo).

```toml
[[accessory]]
id = 370001                       # o item (em items/, como qualquer outro)
model = 500279                    # a arma original cujo modelo ele usa
texture = "textures/370001.png"   # opcional: sem ela, a textura da própria arma
mesh = "models/370001.obj"        # opcional: um modelo 3D nosso no lugar do da
                                  # arma (veja "Modelos de arma" abaixo)
aura = "Bow130"                   # opcional: um efeito do 3DEffect.ini sempre
                                  # sobre a arma ("none" para nenhum; sem ele,
                                  # o da arma original)
aura_tint = "ice"                 # opcional: a cor da aura (ice, legendary
                                  # ou mythic)
```

A aura fica presa à arma, como o brilho dos arcos Super. Escolha efeitos de
brilho de arma (`999.999.<família>.<id>` no `Action3DEffect.ini`); efeitos
de partículas que se espalham, como o `angelwing`, deixam um rastro no chão.

O build desenha o acessório como a arma `<família>9<dois últimos dígitos do
id>` (o 370001 vira o 500901), um ID que nenhuma arma original usa. Ele copia
as poses, os golpes e as flechas do `model` e grava `ini/accessory.ini`, que
o servidor e o cliente leem. Por isso dois acessórios do mesmo tipo de arma
não podem terminar com os mesmos dois dígitos.

O item em si fica em `items/`. O Seraph Bow (`items/seraph_bow.toml`) usa o
Cloud Cloak como `base`: sem atributos e sem desgaste.

### Modelos de arma (`models/*.obj`)

Um acessório pode ter um modelo 3D próprio, num `.obj` (o formato que o
Blender exporta, com coordenadas de textura). O build coloca esse modelo no
`.c3` da arma do `model`, mantendo os ossos e os movimentos dela. Assim ele é
segurado, posado e atira como ela, e a corda do arco é puxada.

1. Exporte a arma original como referência:
   `cargo run -p gravak-content -- weapon-obj 500279 ../content/models/500279-reference.obj`
   (a partir de `client/`).
2. Faça o modelo nesse mesmo espaço (no Blender: importe a referência,
   modele por cima, apague a referência e exporte com "Y up"). Cada vértice
   segue o osso do vértice original mais perto; um grupo `g bone3` fixa o
   osso das faces dele (o 5 é o meio da corda do arco).
3. A textura é a do `texture` do acessório, mapeada pelas coordenadas do
   `.obj`.

O Seraph Bow é feito assim, inteiro por script:
`python content/models/make_seraph_bow.py` gera o modelo, a textura e o
ícone.

## Objetos nos mapas (estátuas, decoração)

Na aba **🗺 Mapas** do editor:

1. Escolha o mapa na lista. Twin City é o **1002**. O mapa aparece como no
   cliente, com fundo, cenário e portais (em azul).
2. **Navegar:**
   - **setas ou WASD** andam pelo mapa (com Shift, mais rápido);
   - roda do mouse, ou **+** e **-**, dá zoom;
   - botão direito ou do meio arrasta o mapa;
   - "Ir para" centraliza numa célula, e duplo clique num objeto da lista vai
     até ele.
   A célula sob o mouse aparece no canto, com o acesso e a altura dela.
3. **Novo objeto:** arraste uma imagem para o mapa, ou informe o caminho e
   clique em **Importar**. Depois clique no mapa onde ele deve ficar. O pé da
   imagem (o centro de baixo) fica na célula clicada.
4. **Editar um objeto:**
   - arraste com o botão esquerdo para mover, célula a célula;
   - "Ajuste (px)" faz o ajuste fino da imagem;
   - **Bloqueia** define quais células ninguém atravessa: Nenhuma, Só a base,
     3×3 ou 5×5;
   - **Shift + clique** liga ou desliga uma célula;
   - **Delete** remove o objeto.
   "Células bloqueadas" mostra em vermelho as células bloqueadas do mapa e em
   laranja as que os nossos objetos bloqueiam.
5. Clique em **💾 Salvar mapa**, que grava `content/maps/<id>.toml`. Reinicie
   o servidor e o cliente: o `run-server.bat` e o `run-client.bat` refazem o
   build sozinhos quando `content/` mudou (o botão **🔨 Build** faz o mesmo na
   hora). Ao carregar um mapa, o console do cliente mostra
   `map 1002: 1 Gravak objects (content …)`; se aparecer
   `Gravak content off`, o build não foi gerado.

No jogo:

- o cliente desenha o objeto na ordem certa: quem está atrás dele fica
  escondido, quem está na frente passa por cima;
- o cliente e o servidor fecham as células bloqueadas;
- todo jogador que passar por ali vê o objeto.

Já existe uma: a **Estátua de Gravak** em Twin City (`maps/1002.toml`), a
relíquia em pé no centro do tablado da praça (célula 438, 377). O sprite
(`objects/gravak-statue.png`) é gerado por `objects/src/make-gravak-statue.py`
a partir de `objects/src/gravak-relic.png`: o fundo sai e a arte é girada com
RotSprite (o algoritmo de girar pixel art sem borrar), reta pelo eixo do disco,
com o cabo embaixo e no tamanho em que foi desenhada. Para uma peça maior,
passe uma escala inteira
(`python make-gravak-statue.py gravak-relic.png ../gravak-statue.png 2`).

O formato, para editar à mão:

```toml
[[object]]
name = "Estátua de Gravak"
image = "objects/gravak-statue.png"
x = 430
y = 380
offset = [0, -10]             # opcional, em pixels
block = [[0, 0], [1, 0]]      # células, relativas a x/y
action = "daily_rewards"      # opcional: o que um clique nele faz
```

### Ações

Um objeto com `action` responde ao clique (esquerdo ou direito) no jogo:

- `daily_rewards`: a **Reward Shrine**. Abre a janela de Recompensas Diárias
  (a Reward Wall), como no Tibia. O servidor só a abre para quem está a até 4
  células do objeto. O ciclo de 7 dias, com as opções de cada dia, fica em
  `server/server/game/data/daily_rewards.dat`.

### O que um item novo ainda não faz sozinho

- **Efeito ao usar** (buff, teleporte…) precisa de código no servidor
  (`server/server/game/src/special_items.rs`).
- **Aparência equipado** (arma na mão, armadura no corpo) vem de `weapon.ini`,
  `armor.ini`, `3dobj.ini` e dos modelos `.c3`. Isso ainda não passa pelo
  editor; veja `docs/editor-roadmap.md`.
