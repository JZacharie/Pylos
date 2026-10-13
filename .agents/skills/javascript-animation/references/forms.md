# Forms

Infer the form from the brief; don't ask unless nothing points anywhere. These are structure notes, not templates: adapt them to the subject.

## Story (a character, something happens)

- 2-5 s per shot, grouped into a few scenes. One beat per shot.
- A title beat (2-4 s), a middle that changes something, an ending that lands and **holds** (let motion stop before the last line appears).
- Wordless works (Kevin Ngo's watermelon piece has no text). If there are captions, keep them short and bilingual only if the audience is.
- A recurring character carries the film: see `character.md`.
- One specific, human detail beats general adjectives (the name on a mug, the one sock that never matches).

## Explainer (make a mechanism clear)

- Open on the question (or a cloud of questions), not the answer.
- One mechanism step per scene, numbered if the audience needs a map.
- Show causality, not labels: things fall, fill, count up, fade away, because of the previous thing. (In the newborn-vision example, photons land on cones and a counter shows how much light each eye catches.)
- An on-screen gauge or counter that persists across scenes turns a sequence into a chain.
- End with a one-line recap of the chain.
- Every fact on screen needs a source you can name. Put the sources next to the piece (e.g. a README).

## Poem / ambient

- No plot; a mood and 2-3 images that transform into each other.
- Slower: 6-10 s per image, long crossfades, very little text.

## Loop

- The last frame must equal the first. Build every motion from `Math.sin(2πt / DUR)` (or anything periodic in DUR) so the seam is invisible.
- Check the seam: render stills of frame 0 and frame FRAMES-1 side by side.

## Interactive page (not a video)

- Same drawing code, but deliver the HTML: keep the preview loop, add hover/click where it helps.
- Still keep `draw(frame)` pure so a preview video can be rendered for sharing.
