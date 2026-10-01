# Gravak — backlog do projeto

Este é o checklist vivo das próximas entregas. Ao concluir uma tarefa, marque
`[x]`, registre o teste realizado e, quando fizer sentido, adicione uma captura
ou um comando de reprodução.

## Próxima etapa — validação manual

- [ ] Testar o pulo em pontes, árvores, obstáculos e terrenos elevados.
- [ ] Validar rarity, upgrade, animações e resultado do upgrade em sessão real.
- [ ] Testar inserção e remoção de gems sem perder itens do inventário.
- [ ] Revisar tooltip do inventário, rarity modal e sockets para manter os mesmos bônus e valores.
- [ ] Validar arcs, barras, alinhamento e escala do HUD em 1024, 1366 e 1704px.
- [ ] Persistir as configurações do HUD entre sessões; atualmente elas ficam apenas em memória.

## UI/UX

- [x] Criar componentes compartilhados para janela, cabeçalho, abas, cards, slots, tooltips, barras, modais, toasts e listas (`client/crates/sandbox/src/components.rs`).
- [x] Refatorar Shop, Warehouse, Trade e Guild usando os componentes compartilhados.
- [x] Padronizar estados normal, hover, pressed, disabled, selected, loading e error (`WidgetState`).
- [ ] Melhorar feedback visual de inventário, gems, sockets, rarity, upgrade e operações rejeitadas.
- [ ] Finalizar action bar e HUD com cooldowns, buffs, alvo, dano, chat e recursos sem sobreposição.
- [ ] Revisar textos, alinhamentos, clipping, hit zones e abertura/fechamento de todos os painéis.
- [ ] Fazer smoke visual completo com screenshots de regressão por resolução.

## Gameplay e progressão

- [ ] Completar árvore de spells, skills adicionais e fórmulas específicas por classe.
- [ ] Adicionar os demais efeitos, buffs, debuffs, elementos de gems e tipos de refinaria.
- [ ] Completar reborn, promoção e progressão específica por classe.
- [ ] Expandir quests com cadeias, requisitos, objetivos, custos e recompensas.
- [ ] Melhorar IA com aggro, patrulha, regras especiais e drops especiais.
- [ ] Adicionar bosses, eventos de mapa, arena, guild war, pets e companions.

## Sistemas sociais e operações

- [ ] Melhorar feedback visual de Friends, Team, Trade e Guild.
- [ ] Completar regras avançadas de loot de grupo e reconnect durante trade.
- [ ] Adicionar nome visual, persistência e reconnect para player shops.
- [ ] Completar cargos avançados de guild, ranking e guerra de guilds.
- [ ] Finalizar smoke tests gráficos dos sistemas sociais.
- [ ] Completar auditoria econômica de stress e testes de concorrência para itens e silver.

## Qualidade técnica

- [ ] Completar fixtures e validações dos pacotes 5065 prioritários.
- [ ] Finalizar cache de vértices estáticos e fases de renderização com invalidação própria.
- [ ] Medir o ganho de agrupamento de draws em uma sessão com mapa completo.
- [ ] Manter o workspace livre de logs, saídas de benchmark e previews temporários.

## Referências

- `docs/feature-roadmap.md` — visão ampla e comparação com as referências.
- `docs/development-checklist.md` — status detalhado do servidor e dos sistemas.
- `docs/client-ui-performance-checklist.md` — performance, UI e validação visual.
