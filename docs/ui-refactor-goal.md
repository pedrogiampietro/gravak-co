# Meta da refatoração da UI

## Objetivo

Construir uma UI própria para o cliente, fiel à essência visual e à hierarquia
do Conquer 5065, mas sem depender de recortes frágeis do atlas original. A
regra principal é: cada item deve nascer dentro do seu slot correto, cada texto
deve estar centralizado na sua caixa e o desenho nunca pode piscar quando os
dados dinâmicos mudarem.

## Critérios de aceite

- [x] Uma coordenada lógica única para desenho e hitbox.
- [x] Camadas separadas: base estática, dados dinâmicos, personagem e overlay.
- [x] Tema próprio compartilhado entre as janelas.
- [x] Character com slots reais alinhados ao papel do personagem.
- [x] Character compacto, com tabs em ícones e áreas de dados legíveis.
- [x] Inventory com 40 slots e valores de moeda alinhados.
- [x] Painéis de dados opacos, sem vazamento do mapa por transparência.
- [x] HUD com trilhos visíveis de HP/MP/STA/EXP e molduras de hotkeys vazias.
- [x] HUD bottom com dez hotkeys em sequência contínua e sem transparência do
      mapa atravessando a barra.
- [x] Shop e Warehouse com moldura, slots e controles próprios.
- [~] HUD principal com molduras próprias para jogador, buffs, quest, chat,
      skillbar, menu rápido e minimap; faltam estados completos e revisão.
- [x] Hover e fechamento sem trocar a geometria da janela.
- [x] Fallback da última textura válida para impedir flicker.
- [x] Testes de geometria e build release passando.
- [ ] Revisão visual manual no cliente em 1024×768, 1366×768 e 1920×1080.
- [ ] Concluir estados e acabamento da mesma linguagem visual para HUD, chat,
      action bar e popups.
- [ ] Comparação final por screenshot com o cliente de referência.

## Ordem de execução

1. Character e Inventory: fonte da verdade dos slots e textos.
2. Shop e Warehouse: mesma moldura e mesmos estados visuais.
3. HUD principal: chat, orbes, action bar, menus e minimap.
4. Janelas sociais, trade, NPC e tooltips.
5. Validação visual, escala e performance.

## Regra para novas telas

Uma tela nova só entra no cliente depois de ter um layout lógico explícito,
funções separadas para base/dinâmico/actor/overlay e testes para cada região
clicável. Nenhum componente deve posicionar um filho usando uma coordenada
independente da definição central da tela.
