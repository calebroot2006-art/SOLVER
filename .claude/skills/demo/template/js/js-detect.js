/*
  Marks the document as scripted before first paint, so CSS can tell the
  difference between "JavaScript has not run yet" and "JavaScript is off".
  Deliberately the only blocking script on the page, and it is two lines.
*/
document.documentElement.classList.add("js");
