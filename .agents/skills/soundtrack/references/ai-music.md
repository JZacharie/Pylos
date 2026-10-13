# AI-generated music (optional)

Use when the user wants something richer than a synthesized score. Needs an API key.

## Lyria 3 via OpenRouter

Audio output requires `stream: true`; chunks arrive base64-encoded in `delta.audio.data`.

```bash
curl -s -N https://openrouter.ai/api/v1/chat/completions \
 -H "Authorization: Bearer $OPENROUTER_API_KEY" -H "Content-Type: application/json" \
 -d '{"model":"google/lyria-3-clip-preview","messages":[{"role":"user","content":"gentle lullaby, music box and soft piano, 80 bpm, warm"}],"modalities":["audio"],"stream":true}' \
 | python3 -c "import sys,json,base64;b=''.join(json.loads(l[5:]).get('choices',[{}])[0].get('delta',{}).get('audio',{}).get('data','') for l in sys.stdin if l.startswith('data:') and 'DONE' not in l);open('bgm.mp3','wb').write(base64.b64decode(b))"
```

Put the tempo in the prompt (e.g. "80 bpm") so the beat grid is predictable.

## Mix it under the picture

```bash
ffmpeg -y -i piece.mp4 -i bgm.mp3 \
 -filter_complex "[1:a]atrim=0:DUR,afade=t=in:st=0:d=1,afade=t=out:st=DUR-1.5:d=1.5,volume=0.8[a]" \
 -map 0:v -map "[a]" -c:v copy -c:a aac -shortest piece_music.mp4
```

(Replace `DUR` with the length in seconds.) Leave `window.SCORE` undefined in the page, or render.mjs will mux its own score as well.

## Then align the picture to it

Run `node scripts/sync-check.mjs piece_music.mp4`, read the offset for each cut, move the boundaries in the page's `SCENES` table by those amounts, re-render and re-check. The music can't move; the drawing can.
