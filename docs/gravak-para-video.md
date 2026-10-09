# Gravak — o que o servidor tem (briefing para roteiro de vídeo)

> Documento de referência para escrever a narração de um vídeo de divulgação.
> Tudo aqui já está implementado e funcionando no jogo. Se algo não estiver
> listado, não afirme que existe.

## O que é o Gravak

- Servidor privado de **Conquer Online**, fiel ao **patch 5065** (a versão de ~2008, a "clássica").
- **O cliente foi reescrito do zero** (em Rust), usando a arte, os modelos 3D e os efeitos originais do 5065. Os efeitos foram reproduzidos estudando o executável original, então brilhos, números de dano, flechas, mortes e skills se comportam como no jogo de 2008, mas com um cliente moderno, leve e estável.
- O servidor segue as regras do original: as mesmas taxas de forja, as mesmas skills de arma, os mesmos NPCs e preços.
- Projeto de fãs, **sem ligação com a TQ Digital**. Conquer Online é marca dos seus donos. (Isso precisa aparecer ou ser dito em algum momento.)
- Público principal: jogadores brasileiros. O site e as notícias estão em português e inglês.

## O servidor em números

| | |
|---|---|
| Patch | 5065 |
| Nível máximo | 130 |
| Renascimentos | 2 |
| Forja máxima | +12 |
| Experiência | 5x |
| Chefões de mundo | 20 |

## As quatro profissões

- **Trojan**: duas lâminas e muita vida. Entra primeiro e sai por último. O Superman decide guerras de guilda.
- **Warrior**: escudo, armadura pesada e a maior defesa do jogo. Segura o portão enquanto os outros atacam.
- **Archer**: arco e flechas contadas na aljava. O Scatter limpa grupos de monstros, e o Fly passa por cima das muralhas.
- **Taoist**: Fire para quem quer dano, Water para curar e reviver o time. Começa frágil e termina indispensável.

Personagens novos já começam com uma arma da sua classe e poções na bolsa.

## Agenda / conteúdo recorrente

- **Guerra de guildas**: todo **sábado, das 20h às 22h** (horário do servidor). As guildas derrubam os portões e o pilar no mapa das guildas. O pilar passa a levar o nome da guilda vencedora, e o líder dela controla os portões.
- **Chefões de mundo**: os **20 reis-monstro** do 5065 (e a GuildBeast) aparecem **a cada 2 a 4 horas** onde a espécie vive. O servidor inteiro é avisado, e quem dá o golpe final leva a fama e o saque.
- **Recompensa diária**: um prêmio por dia na **estátua do Gravak**, na praça de Twin City, num ciclo de 7 dias. Quem perde um dia recomeça o ciclo.
- **Eventos na arena**: Team Deathmatch, Freeze War, Infection, Capture the Flag e Last Man Standing.
- **Dis City**: uma vez por semana, heróis de nível 110 passam pelo Hell Gate e pelas salas do inferno até a Battle Formation.

## Sistemas do jogo

### Progressão
- **Renascimento**: o primeiro e o segundo, com o Alex no Market (nível 120, ou 110 para Water Taoist).
- **Promoções de classe** com as recompensas do guia oficial (sets, armas Super, gemas, ExpBalls).
- **Missões**: a cadeia do Old General Yang em Twin City, cadeias de missões nas outras quatro cidades, a missão da carta de amor (Milly, Minner e Joe) e um registro de missões.
- **Missões de caça (Bounty)**: quadro de tarefas com listas Normal, Silver e Gold, recompensa em pontos e um rastreador no HUD.
- **Área de treino** com bonecos que dão EXP e proficiência, e **treino offline**: com o Heaven's Blessing, o jogador acumula minutos, entra no treino offline, desloga e volta com EXP.
- **Mineração** com PickAxe: minérios, gemas e outros itens.

### Combate
- **15 skills de arma passivas** (Phoenix, Rage, Snow e outras) com a chance do original, que cresce a cada nível. **O efeito muda de cor conforme o nível da skill**, e todos que estão vendo enxergam a cor.
- **Skills XP**: Superman e Cyclone com contador de KOs e o fogo na base da tela; Fly do Archer como no original.
- **Recorde de KO**: o servidor guarda a maior sequência de KOs num único Superman ou Cyclone, e ela aparece no ranking.
- **Lucky time**: quem tem 2 renascimentos usa o Bless e reza; quem está perto absorve a sorte (golpes críticos, golpes ignorados, forja com mais sucesso).
- **Pets** invocados (Golem, WaterElf, NightDevil).
- **PK**: cor do nome, nomes que piscam para atacantes e perda de equipamento ao morrer conforme a cor do nome.
- **Mortes como no original**: um golpe muito maior que a vida do monstro o arremessa longe. A **alma do monstro sobe e voa até quem o matou**.

### Itens e forja
- **Forja até +12** com os mesmos NPCs e preços: qualidade (DragonBall), nível (Meteor), +1 a +12 no WuxingOven e sockets.
- **Janela de forja dos artesãos** (ArtisanWind e os outros): Meteor sobe o nível e DragonBall sobe a qualidade, com a chance clássica.
- **Sistema de raridade (exclusivo do Gravak)**: Common, Rare, Epic, Legendary e Mythic. Cada tier dá slots de **atributos extras** (ataque, defesa, vida, precisão, esquiva, penetração...) sorteados pelo servidor. **Armas Legendary brilham em dourado e Mythic em carmesim.**
- **Brilho das armas Super** igual ao 5065, com o flash do golpe por tipo de arma.
- **Skins de arma**: uma arma pode usar a aparência de outra arma do 5065.
- **Itens próprios do Gravak**: a capa **Cloud Cloak** (preta com nuvens vermelhas), o arco **Seraph Bow** com modelo próprio, acessórios de arma e o **Travel Cube** (leva a qualquer cidade a cada 10 minutos).
- Tooltips como os originais: nome na cor da qualidade, requisitos em vermelho e brilho no ícone.

### Social
- **Nobreza**: o jogador doa prata ou CPs e sobe no ranking de nobres. Os 3 primeiros são **King** (Queen para personagens femininos), até o 15º são **Prince** e até o 50º são **Duke**. Depois vêm Earl, Baron e Knight pela doação. **O King tem um efeito especial: o nome "King" em letras de fogo dourado** e uma coroa ao entrar no jogo, além da insígnia ao lado do nome.
- **Títulos Top**: os melhores de cada profissão ganham o título e **um anel dourado que gira em volta do herói**.
- **VIP**: um título dourado "VIP" sobre o nome.
- **Guildas**: janela de guilda completa (convites, pedidos, busca, emblema, lema), fundo da guilda, armazém da guilda e vice-líderes.
- **Times** com o rosto dos membros e a posição deles no minimapa.
- **Comércio** entre jogadores, **barracas no Market** com placa e balão de anúncio, **casamento**, amigos, **chat por canal** com janelas de conversa privada e **links de itens no chat**.
- **Broadcast** (/bc): uma mensagem paga para o servidor inteiro.

### Qualidade de vida
- **Ajudante embutido** (tecla J): usa poções quando a vida ou a mana cai, cura o herói e o colega mais ferido, revive os caídos e usa buffs. Configurável numa janela própria.
- **Atalhos reconfiguráveis** (tecla L), barra de F com skills, poções e emotes, e uso de skill XP com um clique.
- **Ranking dentro do jogo**: nível, nobreza, bounty, sequência diária, riqueza, PK, guildas e recorde de KO, com filtro por profissão.
- **Teleportes** entre cidades com as Conductresses, pergaminhos de cidade e o Travel Cube.
- **Battle Power** detalhado e **ver o equipamento de outro jogador** (com cônjuge e título de nobreza).

## Loja (CPs)

- **Loja dentro do jogo** com categorias, histórico de compras e estoque diário. Vende:
  - VIP de 7, 15 e 30 dias;
  - Poções de EXP (2x por 1 hora, 3x e 5x por 2 horas);
  - Exp Ball, Meteor, DragonBall e seus Scrolls;
  - Celestial Stone e Exemption Token (para os renascimentos);
  - Heaven's Blessing (treino offline e reviver onde caiu);
  - vestimentas (Angelical Dress, Royal Dignity, White Phoenix, Dark Wizard...) e o Travel Cube.
- **VIP (para a conta inteira)**: +10% de EXP de monstros; prata, Meteors e DragonBalls caem direto na bolsa; teleporte grátis com as Conductresses; filtro de saque.
- **Compra de CPs com dinheiro real** via Mercado Pago: **cartão ou Pix no site, e Pix direto no jogo**. **R$ 10 = 100 CPs.** Os CPs chegam ao personagem em instantes.
- A proposta da página inicial: "**Nada de itens à venda que você não conseguiria no jogo.**"

## Site

- Criação de conta (a mesma do login do jogo).
- **Ranking ao vivo** lido do banco do servidor a cada minuto: Nível, Nobreza, PK, Recorde de KO, Guildas e Riqueza, com filtro por profissão.
- **Powergamers** (quem mais ganhou EXP hoje, em 2, 3 ou 7 dias), **Killers** (quem mais matou jogadores) e últimas mortes.
- **Página de cada personagem**: equipamento, histórico de EXP, mortes e vítimas.
- Página das guildas, **contagem regressiva para a próxima guerra de guildas** e quantos jogadores estão online agora.
- Notícias, download do cliente e loja de CPs.
- Discord e Instagram.

## Cliente e instalação

- Instalador para **Windows 10 ou 11 (64 bits)**, com placa de vídeo DirectX 12 ou Vulkan e 2 GB livres.
- **O cliente se atualiza sozinho** ao abrir, com atualizações assinadas, e a senha é enviada criptografada.
- A tela de login lista os mundos e lembra as contas salvas.
- Para entrar: baixar o instalador, criar a conta no site, abrir o Gravak e entrar.

## Pontos fortes para destacar no vídeo

1. **Nostalgia fiel**: o 5065 como era, com a mesma Twin City, as mesmas taxas e os mesmos efeitos.
2. **Cliente moderno feito do zero**: leve, estável e se atualiza sozinho, mas com a cara do original.
3. **Conteúdo que acontece todo dia**: chefões a cada poucas horas, guerra de guildas no sábado, recompensa diária e eventos.
4. **Status e competição**: Nobreza com o "King" em letras de fogo, anel dos Top, recorde de KO e ranking ao vivo no site.
5. **Novidades sem quebrar o clássico**: raridades com brilho dourado e carmesim, itens próprios e o ajudante embutido.
6. **Fácil de entrar**: conta pelo site, instalador único, pagamento por Pix.

## Ideias de cenas (para casar com a gravação)

- O King pulando com as letras de fogo e a coroa; o anel dourado de um Top.
- Uma arma Mythic ou Legendary brilhando e a forja +12 dando certo.
- Um Superman ou Cyclone com o contador de KO subindo e o fogo na base da tela.
- Um chefão de mundo com vários jogadores em volta; a alma do monstro voando até quem o matou.
- A guerra de guildas: o portão caindo e o pilar.
- A estátua do Gravak e a janela da recompensa diária.
- O site com o ranking ao vivo e a contagem para a guerra.

## Cuidados

- Não chamar de "Conquer Online oficial"; é um projeto de fãs.
- Não prometer datas, prêmios ou eventos que não estão listados aqui.
- Não citar outros servidores.
