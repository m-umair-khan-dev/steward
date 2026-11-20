// Light or dark: the visitor's choice (kept in this browser only), else the
// system's. Runs before paint, so the page never flashes the wrong theme.
(function () {
  var stored = null;
  try { stored = localStorage.getItem("theme"); } catch (e) { /* private mode */ }
  var dark = stored ? stored === "dark" : window.matchMedia("(prefers-color-scheme: dark)").matches;
  document.documentElement.dataset.theme = dark ? "dark" : "light";
  document.addEventListener("DOMContentLoaded", function () {
    var button = document.getElementById("theme-toggle");
    if (!button) return;
    button.addEventListener("click", function () {
      var next = document.documentElement.dataset.theme === "dark" ? "light" : "dark";
      document.documentElement.dataset.theme = next;
      try { localStorage.setItem("theme", next); } catch (e) { /* private mode */ }
    });
  });
})();
