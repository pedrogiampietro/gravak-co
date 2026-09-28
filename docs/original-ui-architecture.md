# Arquitetura da UI própria inspirada no cliente 5065

## Regra principal

As telas do cliente usam uma coordenada lógica única baseada no cliente 5065.
O visual é desenhado pelo próprio cliente — azul profundo, bronze, dourado e
tipografia decorativa — enquanto as proporções, a ordem dos slots e as áreas de
interação preservam a referência original. Renderização e hitboxes devem
consultar a mesma definição de layout.

## Camadas

Cada janela deve ser composta por camadas persistentes:

1. `Base`: moldura, fundo, ornamentos e botões da linguagem visual própria.
2. `Dynamic`: textos, valores, itens e estados de seleção.
3. `Actor`: personagem ou modelo 3D, quando existir.
4. `Overlay`: hover, tooltip e menus contextuais.

Uma mudança de hover ou valor não pode reconstruir a moldura inteira.
Texturas estáticas devem ser carregadas uma vez e permanecer residentes.

## Migração

- [x] Coordenadas centrais para Character e Inventory.
- [x] Character em camadas base/dynamic/actor/equipment.
- [x] Inventory em camadas base/overlay.
- [x] Shop em camadas base/overlay.
- [x] Warehouse em camadas base/overlay.
- [x] Tema visual próprio compartilhado por Character, Inventory, Shop e Warehouse.
- [x] Slots desenhados por uma única função, com estado vazio, ocupado,
      selecionado e hover, evitando itens fora do quadrado.
- [x] Tabs do Character desenhados como ícones próprios, sem texto comprimido
      dentro de abas estreitas.
- [x] Regiões de dados e moeda com preenchimento opaco para impedir que o mapa
      apareça por trás dos valores.
- [x] Fechamento e tabs usando a mesma geometria visual e interativa.
- [x] HUD mantém a última textura válida durante atualizações assíncronas.
- [x] Mapa lógico da HUD principal centralizado com os offsets de `GUI.ini`.
- [~] HUD próprio usando o mesmo tema e a grade lógica 5065: jogador, buffs,
      quest, chat, skillbar, menu rápido e moldura do minimap já usam desenho
      próprio; faltam estados completos e revisão visual manual.
- [ ] Remover definitivamente os geradores legados de cada tela.
- [ ] Criar comparação automática por screenshot em 1024x768 e 1912x914.
