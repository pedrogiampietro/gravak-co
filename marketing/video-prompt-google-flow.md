# Gravak: anúncio em vídeo para o Google Flow (Veo)

Formato: 30 s, vertical 9:16 (Reels/Stories/Shorts). Para o feed, gere também em 16:9.
O Flow gera clipes de 8 s. São 4 cenas encadeadas com **Extend** ou montadas no **Scenebuilder**.

## Como usar as imagens desta pasta

- **Cena 1:** use `08-o-mundo.png` como *start frame* (Frames to Video).
- **Cena 2:** use `07-profissoes.png` como *ingredient* (Ingredients to Video) para manter os quatro heróis consistentes.
- **Cena 3:** use `03-trojan.png` como *start frame*.
- **Cena 4:** use `01-logo.png` como *end frame*, para o vídeo terminar exatamente na logo.

Se preferir gerar tudo do zero, cole só o **prompt mestre** e depois os prompts de cada cena.

---

## Prompt mestre (estilo, vale para todas as cenas)

```
Cinematic trailer for "GRAVAK", a classic 2005-era isometric fantasy MMORPG with a
Chinese wuxia flavour. Visual style: hand-painted 2.5D isometric world (stone plazas,
tiled roofs, willow trees, rivers with lily pads, misty mountains), characters in
ornate gold armour with oversized blades, wings and flowing capes. Night-time colour
grade: deep teal shadows, warm gold highlights, drifting embers and soft volumetric
light. Camera moves are slow, smooth and epic. Typography is a gold serif with a soft
glow (like Cinzel). No modern UI, no real-world brands, no on-screen text except where
asked. Audio: orchestral score with Chinese instruments (erhu, guzheng, taiko drums),
deep cinematic impacts. Voice-over in Brazilian Portuguese, male, deep and calm.
```

---

## Cena 1 (0–8 s): o mundo

```
Slow aerial push-in over an isometric fantasy city at dusk: stone bridges over a teal
river, wooden houses with dark tiled roofs, reeds swaying in the wind, lanterns
lighting up one by one. Golden embers drift upward. The camera glides forward and
slightly down, revealing a grand stone plaza. Soft fog rolls between the buildings.
Audio: wind, distant bells, a single erhu note that swells into strings.
Voice-over (pt-BR): "Você lembra de onde tudo começou?"
```

## Cena 2 (8–16 s): as profissões

```
Four heroes stand side by side on a stone plaza under moonlight, facing the camera:
a Trojan with twin long blades and gold armour, a Warrior in winged golden plate
with a shield, an Archer with white feathered wings and a bow, a Taoist in an orange
robe with a thin sword. The camera tracks slowly from left to right; as it passes
each hero, they strike a short heroic pose and their weapon glows. Capes and hair
move in the wind, embers float around them.
Audio: taiko drum hits on each pose, the music builds.
Voice-over (pt-BR): "Quatro caminhos. Uma lenda."
```

## Cena 3 (16–24 s): ação

```
Dynamic combat montage in the isometric world: the Trojan dashes forward and spins
with both blades, leaving golden arcs of light; the Archer leaps and fires a volley
of glowing arrows; the Taoist raises a hand and lightning strikes from the clouds; the
Warrior slams his shield into the ground, sending out a golden shockwave. Quick cuts,
slow-motion on each impact, sparks and embers everywhere. Colour stays teal and gold.
Audio: whooshes, metal clashes, thunder, the score at full power.
Voice-over (pt-BR): "Monte seu time. Domine Twin City."
```

## Cena 4 (24–30 s): logo

```
The camera rises above the city into the night sky; embers gather and swirl into
the centre of the frame and form the word "GRAVAK" in an ornate gold serif with a
warm glow, with a thin gold ornamental line beneath it and the small text
"CLASSIC · 5065". A sweep of light passes across the letters. Background: deep teal
gradient with a faint diamond lattice and slow drifting embers. Hold on the logo.
Audio: final drum hit, the music resolves on a low sustained note.
Voice-over (pt-BR): "Gravak. Em breve."
```

---

## Variações curtas (15 s, para teste A/B)

**A, nostalgia:**
```
A slow push-in on a classic isometric fantasy plaza at night, embers drifting, then
the camera tilts up to the sky where the gold title "GRAVAK" forms from embers.
Voice-over (pt-BR): "O clássico voltou. Do jeito que você lembra." Orchestral,
erhu lead, warm and emotional.
```

**B, hype:**
```
Rapid cuts of four golden-armoured heroes fighting in an isometric fantasy city:
twin blades, a volley of arrows, a lightning spell, a shield shockwave. Hard cut to
black, then the gold title "GRAVAK" slams in with a taiko hit.
Voice-over (pt-BR): "Escolha seu lado. Gravak, em breve."
```

---

## Dicas no Flow

- Gere 2–4 variações de cada cena e escolha a melhor antes de usar o **Extend**.
- Se o texto da logo sair errado (é comum vir com letras trocadas), gere a cena 4 sem texto ("embers swirl into the centre and burst into light") e sobreponha o `01-logo.png` na edição.
- A narração pode sair mais natural gravada à parte (ou no ElevenLabs) e mixada por cima. Nesse caso, apague as linhas "Voice-over" dos prompts.
- Legenda sugerida para o post: "O clássico voltou. Escolha sua profissão e prepare-se: Gravak, em breve. ⚔️🏹🛡️🔥"
