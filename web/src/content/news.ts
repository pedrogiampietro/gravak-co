// The news, newest first. Each one says what changed in the game, in both
// languages; write it as the player sees it, not as the code does.

export type Category = "update" | "system" | "interface" | "site" | "event" | "maintenance";

type Text = { title: string; summary: string; body: string[] };

export type Post = {
  slug: string;
  /** ISO date, the day it reached the server. */
  date: string;
  category: Category;
  pt: Text;
  en: Text;
};

export const posts: Post[] = [
  {
    slug: "site-no-ar",
    date: "2026-10-07",
    category: "site",
    pt: {
      title: "O site está no ar",
      summary: "Crie a conta pelo site, acompanhe o ranking ao vivo e veja quem está online.",
      body: [
        "A conta criada aqui é a mesma do login do jogo: crie, abra o cliente e entre.",
        "O ranking lê o banco do servidor a cada minuto, com Nível, Nobreza, PK, Recorde de KO, Guildas e Riqueza, e filtro por profissão.",
        "No topo de cada página aparece quantos heróis estão no mundo agora.",
      ],
    },
    en: {
      title: "The website is live",
      summary: "Create your account on the site, follow the live ranking and see who is online.",
      body: [
        "The account you create here is the one you log in with: create it, open the client and sign in.",
        "The ranking reads the server's database every minute: Level, Nobility, PK, KO record, Guilds and Wealth, filtered by profession.",
        "The top of every page shows how many heroes are in the world right now.",
      ],
    },
  },
  {
    slug: "skills-de-arma",
    date: "2026-10-07",
    category: "update",
    pt: {
      title: "Skills de arma revisadas",
      summary: "As 15 skills passivas testadas uma a uma: chance, EXP, níveis e a cor do efeito em cada nível.",
      body: [
        "Phoenix, Rage, Snow e as demais disparam no ataque normal com a chance do original, que cresce a cada nível (a Phoenix vai de 33% a 78%).",
        "O efeito agora mostra a cor do nível da skill para todos que estão vendo, e não só para quem ataca.",
        "Os bonecos da área de treino também disparam a skill e dão EXP para ela.",
        "O sorteio de combate foi refeito: algumas chances saíam abaixo do certo. Isso vale também para drops e forja.",
      ],
    },
    en: {
      title: "Weapon skills reviewed",
      summary: "All 15 passive skills tested one by one: chance, experience, levels and the effect's color at each level.",
      body: [
        "Phoenix, Rage, Snow and the rest go off on a plain attack at the original's chance, which grows with each level (Phoenix goes from 33% to 78%).",
        "The effect now shows the skill level's color to everyone watching, not just to the attacker.",
        "The training stakes set the skill off too, and give it experience.",
        "The combat roll was rebuilt: some chances came out below what they should. This also applies to drops and forging.",
      ],
    },
  },
  {
    slug: "botoes-da-barra",
    date: "2026-10-07",
    category: "interface",
    pt: {
      title: "Os botões redondos da barra",
      summary: "Ver o equipamento de outro jogador, ligar e desligar o mapa e mover a tela, como no original.",
      body: [
        "View Equipment: clique no botão e depois num jogador para ver o que ele veste, com cônjuge e título de nobreza.",
        "Map liga e desliga o minimapa do canto.",
        "Shift Screen: com a caixa marcada, segure Alt e leve o mouse à borda para mover a câmera.",
      ],
    },
    en: {
      title: "The bar's round buttons",
      summary: "View another player's equipment, toggle the map and shift the screen, as in the original.",
      body: [
        "View Equipment: click the button, then a player, to see what they wear, with their spouse and nobility title.",
        "Map turns the corner map on and off.",
        "Shift Screen: with the box ticked, hold Alt and move the mouse to an edge to move the camera.",
      ],
    },
  },
  {
    slug: "nobreza-e-titulos-top",
    date: "2026-10-07",
    category: "system",
    pt: {
      title: "Nobreza e títulos Top",
      summary: "O ranking de Nobreza do original, de King a Knight, e o anel dourado dos melhores de cada profissão.",
      body: [
        "Doe prata (ou CPs) e suba entre os nobres. Os 3 primeiros são King, até o 15º Prince e até o 50º Duke; depois vêm Earl, Baron e Knight pela doação.",
        "Os melhores de cada profissão ganham o título Top e o anel que gira em volta do herói.",
        "O login agora lista os mundos e lembra as contas salvas, e o cliente se atualiza sozinho.",
      ],
    },
    en: {
      title: "Nobility and Top titles",
      summary: "The original's Nobility ranking, from King to Knight, and the golden ring of each profession's best.",
      body: [
        "Donate silver (or CPs) and rise among the nobles. The first 3 are Kings, up to 15th Princes and up to 50th Dukes; then Earl, Baron and Knight by donation.",
        "Each profession's best earn the Top title and the ring circling the hero.",
        "The login now lists the worlds and remembers saved accounts, and the client updates itself.",
      ],
    },
  },
  {
    slug: "itens-como-no-original",
    date: "2026-10-07",
    category: "update",
    pt: {
      title: "Itens e mortes como no original",
      summary: "O brilho das armas Super, skins, flechas, números de dano e monstros que voam longe com o golpe final.",
      body: [
        "Armas Super brilham como no 5065, e as skins de arma aparecem no herói.",
        "Os números de dano e a aljava com as flechas restantes seguem a arte original.",
        "Um golpe muito acima da vida do monstro o arremessa para trás antes de cair.",
      ],
    },
    en: {
      title: "Items and deaths as in the original",
      summary: "Super weapon glows, skins, arrows, damage numbers and monsters flung away by the killing blow.",
      body: [
        "Super weapons glow as they did in 5065, and weapon skins show on the hero.",
        "Damage numbers and the quiver with the arrows left follow the original art.",
        "A blow far past a monster's life flings it back before it falls.",
      ],
    },
  },
  {
    slug: "dialogos-dos-npcs",
    date: "2026-10-06",
    category: "interface",
    pt: {
      title: "Diálogos dos NPCs",
      summary: "A janela de conversa do cliente original, nomes em verde sobre os NPCs e o martelo de reparo.",
      body: [
        "As conversas com os NPCs abrem na moldura original, com as respostas em cartões.",
        "Os nomes dos NPCs aparecem em verde sobre a cabeça.",
        "Na loja, o martelo repara itens com um clique.",
      ],
    },
    en: {
      title: "NPC dialogs",
      summary: "The original client's dialog window, green names over NPCs and the repair hammer.",
      body: [
        "Talking to NPCs opens the original frame, with the answers as cards.",
        "NPC names show in green over their heads.",
        "At the shop, the hammer repairs items with a click.",
      ],
    },
  },
];

export const findPost = (slug: string) => posts.find((post) => post.slug === slug);
