/*
  Review data for the reviews section. THIS FILE IS GENERATED.

  scripts/scaffold.py writes it from the saved Places API response, so every
  quote is verbatim and nothing is retyped. If you are reading this stub, the
  scaffold has not run yet.

  THE RULES IT IS WRITTEN UNDER, which survive every regeneration:

  * Google returns at most five reviews and picks which five. Whatever it gives
    is all there is; there is no page two.
  * `show` is a per-review decision and the reason sits in a comment beside it.
    A review that names a different trading name, or a one star about something
    other than the work, is a judgement call, not an automatic hide.
  * Any card marked `placeholder: true` is a SLOT, not a testimonial. It has no
    reviewer name and no star rating and it says plainly that it is waiting on a
    review. Never write a fake review to fill a grid.
  * main.js counts only real reviews in the "showing N of M" line, so six cards
    can render while it truthfully says four.
  * Every string here was typed by a stranger. main.js renders all of it with
    textContent and never innerHTML.
*/
window.SITE_REVIEWS = [];
