/*
  Shared script for privacy.html and 404.html. Neither page has a form or
  reviews, so all this does is fill contact details from config.js and remove
  the line entirely when there are none yet.

  Removing rather than showing an empty sentence matters on the privacy page:
  "ask us and we will delete it" followed by nothing is worse than not making
  the offer until there is a mailbox to receive it.
*/
(function () {
  "use strict";

  var config = window.SITE_CONFIG || {};

  function contactSentence() {
    var ways = [];
    if (config.phone) ways.push(config.phone);
    if (config.email) ways.push(config.email);
    if (!ways.length) return "";
    return "Reach us on " + ways.join(" or ") + ".";
  }

  var sentence = contactSentence();
  var nodes = document.querySelectorAll('[data-config="contactSentence"]');
  for (var i = 0; i < nodes.length; i++) {
    if (sentence) {
      nodes[i].textContent = sentence;
    } else {
      nodes[i].hidden = true;
    }
  }

  var others = document.querySelectorAll("[data-config]:not([data-config='contactSentence'])");
  for (var j = 0; j < others.length; j++) {
    var value = config[others[j].getAttribute("data-config")];
    if (typeof value === "string" && value !== "") others[j].textContent = value;
  }

  // Same rule as main.js: a tel: link with no number behind it dials nothing,
  // so the button goes rather than sitting there looking tappable.
  var tels = document.querySelectorAll("[data-config-tel]");
  for (var k = 0; k < tels.length; k++) {
    if (config.phoneHref) {
      tels[k].setAttribute("href", "tel:" + config.phoneHref);
    } else if (tels[k].parentNode) {
      tels[k].parentNode.removeChild(tels[k]);
    }
  }
})();
