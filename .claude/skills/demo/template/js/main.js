/*
  The only page script.

  Everything it does is enhancement. With this file blocked or broken the page
  still reads, the motion still runs (it is pure CSS), the phone numbers in the
  markup still dial, and the form still submits through the browser's own
  validation. The reviews are the one exception and they are the last block on
  the page, so losing them costs a visitor nothing above the fold.

    1. Fill the values that live in config.js so nobody has to edit HTML.
    2. Render the reviews from reviews.js.
    3. Validate the inspection form for people. The receiver re-checks it.
    4. Send the request, or fall back to the visitor's mail app.
*/
(function () {
  "use strict";

  var config = window.SITE_CONFIG || {};
  var reviews = window.SITE_REVIEWS || [];

  /* ---------------------------------------------------------------------
     1. Config fill

     Three attributes, because three different things need filling:
       data-config="key"      the key's value becomes the element's text
       data-config-tel        href becomes tel:<phoneHref>, hidden if no phone
       data-config-href="key" the key's value becomes the element's href
     --------------------------------------------------------------------- */

  function fillText() {
    var nodes = document.querySelectorAll("[data-config]");
    for (var i = 0; i < nodes.length; i++) {
      var value = config[nodes[i].getAttribute("data-config")];
      if (typeof value === "string" && value !== "") {
        nodes[i].textContent = value;
      }
    }
  }

  function fillHrefs() {
    var links = document.querySelectorAll("[data-config-href]");
    for (var i = 0; i < links.length; i++) {
      var value = config[links[i].getAttribute("data-config-href")];
      if (typeof value === "string" && value !== "") {
        links[i].setAttribute("href", value);
      }
    }

    // The tel: links. If there is no number configured, the buttons are removed
    // rather than left pointing at "tel:", which on a phone dials nothing and
    // looks broken. A missing number should cost the visitor a button, not a tap.
    var tels = document.querySelectorAll("[data-config-tel]");
    for (var j = 0; j < tels.length; j++) {
      if (config.phoneHref) {
        tels[j].setAttribute("href", "tel:" + config.phoneHref);
      } else if (tels[j].parentNode) {
        tels[j].parentNode.removeChild(tels[j]);
      }
    }
  }

  // The listing address is a house. It only appears if config says it may.
  function fillAddress() {
    var nodes = document.querySelectorAll("[data-address]");
    for (var i = 0; i < nodes.length; i++) {
      if (config.showAddress && config.address) {
        nodes[i].textContent = config.address;
      } else if (nodes[i].parentNode) {
        nodes[i].parentNode.removeChild(nodes[i]);
      }
    }
  }

  /* ---------------------------------------------------------------------
     2. Reviews

     SECURITY. Every string in reviews.js was typed by a member of the public
     into Google. It is treated as hostile all the way to the DOM: this builds
     nodes and sets textContent, and there is no innerHTML anywhere in this
     file. A review containing markup renders as the characters the reviewer
     typed, which is both safe and correct.
     --------------------------------------------------------------------- */

  var STAR = "M8 1.2l2 4.3 4.6.6-3.4 3.2.9 4.7L8 11.7 3.9 14l.9-4.7L1.4 6.1 6 5.5z";

  function stars(count) {
    var wrap = document.createElement("span");
    wrap.className = "stars";
    wrap.setAttribute("role", "img");
    wrap.setAttribute("aria-label", count + " out of 5");
    for (var i = 0; i < count; i++) {
      var svg = document.createElementNS("http://www.w3.org/2000/svg", "svg");
      svg.setAttribute("viewBox", "0 0 16 16");
      svg.setAttribute("aria-hidden", "true");
      svg.setAttribute("focusable", "false");
      var path = document.createElementNS("http://www.w3.org/2000/svg", "path");
      path.setAttribute("d", STAR);
      svg.appendChild(path);
      wrap.appendChild(svg);
    }
    return wrap;
  }

  function reviewCard(review) {
    var figure = document.createElement("figure");
    figure.className = review.placeholder ? "review review-slot" : "review";

    // A slot gets the placeholder chip where a real card gets its stars. It
    // never gets a star rating, because a rating nobody gave is a lie however
    // it is styled.
    if (review.placeholder) {
      var chip = document.createElement("span");
      chip.className = "chip";
      chip.textContent = "Placeholder";
      figure.appendChild(chip);
    } else {
      figure.appendChild(stars(review.rating));
    }

    var quote = document.createElement("blockquote");
    quote.textContent = review.text;
    figure.appendChild(quote);

    var caption = document.createElement("figcaption");

    var name = document.createElement("p");
    name.className = "review-name";
    name.textContent = review.name;
    caption.appendChild(name);

    var when = document.createElement("p");
    when.className = "review-when";
    when.textContent = review.placeholder ? review.when : review.when + " on Google";
    caption.appendChild(when);

    figure.appendChild(caption);
    return figure;
  }

  function renderReviews() {
    var grid = document.getElementById("review-grid");
    var note = document.getElementById("reviews-note");
    if (!grid) return;

    var shown = [];
    for (var i = 0; i < reviews.length; i++) {
      if (reviews[i] && reviews[i].show) shown.push(reviews[i]);
    }

    // index.html ships the empty state in the markup so a visitor without
    // JavaScript reads something sensible. If there are cards to show, it goes.
    if (shown.length > 0) {
      var placeholder = document.getElementById("reviews-empty");
      if (placeholder && placeholder.parentNode) {
        placeholder.parentNode.removeChild(placeholder);
      }
    }

    if (shown.length === 0) {
      // Nothing to render, so the markup's own empty state is left standing.
    } else {
      // Newest first. Google returns them in its own order of relevance, which
      // is not chronological, and a wall of reviews reads oldest-looking-first
      // unless something sorts it.
      shown.sort(function (a, b) {
        return a.date < b.date ? 1 : a.date > b.date ? -1 : 0;
      });
      for (var j = 0; j < shown.length; j++) {
        grid.appendChild(reviewCard(shown[j]));
      }
    }

    // The honest-use rule from reviews.js. This page shows a selection, so it
    // says how many out of how many, every time, computed rather than typed.
    //
    // Slots are counted out deliberately. Six cards render but only four of
    // them are reviews, and this line is about reviews. Counting the slots here
    // would turn the one honest number on the page into a false one.
    if (note && config.googleReviewCount) {
      var real = 0;
      for (var k = 0; k < shown.length; k++) {
        if (!shown[k].placeholder) real++;
      }
      note.textContent =
        "Showing " + real + " of " + config.googleReviewCount + " Google reviews.";
    }
  }

  /* ---------------------------------------------------------------------
     3. Validation

     This exists so a person is told what is wrong before they wait for a round
     trip. It is not a security control: the receiver re-checks every field and
     trusts none of it.
     --------------------------------------------------------------------- */

  var form = document.getElementById("request-form");
  var status = document.getElementById("form-status");
  var success = document.getElementById("form-success");

  function fieldOf(input) {
    return input.closest(".field");
  }

  function setInvalid(input, invalid) {
    var field = fieldOf(input);
    if (!field) return;
    if (invalid) {
      field.setAttribute("data-invalid", "");
      input.setAttribute("aria-invalid", "true");
    } else {
      field.removeAttribute("data-invalid");
      input.removeAttribute("aria-invalid");
    }
  }

  // Deliberately loose. A phone number is valid if it holds enough digits to be
  // one. Anything stricter rejects real people writing real numbers.
  function phoneLooksReal(value) {
    var digits = value.replace(/\D/g, "");
    return digits.length >= 10 && digits.length <= 15;
  }

  function emailLooksReal(value) {
    return /^[^\s@]+@[^\s@]+\.[^\s@]{2,}$/.test(value);
  }

  function validate() {
    if (!form) return { ok: false, first: null };

    var first = null;
    var ok = true;

    function fail(input) {
      setInvalid(input, true);
      ok = false;
      if (!first) first = input;
    }

    var required = ["f-name", "f-phone", "f-address"];
    for (var i = 0; i < required.length; i++) {
      var input = document.getElementById(required[i]);
      if (!input) continue;
      var value = input.value.trim();
      if (value === "") {
        fail(input);
      } else if (input.id === "f-phone" && !phoneLooksReal(value)) {
        fail(input);
      } else {
        setInvalid(input, false);
      }
    }

    var email = document.getElementById("f-email");
    if (email) {
      var emailValue = email.value.trim();
      if (emailValue !== "" && !emailLooksReal(emailValue)) {
        fail(email);
      } else {
        setInvalid(email, false);
      }
    }

    return { ok: ok, first: first };
  }

  /* ---------------------------------------------------------------------
     4. Submit
     --------------------------------------------------------------------- */

  function payload() {
    var data = {};
    var fields = form.querySelectorAll("input, select, textarea");
    for (var i = 0; i < fields.length; i++) {
      if (fields[i].name) data[fields[i].name] = fields[i].value.trim();
    }
    return data;
  }

  function say(message, tone) {
    if (!status) return;
    status.textContent = message;
    if (tone) {
      status.setAttribute("data-tone", tone);
    } else {
      status.removeAttribute("data-tone");
    }
  }

  function contactLine() {
    return config.phone ? " You can also call us on " + config.phone + "." : "";
  }

  function showSuccess() {
    say("");
    if (form) form.hidden = true;
    if (!success) return;
    success.setAttribute("data-open", "");
    // The form the visitor was standing in has just been removed, so focus has
    // to go somewhere deliberate or it falls back to the top of the document
    // and a keyboard user loses their place entirely.
    success.setAttribute("tabindex", "-1");
    success.focus();
    success.scrollIntoView({ block: "center", behavior: "auto" });
  }

  // Used only when no formEndpoint is configured. It cannot know whether
  // anything was sent, so it never claims the request arrived.
  function mailtoFallback(data) {
    if (!config.email) {
      say(
        "The request form is not connected yet." + (contactLine() || " Please try again shortly."),
        "error"
      );
      return;
    }
    var lines = [
      "Name: " + (data.name || ""),
      "Phone: " + (data.phone || ""),
      "Email: " + (data.email || ""),
      "Address: " + (data.address || ""),
      "Roof age: " + (data.roofage || ""),
      "What is happening: " + (data.problem || ""),
      "",
      data.notes || "",
    ];
    window.location.href =
      "mailto:" +
      encodeURIComponent(config.email) +
      "?subject=" +
      encodeURIComponent("Roof inspection request: " + (data.address || data.name || "website")) +
      "&body=" +
      encodeURIComponent(lines.join("\n"));
    say("Your email app should be opening with the request filled in. Send it and we will call you back.");
  }

  function onSubmit(event) {
    event.preventDefault();

    // The honeypot. A person never sees this field, so anything in it is a bot.
    // Behave as though it worked rather than telling it why it failed.
    var honeypot = document.getElementById("f-company-website");
    if (honeypot && honeypot.value !== "") {
      showSuccess();
      return;
    }

    var result = validate();
    if (!result.ok) {
      say("Some fields still need filling in.", "error");
      if (result.first) result.first.focus();
      return;
    }

    var data = payload();
    var button = form.querySelector('button[type="submit"]');

    if (!config.formEndpoint) {
      mailtoFallback(data);
      return;
    }

    if (button) button.disabled = true;
    say("Sending.");

    fetch(config.formEndpoint, {
      method: "POST",
      headers: { "Content-Type": "application/json" },
      body: JSON.stringify(data),
    })
      .then(function (response) {
        if (response.ok) {
          showSuccess();
          return;
        }
        if (response.status === 429) {
          say("That is a few requests in a short time. Please wait a while and try again." + contactLine(), "error");
        } else {
          say("That did not send." + (contactLine() || " Please try again in a moment."), "error");
        }
        if (button) button.disabled = false;
      })
      .catch(function () {
        say("That did not send." + (contactLine() || " Please check your connection and try again."), "error");
        if (button) button.disabled = false;
      });
  }

  /* ---------------------------------------------------------------------
     Wiring
     --------------------------------------------------------------------- */

  fillText();
  fillHrefs();
  fillAddress();
  renderReviews();

  if (form) {
    form.addEventListener("submit", onSubmit);

    // Clear a field's error the moment the person starts fixing it. Waiting for
    // the next submit to forgive them reads as the form arguing back.
    form.addEventListener("input", function (event) {
      var target = event.target;
      if (target && fieldOf(target) && fieldOf(target).hasAttribute("data-invalid")) {
        setInvalid(target, false);
      }
    });
  }
})();
