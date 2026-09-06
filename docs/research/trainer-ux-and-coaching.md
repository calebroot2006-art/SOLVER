---
type: research
status: draft
date: 2026-09-05
---

# Trainer UX and the coach

Researched 2026-09-05 by the `researcher` subagent, reviewed by the main session.

## Question

How do the best current poker trainers and play modes teach, what makes them fun or not,
and how is AI-coach explanation done today? Input to designing the "coach in your corner".

## Answer

Every serious trainer grades against a solver using EV-loss buckets (Best, Correct,
Inaccuracy, Mistake, Blunder) rather than right or wrong, and most tolerate mixed
strategies with an "RNG mode". Natural-language explanations exist (GTO Wizard AI, DTO's
Virtual Coach, Octopi's Ask George) but none publishes its architecture, and GTO Wizard's
own blog lists human-language explanation as a goal rather than a documented shipped
mechanism. Almost all coaching is post-hand or post-session review, not mid-hand. Real
in-hand advice exists only in gray-area tools like Odin, which became a real-time
assistance controversy and is not a design reference. Chess.com's Game Review with a
coach avatar is the closest analogue to the "little guy", and it draws real criticism
(forced avatar, accuracy score misread as skill, template-feeling explanations) that we
should design around.

Correction to the brief I wrote: Octopi's assistant is "Ask George". "Odin" is an
unrelated product by Rory Young. My mistake, not Caleb's.

## Findings

### Feature comparison

* **GTO Wizard.** Trainer grades every action Best, Correct, Inaccuracy, Wrong, or Blunder
  against solver frequency; "RNG mode" pre-rolls the mixed-strategy split so a 60/40 spot
  can be graded fairly on one click; reports total EV loss in bb, average per hand, and
  loss as percent of pot. Formats: cash 2-max to 9-max, MTT ICM 3 to 9 players at 15 to
  60bb, spin 3-max, heads-up SNG.
  Sources: https://help.gtowizard.com/measure-performance/ ,
  https://help.gtowizard.com/how-to-use-the-trainer/ ,
  https://blog.gtowizard.com/kick-off-2025-with-new-cash-and-icm-solutions/
* **GTOBase.** Pre-solved viewer and hand-history analyser for all 22,100 flops; no play
  or drill mode. $150 a month to $1,200 a year. The "study tool with no game" failure
  mode Caleb wants to avoid. Source: https://gtobase.com/
* **DTO Poker.** Play against solved opponents, instant grading, a "Virtual Coach" giving
  plain-language reasoning, large MTT and ICM library. Sources: https://www.dtopoker.com/
  and https://www.dtopoker.com/tournament
* **GTO Gecko.** Preflop, street-by-street, and full-hand trainers, "SHAP-based
  plain-English explanations", leak dashboards by position and board texture. $25 to $40
  a month. Vendor source only. Source: https://gtogecko.com/
* **Octopi Poker.** Trainer with instant feedback and a "perfect streak" chase; assistant
  named Ask George. Sources: https://cardplayerlifestyle.com/poker-courses/octopi-poker-review/
  and https://pokerjudge.com/blog/poker-products/octopi-poker/
* **PokerCoaching.com.** 400+ human-written quiz hands with explanations; human-authored,
  not solver-graded. Source: https://pokercoaching.com/
* **RangeTrainerPro.** Range memorisation from Monker and Pio solves.
  Source: https://legacy.rangetrainerpro.com/
* **Preflop Academy.** 1.1M+ preflop charts, daily streak, free tier of 10 hands a day.
  Source: https://poker.academy/blog/post/discover-preflop-academy-mobile-app-today
* **PokerSnowie.** Play "Challenges" against the AI and get an error-rate score over
  time. An error browser highlights mistakes and shows advice on click. Graphs split by
  street and session type. The clearest per-street leak with drill-back pattern found.
  Sources: https://www.gipsyteam.com/news/18-01-2026/pokersnowie-review and
  https://www.pokerscore.co.uk/pokersnowie-review/
* **Advanced Poker Training.** Play against a spectrum of styles (calling station to TAG
  to GTO-leaning), MTT and SNG with ICM and bubble drills, adaptive difficulty.
  Source: https://www.pokertraining.com/
* **Vision GTO (Run It Once).** PLO only, but the one product combining a GTO baseline
  with exploit-versus-persona training (Maniac, Nit, Whale).
  Source: https://www.mypokercoaching.com/vision-gto-trainer-review/
* **SOLVED GTO.** Free, 6-max cash only; ICM and push-fold listed as in development.
  Source: https://www.solvedgto.com/

### Grading done well

* GTO Wizard's tiers are EV-loss based: Inaccuracy is a legal minority action that costs
  little; Blunder is a never-taken action that costs a lot; only actions correct at some
  frequency count as Correct. Sources: https://help.gtowizard.com/how-to-use-the-trainer/
  and https://www.pokernews.com/poker-tools/gto-wizard/trainer.htm
* RNG mode is the accepted answer to mixed strategies.
* Snowie's click-an-error-to-jump-to-the-node with commentary is the drill-back pattern.
* No product documents spaced repetition of missed spots (Anki-style intervals). Streaks
  are engagement mechanics, not scheduling. Inferred from absence; a plausible
  differentiator for us.

### How AI explanations are actually produced

* GTO Wizard's "GTO Wizard AI Explained" says translating solver output into human
  language is a stated goal and documents the solver, not the explanation layer.
  Source: https://blog.gtowizard.com/gto-wizard-ai-explained/
* GTO Wizard exposes the inputs a coach would reason over: EV in bb, equity, a 0 to 10
  "Value Removal" blocker score, and aggregate reports over 1,755 flops.
  Source: https://blog.gtowizard.com/interpreting-equity-distributions/
* A 2026 arXiv benchmark comparing GTO Wizard AI's policy to GPT-5.3 found the language
  model unbalanced with wrong raise and fold frequencies. An off-the-shelf model does not
  reproduce solver strategy on its own. The coach must be fed the solver's numbers, never
  asked to guess the strategy. Source: https://arxiv.org/html/2603.23660v1
* DTO's Virtual Coach and Octopi's Ask George: marketing claims, no architecture.
* Odin (odinpoker.io) is a separate product that became a real-time-assistance tool and
  was banned by sites. Not a coaching reference.
  Source: https://www.vegasslotsonline.com/news/2023/03/08/poker-trainer-odin-just-became-an-rta-tool/

### What makes it fun

* **Chess.com Game Review.** An Accuracy score against the engine's top move and a
  selectable coach avatar that comments during retries; Chess.com removed a separate
  Hint feature and routes all guidance through the Coach retry loop.
  Source: https://support.chess.com/en/articles/8584089-how-does-game-review-work
* Criticism: Accuracy is not comparable across time controls and is misread as strength;
  the coach explains by pattern-matching and misses the strategic point; Android users
  were forced into a human avatar with no "none" option.
  Sources: https://www.chess.com/forum/view/general/game-review-estimated-game-rating-is-a-lie-and-deceit ,
  https://www.chess.com/forum/view/site-feedback/android-now-forced-to-have-human-avatar-for-coach-in-game-review ,
  https://chessgrader.com/blog/is-chess-com-game-review-accurate/
* **Lichess Puzzle Streak.** Progressively harder puzzles, one miss ends the run, one skip
  allowed. Pure streak pressure. Source: https://lichess.org/streak
* **Duolingo.** Streaks and leaderboards measurably raise engagement and also produce
  documented "streak anxiety" where keeping the streak displaces the learning.
  Sources: https://www.orizon.co/blog/duolingos-gamification-secrets and
  https://thedecisionlab.com/insights/consumer-insights/streak-creep-the-perils-of-too-much-gamification

### Mid-hand versus post-hand coaching

* Every named product grades after the decision. One niche tool, The Poker Coach, shows a
  suggested action and rationale on demand during play.
  Source: https://www.playgreatpoker.com/PokerCoach.html
* Live in-hand solver advice against real opponents is the definition of real-time
  assistance and a policy tripwire. Our app plays only against its own bots, so a coach
  that speaks before you act is a UX choice here, not an ethics problem. Worth stating
  in the product brief so it never drifts.

### Formats supported

* GTO Wizard is broadest (2-max to 9-max cash, MTT chip EV and ICM, spin, HU SNG, all
  tournament formats heads-up postflop).
  Source: https://blog.gtowizard.com/icm-mtt-9max-cash-solutions-great-improvements/
* SOLVED GTO is 6-max cash only today; a live example of the tournament gap.
* APT spans cash and tournaments with ICM drills.
* Breadth and depth are a trade-off across the market; no product has both.

### How players learn

* Deliberate practice framing across secondary sources: repetition, immediate feedback,
  name the specific gap, drill it until automatic; spend 60 to 70 percent of study time
  on identified leaks. Sources: https://www.pokerlistings.com/poker-strategies/psychology/how-deliberate-practice-works-in-poker
  and https://www.casino.org/blog/practice-in-poker/
* One source cites unnamed "Harvard research" for spaced repetition and a 90 percent
  accuracy threshold before moving on. The attribution is unverified; spaced repetition
  itself is well established elsewhere. Source: https://www.splitsuit.com/how-to-practice-poker

## Unverified

* The architecture behind GTO Wizard AI's, DTO's, and Octopi's explanations.
* Whether any trainer implements true spaced repetition.
* The Harvard citation.
* GTO Gecko's SHAP claim (vendor only).
* Exact EV-loss thresholds for Snowie, DTO, SOLVED, and Octopi.

## Also worth knowing

* No competitor has a proven, shipped, grounded language-model coach. The gap is real and
  so is the risk: the first one that states a wrong number loses trust permanently.
* GTOBase (no game) and SOLVED GTO (no tournaments) are useful negative references.

## What this means for our plan

* Grade on EV loss with GTO Wizard's five tiers and an RNG mode, never "matched the top
  action". Every grade shows the exploitability of the solve behind it.
* The coach's default is to speak *after* the action, in a corner bubble, with a
  one-click "why?" that opens the numbers. A setting lets Caleb turn on "ask before I
  act" and a separate "warn me before a blunder" mode, because we play only against bots.
* Explanations are grounded: the coach receives the solver's frequencies, EVs, equity,
  and blocker data as structured input and may only cite numbers it was given (see
  `app-stack-and-coach-integration.md` for the mechanism).
* Leak tracking is per spot category and per street, Snowie-style, with click-to-drill.
  Spaced repetition of missed spots is a first-class feature since nobody ships it.
* Avoid streak anxiety: track progress, do not punish a missed day. The avatar is
  optional and can be turned off. Never show a single "accuracy" number without context.
