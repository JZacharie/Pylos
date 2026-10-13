# Characters (only when one appears in more than one scene)

## Design it once, alone, before the scenes

Render the character by itself in every expression and pose the storyboard needs (e.g. eyes open / asleep / smiling / crying; mouths; the 2-3 poses). Look at that one sheet and fix it there. A character problem found inside the full film costs a whole re-render per attempt; found on the sheet it costs one still.

To render it alone, add a temporary first shot that draws only the character in a grid of expressions and poses, and take a still of it (`--stills 0`); remove the shot afterwards. This is a self-check. Don't stop to ask the user to approve it.

Write the character as one function with options, e.g. `hero(cx, cy, r, { eyes, mouth, lookX, lookY, rot, sway })`, so every scene draws the same person.

## Anchor points

Expose the places props attach to (mouth, hands, feet) as functions of the character's position, size and rotation, e.g. `mouthOf(cx, cy, r, rot)`. Place bottles, cards and toys from those anchors. Hard-coded coordinates drift the moment the character moves, and the bottle ends up nowhere near the mouth.

## From photos to a character (only if the user gave photos)

1. List the 3-5 features that make this person *them*: for a baby it might be hair that sticks straight up, very full cheeks, a particular printed onesie, a tiny fist. Ignore everything else.
2. Exaggerate those features and simplify the rest. A likeness comes from the signature features, not from accuracy.
3. For babies and cute characters: big head, features set low on the face, small widely spaced eyes with a highlight, soft blush. Features high on the face read as older.
4. Pull real colors and objects from the photos: the actual stroller, bathtub, blanket print, room colors. These make the piece feel like *theirs* more than facial accuracy does.
5. Stay clearly stylized. A near-realistic drawing of someone the viewer knows well reads as uncanny; a cartoon doesn't.
6. Photos are for looking at locally. Never upload them, embed them, or put them in anything published.

Grown-ups next to a baby (a family portrait): tell them apart by hair (long, tied, bob, short, buzz), glasses, clothes color and what they hold, rather than facial detail.

## Poses that read

- Pick the camera angle that makes the pose unmistakable. Tummy time from the side is a long blob with stubby arms (it reads as a caterpillar); from the front it's a head, shoulders, two forearms on the mat and feet peeking up behind.
- Every head sits on a neck or shoulders. A gap between head and body reads as a floating head.
- Hands: rounded mitt + thumb, never a polygon.
- Draw back to front: back hair, body, neck, face, front hair, then anything held in front.
- Hair: a few curved, tapering strands of different lengths leaning together read as hair; evenly spaced identical spikes read as a crown or a sea urchin.
