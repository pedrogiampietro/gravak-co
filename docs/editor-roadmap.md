# Ferramentas de conteúdo (Gravak Editor)

Objetivo: criar e alterar o conteúdo do jogo (itens, mapas, NPCs, monstros) sem
mexer à mão em arquivos binários. É o papel que o RME, o ItemEditor e o
OTEditor têm no OTServ.

## Decisões

- **App nativo em Rust** (`client/crates/editor`, egui/eframe sobre wgpu 30,
  a mesma versão do cliente). Assim o editor pode reaproveitar os leitores de
  `sandbox-data` e, nos editores de mapa, o renderer do cliente: o mapa
  aparece no editor como no jogo.
- **Os originais são a base; o nosso conteúdo é a fonte da verdade.** Nada é
  gravado em `R:\5065`. O conteúdo fica em `content/`, em texto legível
  (revisável no git e com merge possível entre pessoas). O build
  (`client/crates/content`, `gravak-content build`) junta tudo em
  `content/build/`.
- **Mesma saída para cliente e servidor.** O cliente lê `content/build/client`
  antes da instalação (`GRAVAK_CONTENT`). O servidor usa o `itemtype.dat`
  gerado (`ITEMTYPE_DATA`). Os dois mudam juntos.
- **Objetos de mapa ficam fora do `.DMap`.** Ficam em `content/maps/<id>.toml`;
  o build gera `gravak/maps/<id>.txt`, que o cliente desenha (montado em
  `sandbox_data::map::ComposedMap`, o mesmo código que o editor usa) e com o
  qual o cliente e o servidor fecham as células bloqueadas. Assim os mapas
  originais continuam intactos, e a escrita de `.DMap` só será necessária
  para mudar o próprio terreno.
- **Arquivos originais reescritos byte a byte.** Linhas que não mudamos saem
  idênticas, inclusive nomes na codificação original. Só os itens que
  alteramos são regravados.

## Marcos

1. [x] **Fundação** (`gravak-content`):
   - leitura e escrita do `itemtype.dat` (decriptar e encriptar), com testes de
     ida e volta;
   - leitura da instalação (arquivos soltos e `.wdf`);
   - encoder DDS DXT5 para os ícones;
   - CLI com `build`, `check`, `items`, `item`, `decrypt` e `encrypt`.
2. [x] **Item Editor**:
   - lista com busca e filtro "só os nossos";
   - formulário com as 39 colunas em grupos; campo alterado fica destacado e
     tem botão para voltar ao valor original;
   - novo item a partir de uma base, com sugestão de ID;
   - duplicar e remover customização;
   - importar ícone (arrastar ou caminho), tirando o fundo (branco ou xadrez),
     recortando e salvando com 64×64; item novo herda o ícone da base;
   - Build pelo editor.
3. [x] **Map Viewer** (aba 🗺 Mapas):
   - qualquer mapa de `GameMap.dat` montado como o cliente monta
     (`ComposedMap`: fundo `.pul`, objetos de terreno e de cena);
   - pan e zoom, "Ir para" uma célula, célula sob o mouse com acesso e
     altura;
   - camadas: cenário, células bloqueadas e portais.
   - [ ] NPCs, spawns e portais do servidor por cima.
   - [ ] Mapas em camadas (`.pux`), que o cliente também ainda não desenha.
4. [ ] **Map Editor, parte 1 (dados do servidor)**:
   - colocar, mover e editar NPCs (`npcs`), spawns (`monster_spawns`) e
     portais;
   - salvar como migration/CSV versionado.
5. [~] **Map Editor, parte 2 (cenário)**:
   - [x] **objetos 2D importados de PNG** (estátuas, decoração): colocar,
     arrastar, ajuste fino e células bloqueadas (presets e Shift + clique),
     tudo em `content/maps/<id>.toml`;
   - [x] o cliente desenha os objetos na ordem de profundidade; o cliente e o
     servidor (`GRAVAK_CONTENT`) fecham as células;
   - [ ] desfazer e refazer;
   - [ ] pintar células andáveis e altura do próprio mapa (escrita de
     `.DMap`);
   - [ ] mover ou remover objetos originais do mapa.
6. [ ] **Extras**:
   - NPC desenhado como sprite 2D (a estátua clicável);
   - aparência de itens equipados (`weapon.ini`/`armor.ini`, reaproveitando
     modelos `.c3`);
   - editores de Shop, monstros e drops;
   - importar a arte do chão (cortar uma imagem em `.pul`/`.pux`);
   - criar mapa novo.

## O que é diferente do Tibia

No Tibia o mapa é feito de tiles e o RME pinta o chão item por item. No Conquer,
o chão é uma **arte grande pré-desenhada**, cortada em pedaços (`.pul`/`.pux`).
Por cima dela ficam a grade de células (andável e altura) e os objetos. Por
isso, o editor de mapa trabalha com:

- objetos, NPCs, spawns e portais;
- células andáveis e altura.

A arte do chão é desenhada fora do editor e importada.
