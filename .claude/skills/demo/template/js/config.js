/*
  Site configuration for __BUSINESS_NAME__.
  Everything a partner might need to change lives here, not buried in the HTML.

  This file is PUBLIC. Anything in it can be read by anyone who visits the site.
  Never put secrets here (no API keys, no SMTP passwords, no Places key).

  WHERE THESE FACTS CAME FROM
  Everything below was read off the client's Google Business Profile through the
  Places API on 2026-08-29, place ID __PLACE_ID__. None of it has
  been confirmed with the client. The `confirmed` block at the bottom is what
  stops the site going live on facts nobody checked. build.py refuses to build
  while any of it is false.
*/
window.SITE_CONFIG = {
  // Business facts
  businessName: "__BUSINESS_NAME__",
  shortName: "__SHORT_NAME__",
  trade: "__TRADE_LOWER__",
  city: "__CITY__",

  // The service area line. The listing address is a house on a residential
  // street, so this is written as an area rather than a shopfront until the
  // client says otherwise. See `showAddress` below.
  serviceArea: "__SERVICE_AREA__",

  // Contact.
  // The listing says __PHONE__. Two other numbers exist on a 2021 flyer
  // A business that has traded under more than one name often has more than one number in circulation, so this is a best guess until somebody rings it. See the listing audit in the client's consulting folder.
  phone: "__PHONE__",
  phoneHref: "__PHONE_E164__", // E.164, for the tel: link. Keep the two in step.

  email: "", // Empty hides the email line. Business mailbox only, never a personal Gmail.

  // The listing address is __ADDRESS__. It is a house.
  // Publishing a home address is the client's call, not ours, so this stays
  // false and the footer shows the service area instead.
  address: "__ADDRESS__",
  showAddress: false,

  hours: "__HOURS__",

  // The Google listing. Read off the API response, not typed by hand.
  googleRating: "__RATING__",
  googleReviewCount: "__REVIEW_COUNT__",
  googleUrl: "__GOOGLE_URL__",
  googleWriteReviewUrl: "__GOOGLE_WRITE_REVIEW_URL__",

  // Where the inspection request form sends its data.
  // Option A: the intake receiver in tools/intake, behind HTTPS. It rate-limits,
  //   validates server-side and stores requests. Add its origin to connect-src in
  //   index.html AND in _headers, or the browser blocks the request.
  // Option B: a Formspree or Basin style endpoint that accepts a JSON POST.
  // Option C: leave empty. The form opens the visitor's mail app pre-filled,
  //   which needs `email` above to be set.
  formEndpoint: "",

  /*
    THE GATE.

    Every one of these is a fact a homeowner will act on: they will ring the
    number, expect the service, and believe the guarantee. Every one of them
    currently comes from a Google listing rather than from the client.

    Flip a flag to true only after a person has confirmed it out loud. build.py
    refuses to build the live site while any is false, which is the point:
    a protection that defaults to off is not a protection.
  */
  confirmed: {
    phone: false, // has somebody rung __PHONE__ and reached them?
    services: false, // do they do all four of the services listed on the page?
    serviceArea: false, // how far out do they actually travel?
    warranty: false, // the workmanship warranty line in the FAQ
    insurance: false, // "licensed and insured" is a legal claim, not a slogan
    tradingName: false, // is __BUSINESS_NAME__ the name they use today?
  },
};
