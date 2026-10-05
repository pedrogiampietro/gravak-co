# content/

O conteúdo **nosso** do jogo, em arquivos legíveis e versionados. Os arquivos
originais do cliente 5065 (`R:\5065`) não são alterados: o build junta este
conteúdo aos originais e grava o resultado em `content/build/`, que o cliente e o
servidor leem antes da instalação.

```
content/
  items/      um .toml por item (novo, ou original alterado)
  icons/      ícones dos itens (PNG; o build gera os .dds)
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
4. **🔨 Build** gera `content/build/client/`:
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

### O que um item novo ainda não faz sozinho

- **Efeito ao usar** (buff, teleporte…) precisa de código no servidor
  (`server/server/game/src/special_items.rs`).
- **Aparência equipado** (arma na mão, armadura no corpo) vem de `weapon.ini`,
  `armor.ini`, `3dobj.ini` e dos modelos `.c3`. Isso ainda não passa pelo
  editor; veja `docs/editor-roadmap.md`.
