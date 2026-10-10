# Covil dos Goblins: prompt para o Gemini (mapa)

Para o **Gemini** gerar a arte do mapa: o conceito, os pisos, as paredes
altas e os enfeites. Cole o prompt abaixo inteiro numa conversa nova. Ele
pede **uma imagem por vez**: depois de cada uma, responda "next" (ou
"próxima") até terminar. Salve cada imagem com o nome que ele disser, em
`content/art/fungos/`.

Se o Gemini juntar várias peças numa imagem só, peça de novo aquela peça
sozinha ("generate only the image N again, alone").

---

```
You are making the art for an underground goblin den in an isometric 2:1 fantasy MMORPG in the hand-painted style of Conquer Online (early 2000s Asian MMO look): damp packed brown earth, embedded grey rocks, roots and moss, glowing blue cave mushrooms, crude goblin wood and scrap, warm torchlight against cold blue mushroom light.

Generate the images below ONE AT A TIME, one image per message, in this order. Each is a separate file: never put two items in the same image. Every image is 1024x1024. Keep exactly the same style, palette and lighting in all of them, using image 1 as the style reference. After each image, write its file name and wait for me to say "next".

TEXTURES (seamless and tileable unless said otherwise, even lighting, no perspective, no objects, no shadows cast by objects):

1. conceito.png: isometric 2:1 view of a narrow goblin den: earth floor with roots and moss, very tall rough earth and rock walls forming corridors about 3 meters wide, glowing blue mushrooms, hanging lanterns, a wooden barricade, a campfire, sacks of loot, a bone totem; moody; this one is a scene, not a texture.
2. chao-1.png: straight top-down view of a damp cave floor of packed brown earth with small stones, roots and patches of moss.
3. chao-2.png: straight top-down view of the same earth floor with puddles of dark water and scattered small glowing blue mushrooms.
4. chao-3.png: straight top-down view of the same earth floor with gnawed bones, broken pottery shards and straw.
5. parede-face.png: front view of ONE section of a very tall rough cave wall, shown whole from its top edge down to its base on the ground: brown earth and embedded grey rocks, hanging roots near the top, damp dark soil and moss at the base. Tileable horizontally only (the top and the base are different).
6. parede-topo.png: straight top-down view looking down at the flat top of a thick cave wall: dark earth and rock with grass tufts and moss; no wall face visible, no edges.

PROPS (single isometric game asset, 2:1 isometric angle seen from above at about 30 degrees, light from the top-left, centered, the whole object visible, plain pure white background, no ground, no shadow on the background):

7. cogumelos.png: a cluster of glowing blue cave mushrooms of different sizes.
8. cogumelo-gigante.png: a single giant glowing blue mushroom taller than a man.
9. fogueira.png: a small goblin campfire with stones around it and a skewer of meat.
10. barricada.png: a crude wooden barricade of sharpened stakes tied with rope.
11. totem.png: a goblin totem pole of wood topped with an animal skull and feathers.
12. sacos.png: a pile of stolen loot sacks with coins spilling out.
13. bau.png: a small crude wooden chest held together with rope and nails.
14. ossos.png: a small pile of gnawed animal bones.
15. barril.png: two old wooden barrels, one broken.
16. pedras.png: a pile of fallen rocks and rubble.

WALL PROPS (same rules as the props, but seen from the FRONT, as if hanging on a wall):

17. lanterna.png: a crude iron lantern with a warm flame, hanging from a hook.
18. tocha.png: a wooden wall torch wrapped in cloth with a warm flame, in an iron holder.
19. estandarte.png: a torn goblin war banner of red cloth with a crude skull symbol, hanging from a wooden pole.
20. raizes.png: a curtain of hanging roots and vines.

Start with image 1.
```
