"use strict";

(() => {
  const SUPPORTED_LANGUAGES = ["en", "fr"];
  const FRENCH_ANCHORS = { "#install": "#installer", "#systems": "#systemes" };

  function chosenLanguage() {
    try {
      return localStorage.getItem("pigoune-language");
    } catch {
      return null;
    }
  }

  function preferredLanguage() {
    const languages = navigator.languages && navigator.languages.length > 0
      ? navigator.languages
      : [navigator.language || ""];
    for (const language of languages) {
      const base = language.toLowerCase().split("-")[0];
      if (SUPPORTED_LANGUAGES.includes(base)) {
        return base;
      }
    }
    return "en";
  }

  if ((chosenLanguage() ?? preferredLanguage()) === "fr") {
    location.replace("fr/" + (FRENCH_ANCHORS[location.hash] ?? location.hash));
  }
})();
