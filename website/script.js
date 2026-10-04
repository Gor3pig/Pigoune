"use strict";

const LATEST_RELEASE_API = "https://api.github.com/repos/Gor3pig/Pigoune/releases/latest";

function revealSectionsWhileScrolling() {
  const sections = document.querySelectorAll(".reveal");
  if (!("IntersectionObserver" in window)) {
    return;
  }
  document.documentElement.classList.add("reveals");
  const observer = new IntersectionObserver((entries) => {
    for (const entry of entries) {
      if (entry.isIntersecting) {
        entry.target.classList.add("revealed");
        observer.unobserve(entry.target);
      }
    }
  }, { rootMargin: "0px 0px -10% 0px" });
  sections.forEach((section) => observer.observe(section));
}

function enlargeScreenshotsOnClick() {
  const lightbox = document.createElement("dialog");
  lightbox.className = "lightbox";
  lightbox.setAttribute("aria-label", document.body.dataset.closeLabel);
  const picture = document.createElement("img");
  lightbox.append(picture);
  document.body.append(lightbox);
  lightbox.addEventListener("click", () => lightbox.close());

  document.querySelectorAll("a.zoom").forEach((link) => {
    link.addEventListener("click", (event) => {
      event.preventDefault();
      const thumbnail = link.querySelector("img");
      picture.src = link.href;
      picture.alt = thumbnail.alt;
      lightbox.showModal();
    });
  });
}

async function showLatestVersion() {
  const button = document.querySelector("[data-latest-label]");
  try {
    const response = await fetch(LATEST_RELEASE_API, { headers: { Accept: "application/vnd.github+json" } });
    if (!response.ok) {
      return;
    }
    const release = await response.json();
    const version = String(release.tag_name || "").replace(/^v/, "");
    if (/^\d+(\.\d+)*$/.test(version)) {
      button.textContent = button.dataset.latestLabel.replace("{version}", version);
    }
  } catch {
    return;
  }
}

function rememberChosenLanguage() {
  document.querySelectorAll("[data-language]").forEach((link) => {
    link.addEventListener("click", () => {
      try {
        localStorage.setItem("pigoune-language", link.dataset.language);
      } catch {
        return;
      }
    });
  });
}

rememberChosenLanguage();
revealSectionsWhileScrolling();
enlargeScreenshotsOnClick();
showLatestVersion();
