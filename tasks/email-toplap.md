# Email to TOPLAP — draft (2026-08-11, not sent)

Goal: offer the live coding community an early look at LOOM, plus a challenge aimed at
people who have never live coded. Honest about the current state: the browser prototype
runs on a JS engine today, the Rust core is what makes the three surfaces agree.

Where to send: toplap.org lists the community channels (mailing list, forum, Discord).
Pick one, do not blast all three. The forum is the least intrusive first move.

---

**Subject:** LOOM, a small live coding language aimed at people who have never coded a beat

Hi TOPLAP,

I have been building LOOM, a live coding music language that runs in the browser and on
a phone, with a terminal client on the way.

The whole language fits in a paragraph. One voice per line. `x` and `.` for drums, note
names for melody, and a handful of verbs: `rev`, `fast N`, `slow N`, `every N <verb>`.

```
kick:  x . . . x . . .
snare: . . x . . . x .
hat:   x x x x x x x x | every 4 fast 2
bass:  c2 . eb2 . g2 . eb2 . | saw | every 8 rev
```

Where it actually stands: the browser prototype works today on a JavaScript engine. The
Rust core, `loom-core`, is zero dependency and fully tested, and it is being swapped in
so the browser, the phone and the terminal behave identically instead of drifting apart.
That port is the current work, not a finished claim.

Two things I would like to offer the community.

First, an early look before anything public, and I would rather have it torn apart here
than praised elsewhere. Blind spots from people who have been doing this for years are
worth more to me than users right now.

Second, a challenge I would like to run with you: **make your first beat in 30 minutes**,
for people who have never live coded. Not a competition between experts. The point is to
find out where a complete beginner gets stuck in the first half hour, which is the part
most tools never measure.

Is there a channel, a thread or an algorave slot where either of these would be welcome?
Happy to take a no, or to be told the idea needs reshaping first.

Munay Passaqui
Lyon, France

---

## Notes before sending

- Attach nothing. A link to a running page beats a README.
- If a demo link is required, make sure the shared browser build actually loads on a
  phone first. Claiming "runs on your phone" and shipping a link that breaks on mobile
  would sink the introduction.
- Do not mention Mousikê here. Different audience, different project.
