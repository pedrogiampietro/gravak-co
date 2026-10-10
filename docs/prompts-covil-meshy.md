# Covil dos Goblins: prompts para o Meshy (monstros)

Para o **Meshy** gerar as 5 criaturas já em 3D, com rig e animações. **Uma
conversa por criatura**: cole o prompt dela no Agente do Meshy. Ele gera o
modelo a partir do texto; se você tiver um conceito (imagem) da criatura,
anexe também, que ele fica mais fiel.

Antes de pedir o rig, gire o modelo e confira mãos, pés e costas; se algo
saiu ruim, peça "regenerate, keep the hands and feet complete".

Baixe e salve em `content/art/monstros/<pasta>/`:

- `walk.glb` (obrigatório: o modelo, o rig, a textura e o andar);
- `idle.glb`, `attack.glb`, `death.glb` se o Meshy exportar cada animação num
  arquivo; `run.glb` se vier.

---

## 1. Goblin saqueador — pasta `goblin-saqueador`

```
Create a game-ready 3D monster for an isometric fantasy MMORPG in a hand-painted style like Conquer Online: a small scrawny goblin scavenger with green skin, big pointed ears, a rusty dagger in the right hand and patched leather rags. Two arms and two legs, standing in an A-pose, arms slightly away from the body. Low poly (3,000 to 5,000 triangles), one single base-color texture (no PBR, no normal map). Then auto-rig it as a humanoid and add these animations: idle, walk, attack (a quick dagger stab), hit, death. Export as GLB with all animations.
```

## 2. Goblin xamã — pasta `goblin-xama`

```
Create a game-ready 3D monster for an isometric fantasy MMORPG in a hand-painted style like Conquer Online: a goblin shaman with green skin, a bone mask, feathers, a tattered cloak down to the knees and a wooden staff topped with a glowing blue mushroom in the right hand. Two arms and two legs, standing in an A-pose, arms slightly away from the body. Low poly (3,000 to 5,000 triangles), one single base-color texture (no PBR, no normal map). Then auto-rig it as a humanoid and add these animations: idle, walk, attack (casting a spell with the staff pointed forward), hit, death. Export as GLB with all animations.
```

## 3. Homem-rato — pasta `homem-rato`

```
Create a game-ready 3D monster for an isometric fantasy MMORPG in a hand-painted style like Conquer Online: a hunched ratman with grey fur, a rat head with long teeth, a long tail, a torn leather vest and a crude wooden club in the right hand. Two arms and two legs, standing in an A-pose, arms slightly away from the body. Low poly (3,000 to 5,000 triangles), one single base-color texture (no PBR, no normal map). Then auto-rig it as a humanoid and add these animations: idle, walk, attack (a club swing), hit, death. Export as GLB with all animations.
```

## 4. Cogumelo andante — pasta `cogumelo-andante`

```
Create a game-ready 3D monster for an isometric fantasy MMORPG in a hand-painted style like Conquer Online: a walking mushroom creature with a wide red cap with white spots, a stubby pale body, small angry eyes, short arms and short legs. Two arms and two legs, standing in an A-pose, arms slightly away from the body. Low poly (3,000 to 5,000 triangles), one single base-color texture (no PBR, no normal map). Then auto-rig it as a humanoid and add these animations: idle, walk, attack (a headbutt with its cap), hit, death. Export as GLB with all animations.
```

## 5. Chefe goblin (Guardião) — pasta `chefe-goblin`

```
Create a game-ready 3D boss monster for an isometric fantasy MMORPG in a hand-painted style like Conquer Online: a huge muscular goblin warlord with green skin, armor made of scrap metal, pots and dented shields, a crown of bones and a giant spiked wooden club in the right hand. Two arms and two legs, standing in an A-pose, arms slightly away from the body. Low poly (4,000 to 6,000 triangles), one single base-color texture (no PBR, no normal map). Then auto-rig it as a humanoid and add these animations: idle, walk, attack (a heavy overhead club smash), hit, death. Export as GLB with all animations.
```
