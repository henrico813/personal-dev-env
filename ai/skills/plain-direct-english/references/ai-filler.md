# AI filler

Each kind of filler that the skill cuts, with a bad and a better
version. The skill's checker, `scripts/check_sentences.py`, finds the
filler words, hedges, closing offers, praise, emojis and banned
words, with the line of each. It skips code blocks and text in
backticks. It cannot find a restated question or a summary of a
summary, so look for those yourself.

- Filler words, such as "comprehensive", "robust", "seamless",
  "leverage" and "ensure". Bad: "A robust fix to ensure seamless
  resumes." Better: "Resuming skips files already copied."
- Hedges. Say what you checked and what you did not. Bad: "This
  should probably fix it, I think." Better: "The new test passes. I
  did not run it on Windows."
- Restating the question. Bad: "You asked what batch 2 is. Batch 2
  is ..." Better: "Batch 2 is ..."
- "Great question" and other praise. Bad: "Great question! Batch 2
  ..." Better: "Batch 2 ..."
- Closing offers. Bad: "Let me know if you want any changes!"
  Better: end on the last fact or the next step.
- Summaries of summaries. Bad: "In summary, two PRs are ready" after
  the list that says so. Better: cut it.
- Emojis, including check marks and warning signs. Bad: a check-mark
  emoji before "Merged". Better: "Merged".
- The words "canonical" and "contract". Bad: "the canonical config",
  "the API contract". Better: "the main config", "what the API
  accepts".
