# Checklist do cliente, performance e UI/UX

Este é o checklist vivo para transformar o cliente em uma experiência jogável,
responsiva e visualmente consistente. Cada etapa deve ser validada com teste
automatizado quando possível e com uma sessão manual usando os assets 5065.

Legenda: `[x]` concluído; `[~]` parcialmente pronto ou precisa de validação;
`[ ]` pendente.

## Diagnóstico inicial

- [x] Confirmar que o cliente deve ser executado em Release (`cargo run --release`); o modo Debug não representa a meta de desempenho.
- [x] Identificar criação de um `wgpu::Buffer` por elemento desenhado a cada frame como gargalo de CPU/GPU.
- [x] Substituir os buffers temporários por um buffer dinâmico reutilizado por frame, com crescimento automático quando necessário.
- [x] Testar o renderer após a mudança: 39 testes ativos do sandbox passaram; 4 previews visuais continuam ignorados por dependerem dos assets instalados.
- [x] Medir FPS, tempo de frame, quantidade de desenhos e texturas em uma sessão real; a instrumentação é ativada por `GRAVAK_PERF=1` e emite uma linha a cada dois segundos.
- [x] Separar o custo real do renderer da espera de apresentação: no modo normal (`AutoVsync`) a cena offline ficou em aproximadamente 16,4 ms/frame, sendo 15,3 ms de espera no `acquire`; no modo `GRAVAK_PRESENT_MODE=immediate`, a mesma cena ficou em 0,64 ms/frame (~1.560 FPS). Online, após o aquecimento, ficou em 2,4–2,8 ms/frame (~350–410 FPS), com 65 draws e cerca de 12.400 vértices.
- [x] Reduzir picos de aquecimento: novas texturas externas agora têm orçamento padrão de uma carga por frame (`GRAVAK_ASSET_LOADS_PER_FRAME` permite ajustar de 1 a 64), distribuindo a preparação inicial sem bloquear um único frame; o valor pode ser aumentado conscientemente em máquinas mais rápidas.
- [x] Diagnóstico disponível no log (`GRAVAK_PERF=1`) e no overlay `F11`, agora com FPS, frame time, draws, vértices e texturas; o log também separa `build`, `upload`, `acquire`, `encode` e `submit`.
- [x] Remover I/O de diagnóstico do caminho normal: logs de drag-and-drop/rede só são escritos quando `GRAVAK_DEBUG_LOG=1`; o pacote `UserInfo` não imprime mais uma linha no console a cada atualização de vida/mana.

Smoke de carga online, com `GRAVAK_PRESENT_MODE=immediate` e o cliente em
Release:

| Entidades remotas | Frame observado | Draws | Vértices | Resultado |
| ---: | ---: | ---: | ---: | --- |
| 10 | 2,1–3,1 ms (~320–475 FPS) | 60 | 10.758 | estável |
| 25 | 1,8–4,4 ms (~230–550 FPS) | 63–64 | ~12.400 | estável |
| 50 | 1,4–2,0 ms (~500–715 FPS) | 66–68 | 14.058–15.690 | estável |

Validação após o carregador assíncrono: em Release, a sessão offline ficou em
0,63–0,72 ms/frame (~1.400–1.580 FPS), com `upload` de aproximadamente
0,02 ms; a sessão online ficou em 2,37–2,70 ms/frame (~370–420 FPS), com
`upload` de aproximadamente 0,06 ms. Não houve `Skipped texture`, panic ou
erro de rede nos logs; a autenticação e o carregamento do personagem foram
confirmados antes da medição online.

Após eliminar o I/O de diagnóstico do caminho normal, a validação online
autenticada em Release/immediate ficou em 0,77 ms/frame, com pico de 4,89 ms,
57 draws e 7.476 vértices.

Após mover a geração da geometria idle estática do personagem local e dos
personagens remotos para um worker, uma nova sessão online autenticada em
Release/immediate ficou em 0,74–0,76 ms/frame, com picos de 4,32–5,61 ms,
59 draws e 10.740 vértices. O pico de aproximadamente 40 ms que aparecia no
primeiro carregamento deixou de ocorrer nessa medição.

### Validação mais recente

- [x] A sincronização inicial não é mais tratada como level-up e o índice estático do mapa (9.376 sprites) é preparado antes do primeiro frame. O benchmark online autenticado em Release/immediate confirmou 0,83–0,84 ms/frame, picos de 1,92–3,72 ms, 85–87 draws e 21.648–21.660 vértices.
- [x] O loop passou a ter pacing configurável: o uso normal segue o VSync do monitor e `GRAVAK_MAX_FPS=60|100|200|300|400|500` aplica um teto fixo quando necessário; a validação online autenticada em Release/immediate ficou em 0,87–0,93 ms/frame no primeiro ciclo e ~0,88 ms no ciclo aquecido, com pico aquecido de 1,85 ms.
- [x] Modelos de personagens, NPCs e monstros remotos, além da atualização do paper-doll local após equipamento, passaram a ser carregados em worker. Isso remove a leitura/decodificação de C3 do loop de eventos da janela.
- [x] O recebimento em lote de `EquipmentItem` deixou de reconstruir o paper-doll de forma síncrona no thread da janela; cada atualização agora é enfileirada no mesmo worker de modelos. Na validação online autenticada mais recente, os ciclos aquecidos ficaram em ~1,31–1,42 ms/frame, com picos usuais de ~2,2–3,1 ms; houve um pico isolado de ~12,4 ms, que não se repetiu nos ciclos seguintes.
- [x] O diagnóstico `GRAVAK_PERF=1` passou a registrar também os máximos de `upload`, `acquire`, `encode` e `submit`, além de `build`; na validação seguinte os máximos ficaram em aproximadamente 0,26, 0,15, 0,20 e 2,30 ms, respectivamente, permitindo separar custo de CPU, GPU e apresentação.

## Arquitetura de performance

- [x] O culling do mapa passou a reutilizar os índices de sprites estáticos potencialmente visíveis por célula de câmera, mantendo o teste exato de posição ao gerar cada quad. A sessão online Release/immediate caiu para ~0,74–0,84 ms/frame, com `build` aquecido em ~0,21–0,27 ms e picos de frame entre 1,68 e 4,37 ms.
- [x] A lista de índices do culling passou a ser compartilhada entre frames enquanto a célula de câmera permanece igual, eliminando a cópia da lista no loop principal. A validação final ficou em ~0,74–0,83 ms/frame, com `build` aquecido em ~0,21–0,26 ms e máximo de 0,51 ms no segundo ciclo.

- [x] Reutilizar o buffer de vértices do renderer em vez de criar um buffer GPU por draw.
- [x] Confirmar o ganho em Release com sessão offline e online; o login online também foi validado depois de relançar o game-server e o auth-server na ordem correta.
- [~] Evitar reconstruir geometria estática do mapa quando apenas câmera, personagem ou UI mudarem; sprites estáticos agora são separados dos animados, seus metadados são cacheados e os candidatos visíveis são reutilizados por células de câmera sem pré-carregar todas as texturas. Ainda falta manter vértices estáticos em cache e agrupar draws por fase.
- [~] Separar renderização de mundo, personagem, efeitos e UI em fases com invalidação própria.
- [~] Agrupar draws por textura quando a ordem de profundidade permitir, reduzindo trocas de bind group; draws adjacentes com mesma textura, pipeline e profundidade agora são coalescidos sem reordenar camadas. Ainda falta medir o ganho em uma sessão com o mapa completo.
- [x] Colocar limite/streaming de sprites fora da área visível e cachear animações já decodificadas; entidades, efeitos e loot fora da câmera agora são ignorados com margem de segurança, o cache estático deixou de duplicar strings por frame e o smoke online foi validado com 10/25/50 entidades. Streaming de mapas maiores continua pendente.
- [x] O launcher separa jogo e benchmark: o uso normal fica em `AutoVsync`, evitando tearing e trabalho excedente da GPU; `GRAVAK_PRESENT_MODE=immediate` fica explícito para benchmark, e `GRAVAK_MAX_FPS=60|100|200|300|400|500` continua disponível para um teto fixo.
- [x] Pré-carregar somente o conjunto inicial do mapa e carregar assets adicionais progressivamente; a leitura/decodificação de assets agora ocorre em uma thread dedicada, enquanto o frame principal limita os uploads para a GPU pelo orçamento configurável.
- [x] Medir e corrigir picos de abertura de Character, Inventory, Shop, Warehouse e Guild; `GRAVAK_BENCHMARK_SCREEN` agora cobre as cinco telas, os painéis são gerados fora do frame principal e a faixa de rede/overlay de equipamentos também é assíncrona. Em Release com `GRAVAK_PRESENT_MODE=immediate`, os piores frames iniciais ficaram em aproximadamente 4,2–5,7 ms e os frames aquecidos em 0,68–0,71 ms, acima da meta de 60–500 FPS.
- [x] Fazer smoke de 10/25/50 entidades remotas e reconnect durante movimento sem queda de responsividade; a carga 10/25/50 foi validada e o probe confirmou, sob 50 bots, o mesmo `character_id`, reload do mapa 1002 e movimento confirmado após reconectar.
- [x] Validar reconnect concorrente do servidor com o `benchbot`: `GRAVAK_BENCHBOT_COUNT=50` e `GRAVAK_BENCHBOT_RECONNECT_ROUNDS=2` concluíram 150 logins de mundo, 50 sessões e encerramento limpo, sem falha; o resultado das tarefas agora é propagado como erro.
- [x] Eliminar o pico tardio causado pela chegada dos personagens remotos: etiquetas de atores são geradas fora do frame principal, poses estáticas de personagens em idle são cacheadas persistemente e a construção de poses remotas/NPCs/monstros tem orçamento por frame. No benchmark online em Release com 50 bots, o pior frame ficou em 4,16 ms na primeira amostra e 1,91 ms na segunda; o custo médio ficou em 0,81–0,77 ms, muito abaixo do orçamento de 16,67 ms para 60 FPS.

- [x] Retirar a geração síncrona de geometria idle do primeiro frame: o renderer enfileira as poses de NPCs e personagens para um worker, mantém o frame de renderização livre para upload/submit e usa o cache compartilhado quando o resultado chega. A sessão online autenticada confirmou pico máximo de 5,61 ms, sem repetir o pico inicial de aproximadamente 40 ms.
- [x] Tirar o carregamento de modelos remotos do loop da janela: personagens, NPCs e monstros agora são lidos/decodificados por um worker dedicado; o loop principal recebe o modelo pronto sem bloquear a entrada, a rede ou a apresentação.
- [x] Tirar também a reconstrução síncrona do paper-doll local durante o burst inicial de equipamentos; `benchmark-online.ps1` aceita `-Spawn X,Y` para separar uma posição válida do cliente da posição persistida no servidor durante medições.
- [x] Estabilizar o frame pacing: o cliente Release usa VSync por padrão (`GRAVAK_MAX_FPS=0`) e permite limites de 60, 100, 200, 300, 400 ou 500 FPS, mantendo baixa latência sem deixar CPU/GPU em loop ilimitado.
- [x] Validar a matriz real de caps com `benchmark-gameplay.ps1 -MaxFps 60|100|200|300|400|500`: os caps de 60–200 permaneceram próximos do alvo e os caps de 300/400/500 passaram a usar polling de precisão, atingindo aproximadamente 300,5/400,5/494,5 FPS aquecidos em Release offline; o smoke online a 500 FPS ficou em aproximadamente 455 FPS aquecidos com 81–83 draws e cerca de 30.030 vértices.
- [x] Reduzir o custo do frame pacing fixo sem perder precisão: o loop usa `WaitUntil` até 2 ms antes do prazo e `Poll` somente na janela final. A matriz Release offline mediu aproximadamente 60/99,5/198,5/297/396/494,5 FPS nos caps documentados; o smoke online a 500 FPS chegou a aproximadamente 456 FPS aquecidos, com 83 draws e 30.030 vértices.
- [x] Eliminar o pico de caminhada causado pela varredura do mapa inteiro: o culling agora consulta um índice espacial de sprites estáticos por célula, em vez de percorrer os 9.376 sprites quando a câmera muda de célula. A sessão Release/immediate a 500 FPS ficou em ~0,57–0,75 ms/frame, com piores frames de ~2,4–5,5 ms no aquecimento.
- [x] Criar uma medição reproduzível do gameplay completo: `client/tools/benchmark-gameplay.ps1` executa a cena Release offline ou autenticada, encerra o processo pela duração solicitada e filtra os diagnósticos de FPS. A sessão offline ficou em 0,42–0,43 ms/frame após aquecimento; a online, com personagem/NPCs/monstros/efeitos, em 0,54–0,64 ms/frame. A validação também revelou e corrigiu o caso de posição server-side bloqueada no mapa local, sem relaxar coordenadas informadas explicitamente.
- [x] Evitar que um modelo de monstro ausente interrompa a cena online: `look=900/910` usa o modelo NPC catalogado apenas como fallback para os fixtures de Guard, enquanto outros looks continuam reportando erro de conteúdo. O smoke online passou sem `Monster model unavailable` e ficou em 0,53–0,56 ms/frame após aquecimento.

Validação atual das cinco telas (Release/immediate, 7 s por tela, build de
2026-09-27): após o aquecimento, Character ficou em 0,31 ms/frame com pior
frame de 0,75 ms; Inventory em 0,31 ms com 0,68 ms; Shop em 0,31 ms com
0,71 ms; Warehouse em 0,31 ms com 0,70 ms; e Guild em 0,31 ms com 0,77 ms.
Os piores frames do aquecimento ficaram entre 3,35 e 4,86 ms e nenhuma tela
encerrou com erro. O benchmark confirma que o custo do renderer não é mais o
limitador para 60–500 FPS nessas telas; a próxima frente continua sendo a
consolidação dos componentes visuais compartilhados e a validação manual dos
estados de interação.

## Sistema visual da UI/UX

- [~] Migrar os componentes visuais para a base original 5065; o catálogo de
  assets e a primeira troca do frame/grade do Inventory já usam os DDS
  originais. O plano por tela está em `docs/original-client-ui-checklist.md`.

- [~] Definir layout base responsivo: escala, margens, grid, z-order, clipping e áreas seguras; Character já concentra sua geometria em `StatusLayout`, mas a escala geral ainda precisa ser aplicada às demais telas.
- [~] Definir tokens visuais: Cinzel Decorative apenas em títulos/menus, fonte de leitura para dados, cores de estado, opacidade e hover; as fontes agora são compartilhadas e o escopo tipográfico está separado no Character.
- [~] Criar componentes reutilizáveis: o `UiRect` centraliza geometria e hitbox, e `draw_text_button` já é compartilhado pelo Character entre título/reputação e pelo Inventory nos filtros; ainda faltam janela, cabeçalho, aba, card, slot, tooltip, barra, modal, toast e lista rolável.
- [~] Centralizar hit zones e desenho no mesmo modelo de layout para eliminar desalinhamento entre texto, bordas e clique; Character agora deriva abas e slots de `StatusLayout` para renderização e interação, além do contrato em múltiplas resoluções.
- [~] Padronizar estados `normal`, `hover`, `pressed`, `disabled`, `selected`, `loading` e `error`; o Character já cobre hover/disabled de slots, abas e botão de atributo, e o Inventory agora compartilha selected + hover dos filtros e a geometria dos slots, faltando aplicar o contrato às demais telas.
- [~] Refatorar Character: moldura, título, botão X, paper doll, slots, placeholders, painel de atributos e rodapé; abas Equipment/Attributes, Title (com ações de equipar/remover), Reputation e Fashion agora usam o mesmo card direito, com quebra de texto e previews exportados para revisão.
- [~] Refatorar Inventory: filtros, pilhas, seleção, tooltip, drag-and-drop, contexto e feedback de operação; título, filtros, slots, rodapé, X, hover real dos filtros e tooltip já têm o primeiro corte funcional. O hover é propagado pela assinatura/cache da textura assíncrona nos caminhos standalone, Shop e Warehouse. O contrato cobre 1024, 1366 e 1704, e os previews vazio/preenchido já foram exportados; faltam estados de contexto, feedback de operação e a revisão visual completa do arrastar/soltar.
- [~] Refatorar action bar/HUD: cooldown, buffs, recursos, alvo, feedback de dano e chat sem sobreposição; a camada atual já possui layout responsivo, hit zones, recursos independentes, cinco badges simultâneos de combate e previews revisados em 1024, 1366 e 1704. Ainda falta validar estados em sessão e consolidar componentes.
- [~] Action bar: catálogo de skills, buffs sem alvo e cooldown individual por skill já estão implementados no cliente e no servidor; falta validar todos os estados em sessão e consolidar componentes.
- [ ] Refatorar diálogos/NPC/Shop/Warehouse/Trade/Guild usando os mesmos componentes.
- [~] Criar previews visuais por resolução 1024, 1366 e 1704 e revisar cada tela antes de marcar como concluída; Character, Inventory vazio/preenchido e HUD já foram exportados com a instalação 5065. Ainda faltam estados de hover/tooltip em imagens de referência, combate e as demais telas.

## Fluxo de validação por tela

- [ ] Abrir/fechar por botão X, Escape e clique fora quando aplicável.
- [ ] Confirmar que hover não deixa artefatos, globos ou texturas de estado anterior.
- [ ] Confirmar escala, clipping e centralização em 1024/1366/1704 px.
- [ ] Confirmar que texto permanece dentro do card e que a hit zone acompanha o card.
- [ ] Confirmar que a tela continua responsiva durante atualização de rede, equipamento e animação.
- [ ] Registrar screenshot de referência e o caso de regressão no checklist.

## Ordem de execução

1. Instrumentação de FPS/frame time/draw calls.
2. Cache e invalidação do renderer (malha estática de monstros iniciada; mapa é o próximo corte).
3. Sistema visual compartilhado e layout responsivo (fontes e `StatusLayout` já são a fundação).
4. Refatoração do Character + Inventory.
5. HUD, action bar e feedback de combate.
6. Shop, Warehouse, Trade, Guild e diálogos.
7. Teste de carga visual e revisão final por resolução.
