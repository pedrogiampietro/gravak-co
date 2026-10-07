# Gravak — site

Next.js 16 (App Router, Cache Components), Tailwind 4 e shadcn/ui. Português em `/pt`, inglês em `/en`
(o `src/proxy.ts` escolhe pelo idioma do navegador).

- **Início**: Twin City desenhada pelo nosso cliente, quem lidera o ranking, os números do servidor,
  as quatro profissões, os sistemas, a agenda e os mapas.
- **Ranking** (`/ranking`): as mesmas consultas dos Highscores do jogo (`server/.../highscores.rs`),
  com filtro por profissão; a Nobreza mostra o título como `nobility::rank_at`.
- **Personagem** (`/characters/<nome>`, busca em `/characters?name=`): profissão, nível, guilda, cônjuge,
  nobreza, PK, equipamento com os ícones do jogo, experiência dos últimos 7 dias, mortes e vítimas.
- **Powergamers** (`/powergamers`): quem mais ganhou experiência em 1, 2, 3 ou 7 dias.
- **Killers** (`/killers`): quem mais abateu jogadores em 24 h, 7 dias, 30 dias ou sempre, e as últimas mortes.
- **Guildas** (`/guilds`, `/guilds/<nome>`): a lista e cada guilda com seus membros.
- O servidor alimenta essas páginas (migração 77): `last_seen` e `experience_days` a cada 30 s
  para quem está online, e `character_deaths` a cada morte fora da arena e dos eventos.
- Os nomes e ícones dos itens (`src/content/items.json`, `public/items`) saem do cliente:
  `cargo test --release -p conquer-sandbox site_items -- --ignored`.
- **Criar conta** (`/register`): grava em `accounts` com bcrypt de custo 12, como o `Account::create`
  do servidor; o login do jogo aceita a senha.
- **Download** (`/download`): o instalador de `package-client.bat`.
- **Notícias** (`/news`): escritas em `src/content/news.ts`, em português e inglês; as 5 mais novas
  aparecem no quadro da página inicial.
- **Barra do topo**: Discord e Instagram (`NEXT_PUBLIC_DISCORD_URL`, `NEXT_PUBLIC_INSTAGRAM_URL`; somem
  enquanto vazios), Download, quantos estão online e o idioma. O servidor grava os online em
  `server_status` a cada 30 s (migração 76); sem a tabela, o site só testa as portas 9958 e 5816.
- **Agenda**: contagem para a próxima guerra de guildas no relógio do servidor (`GUILD_WAR_UTC_OFFSET`).

## Rodar

```bash
cd web
cp .env.example .env.local
npm install
npm run dev
```

O banco é o Postgres do servidor (`server/docker-compose.yml`). Publique o `GravakSetup.exe` em
`web/public/downloads/` ou aponte `NEXT_PUBLIC_DOWNLOAD_URL` para onde ele estiver.

## Imagens

As de `public/art` saem do próprio cliente (mapas e heróis em 3D):

```bash
cd client
cargo test --release -p conquer-sandbox site_art -- --ignored
```
