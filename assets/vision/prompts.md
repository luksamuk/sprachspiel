# Vision Test Prompts

Test prompts for Vision functionality, organized by test case.

## Single Image Description

### Character Identification (protagonist.jpg, protagonist2.jpg)
- `Descreva este personagem e identifique de qual jogo ele é.`
- `Who is this character and what game is this from?`
- `What artistic style is used in this cover art?`

## Multi-Image Comparison

### Protagonist Comparison (protagonist.jpg + protagonist2.jpg)
```
/vision assets/vision/protagonist.jpg,assets/vision/protagonist2.jpg Compare these two game cover arts. They are from the same franchise but different protagonists. Identify both characters, their respective games, and analyze the artistic choices.
```

**Expected answer:** Alucard (Castlevania: Symphony of the Night) and Soma Cruz (Castlevania: Aria of Sorrow). Artistic differences: color palette, character design era, mood/atmosphere.

## Handwritten Notes OCR (manuscrito01.jpg, manuscrito02.jpg)

- `Extraia o texto manuscrito desta imagem com o máximo de fidelidade possível.`
- `Use OCR and vision capabilities to extract the handwritten text from this image. Preserve the semantic meaning.`
- `Transcreva todo o texto contido nestas notas manuscritas.`

**Challenge:** These are real handwritten notes in Portuguese. The model should extract what it can and admit uncertainty for illegible parts rather than fabricating text.

## Scene Description (manga.jpg)

- `Descreva a cena mostrada neste painel de manga.`
- `What is happening in this manga panel? Describe the characters, action, and dialogue (if visible).`

## Regression Notes

- Vision uses `/api/generate` endpoint (same as OCR)
- Multi-image support uses comma-separated paths: `/vision img1.jpg,img2.jpg Compare both`
- Vision system prompt: `"You are a vision model. Analyze the image as instructed. Describe what you see thoroughly and accurately. Output only your analysis."`
- Single image is backward compatible — no comma needed