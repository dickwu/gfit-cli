# Email templates

Three tiers, escalating in warmth and lowering in pressure as a client stays quiet longer.
Pick the one that matches the client's bucket (see `../references/classification.md`).

**How to use these**

- Fill the placeholders: `{{first_name}}`, `{{coach_name}}` (default **Grant**),
  `{{checkin_url}}` (default **https://www.gfitwellness.ca/weekly-check-in**).
- Create **one Gmail draft per client** — `To:` the client, the `Subject:` line below, and
  the body. **Never send.**
- These are starting points written in the coach's voice. Keep them personal; tweak a line
  for a specific client when you know something about them. Don't make them sound corporate.

---

## Tier 1 — Missed this week (8–14 days)

A light, encouraging nudge. Assume the best; just make it easy to check in.

**Subject:** Checking in — how's your week going?

```
Hi {{first_name}},

I hope you're having a great week! I just wanted to check in and see how everything is
going with your training and nutrition.

Whenever you get a chance, please send me a quick update here: {{checkin_url}}

I'm always here to support you with your health and fitness goals.

Sincerely,
{{coach_name}}
```

---

## Tier 2 — Missed a few weeks (15–28 days)

You've noticed they've been quiet for a couple of weeks. Show you care and offer to clear a
blocker — without any guilt.

**Subject:** Thinking of you — let's get your check-in back on track

```
Hi {{first_name}},

I noticed it's been a couple of weeks since your last check-in, and I wanted to reach out
to make sure everything's okay.

Life gets busy — that's completely understandable — but I don't want you to lose the
momentum you've worked hard to build. Whenever you have a few minutes, send me an update so
we can keep your plan working for you: {{checkin_url}}

And if something's been getting in the way of your training or nutrition, just reply to this
email and tell me — I'm here to help you work through it.

Talk soon,
{{coach_name}}
```

---

## Tier 3 — Long disengaged / never checked in (28+ days)

A genuine, low-pressure reconnect. The goal is to reopen the door, not to chase. Make even a
one-line reply feel welcome.

**Subject:** I'd love to reconnect, {{first_name}}

```
Hi {{first_name}},

It's been a while since I've heard from you, and I genuinely want to make sure you're doing
well. I don't want to lose touch — your health and your goals still matter to me.

If you're ready to pick things back up, I'd love to help you ease back in. You can send a
quick check-in here whenever you're ready: {{checkin_url}}

And if now isn't the right time, just reply and let me know what's going on — no pressure at
all. Even a one-line reply helps me understand how to best support you.

Hope to hear from you soon,
{{coach_name}}
```
