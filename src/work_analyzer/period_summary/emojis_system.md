Memory cues for the covered period:

Return these three additional JSON fields alongside the recap text:

- `activity_emoji`: a single emoji representing the period's most distinctive activity, event, or recurring theme. Prefer something concrete and memorable over a generic productivity symbol. Use `null` when there is no meaningful activity cue.
- `mood_emoji`: a single emoji representing the emotional tone actually expressed in the current period's traces. Prefer the representative emotional thread, not automatically the strongest or most negative moment. Use `null` when the mood is unclear, insufficiently supported, or too mixed for one faithful cue.
- `dominant_emoji_kind`: `"activity"`, `"mood"`, or `null`. Choose which of the two cues would best help the writer remember this period at a glance. Favor activity when a distinctive event or theme anchors the period; favor mood when a clearly expressed emotional thread is more characteristic. If both are equally representative, prefer activity. If only one emoji is available, choose that kind. Return `null` only when both emojis are `null`. Never select a kind whose emoji is null.

Ground both emoji choices and the dominant choice in the covered period's evidence. Earlier summaries provide continuity, not the current period's activity or mood. Distinguish the writer's feelings from those attributed to other people, fictional characters, imagined situations, or literary scenes. Do not infer mood simply from an activity: working does not necessarily imply stress, and a trip does not necessarily imply happiness. Do not invent feelings or turn the emoji into an emotional diagnosis.

Each emoji field must contain just one emoji (including a composed emoji where appropriate), without words, explanations, or several adjacent emojis. Keep the activity and mood fields independent; they need not convey the same tone.
