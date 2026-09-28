# Migração para os componentes originais 5065

Este checklist define a nova prioridade visual do cliente. A referência de
componentes é a instalação em `R:\5065`; as imagens anexadas e o cliente
original servem para validar composição, posição, estados e escala.

## Regra de composição

- [x] Centralizar os caminhos dos componentes originais em
      `client/crates/sandbox/src/original_ui.rs`.
- [x] Preservar a grade lógica original de 1024×768 e as áreas registradas em
      `R:\5065\ini\GUI.ini`.
- [x] Usar o cliente 5065 como referência de proporção, ordem, escala e
      comportamento, sem copiar a composição quebrada diretamente dos atlas.
- [x] Desenhar a camada visual própria em `ui_theme.rs`, preservando a essência
      de Conquer com azul profundo, bronze, dourado e fonte decorativa apenas em
      títulos/tabs.
- [ ] Validar normal, hover, pressionado, desabilitado, selecionado e vazio em
      cada tela própria.

## P0 — base do cliente e telas principais

- [~] HUD principal: `GUI.ini [0-130]`, chat, orbes, action bar, botões, mini-map
      e logo.
      O mapa lógico de controles já está centralizado em `original_ui_layout.rs`;
      a arte nativa de cada componente ainda precisa ser ligada ao runtime.
- [x] Character shell: `DialogQuery.dds`, botões verticais nativos, Battle
      Power e botão de fechar.
- [x] Composições nativas de Role, Weapon e Equipment foram geradas a partir
      dos DDS e conferidas em `docs/ui-preview`.
- [~] Character/Role: `DialogQuery_Role.dds` já está ligado ao runtime com
      texto dinâmico nas coordenadas de `GUI.ini`; `Role_More.dds` e o estado
      visual dos botões de atributo continuam pendentes.
- [~] Character/Equipment: `DialogNpcEquip.dds`, slots nativos, ícones de
      equipamento e paper doll já estão ligados ao runtime; faltam os estados
      completos de hover/seleção e a revisão manual em cada resolução.
- [~] Character/Weapon: `DialogQuery_Weapon.dds` já está ligado ao quarto
      modo nativo do Character e recebe ícones das armas equipadas; lista,
      seleção e proficiência ainda precisam ser conectadas.
- [~] Character/Mission/Skills: `DialogQuery_Mission.dds` já está ligado ao
      quarto painel visual; dados e ações de skills continuam pendentes.
- [~] Inventory: grade lógica original, moeda, filtros e estados de item na
      nova camada visual; a revisão final de estados ainda está pendente.
- [~] Warehouse: grade e controles na nova camada visual; falta revisar estados
      de depósito e escala 1024×768.
- [~] Shop/NPC: slots e moldura na nova camada visual; preço, compra, venda,
      reparo e feedback ainda precisam da revisão visual final.
- [ ] Tipografia: usar a fonte de leitura e a fonte decorativa original apenas
      nos títulos, menus e rótulos que tiverem esse papel no cliente 5065.

## P1 — janelas de jogo

- [ ] Skills e action bar (`GUI.ini` IDs de skill/action), com cooldown e
      seleção usando os botões originais.
- [ ] Friends, Team, Guild e Mail, incluindo listas, tabs e botões de estado.
- [ ] Trade e confirmação de operações.
- [ ] Dialogs de NPC, teleporte, quest e lojas.
- [ ] Tooltips, menus de contexto, mensagens e popups modais.

## P2 — acabamento e cobertura

- [ ] Efeitos, ícones de status, minimap, target panel e feedback de combate.
- [ ] Escala e clipping em 800×600, 1024×768, 1366×768 e 1920×1080.
- [ ] Comparar cada tela com uma captura do cliente original e registrar os
      casos de regressão.
- [ ] Confirmar que a migração não reintroduz picos de frame ou carregamento
      síncrono; manter o benchmark de 60/100/200/300/400/500 FPS.

## Ordem de trabalho

1. HUD e escala lógica.
2. Character completo (Role, Equipment e Weapon).
3. Inventory, Warehouse e Shop.
4. Skills, social, trade e dialogs.
5. Estados, responsividade, performance e revisão visual final.

Enquanto este checklist estiver em andamento, novas features de gameplay ficam
em espera, salvo quando forem necessárias para alimentar uma tela original.
