## Menu do item igual aos NPCs, tooltips com requisitos, $go

### Servidor
- **Menu do item = NPCs** (`forge.rs`), para item na bolsa ou equipado:
  - QUALITY → MagicArtisan: `1 + 100/chance` DragonBalls, nunca falha
  - LEVEL (novo, MsgItem 44) → MagicArtisan até nível 120 (Meteors), WeaponMaster acima (1 DragonBall)
  - UPGRADE → WuxingOven: dois itens do mesmo +n até +9; depois 12 DB, 25 DB, 4 DBScrolls
  - GEM SOCKET → OPEN → GodlyArtisan: arma 1/5 DB; outras peças 12 DB e depois 7 StarDrills ou ToughDrill (12%)
  - O WuxingOven lê o preço do +9→+12 da mesma função do menu.
- **+n Stones** nascem com o +n do nome (antes eram +0 e não compunham); a migração 74 conserta as que já existem.
- **Proficiência por tipo de arma** (410 = todas as blades), como no original: antes, a SunBlade não equipava mesmo após o ProficiencyGod.
- Comandos de GM: `$proficiency 410 12`, `$go <NPC, monstro ou jogador>`.
- Testes: `forge_journey` (Normal → Super +12, 2 gemas, HP, bless, pelos NPCs) e `menu_journey` (o menu cobra o mesmo que os NPCs).
- Guia: `docs/guia-forja.md`.

### Cliente
- Tooltip: requisitos (nível, proficiência, atributos, sexo) em vermelho quando faltam; qualidade no nome (Super SunBlade, Super DragonGem); efeito da gema sob o socket ("Attack +15%"); descrição de gemas e pedras; Attack Rate só em armas.
- Painéis do menu mostram o custo dos NPCs e o que você tem; nova ação LEVEL.
- A bolsa e as janelas não esperam mais redesenhos atrasados: o item jogado no chão sai da bolsa na hora.

### Como testar
1. Reiniciar o servidor (migração 74).
2. `$level 130`, `$item 410205`, `$proficiency 410 12`, equipar.
3. Shift + botão direito no item: QUALITY / LEVEL / UPGRADE / GEM SOCKET, conferindo o custo no painel.
4. `$go WuxingOven`.

Testes: servidor 271, cliente 124.

🤖 Generated with [Claude Code](https://claude.com/claude-code)
