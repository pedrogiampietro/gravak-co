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
a partir de `objects/src/gravak-relic.png`: o fundo sai, a arte é ampliada
(Scale2x duas vezes) e girada para ficar em pé sobre o cabo.

O formato, para editar à mão:

```toml
[[object]]
name = "Estátua de Gravak"
image = "objects/gravak-statue.png"
x = 430
y = 380
offset = [0, -10]             # opcional, em pixels
block = [[0, 0], [1, 0]]      # células, relativas a x/y
```

### O que um item novo ainda não faz sozinho

- **Efeito ao usar** (buff, teleporte…) precisa de código no servidor
  (`server/server/game/src/special_items.rs`).
- **Aparência equipado** (arma na mão, armadura no corpo) vem de `weapon.ini`,
  `armor.ini`, `3dobj.ini` e dos modelos `.c3`. Isso ainda não passa pelo
  editor; veja `docs/editor-roadmap.md`.
