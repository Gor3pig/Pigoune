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

function runCarousels() {
  const calmRequested = window.matchMedia("(prefers-reduced-motion: reduce)").matches;
  const savingData = Boolean(navigator.connection && navigator.connection.saveData);
  document.querySelectorAll(".carousel").forEach((carousel) => {
    const labels = carousel.dataset;
    const stage = carousel.querySelector(".stage");
    const slides = [...carousel.querySelectorAll(".slide")];
    const count = slides.length;
    let current = 0;
    let manualPause = calmRequested || savingData;
    let visible = true;
    let hovered = false;

    const previous = document.createElement("button");
    const next = document.createElement("button");
    for (const [button, name, text, label] of [
      [previous, "prev", "‹", labels.labelPrevious],
      [next, "next", "›", labels.labelNext],
    ]) {
      button.type = "button";
      button.className = `arrow ${name}`;
      button.textContent = text;
      button.setAttribute("aria-label", label);
      stage.append(button);
    }
    const controls = document.createElement("div");
    controls.className = "controls";
    const toggle = document.createElement("button");
    toggle.type = "button";
    toggle.className = "toggle";
    const rail = document.createElement("div");
    rail.className = "rail";
    const state = document.createElement("div");
    state.className = "state";
    rail.setAttribute("aria-hidden", "true");
    controls.append(toggle, rail);
    const thumbnails = document.createElement("div");
    thumbnails.className = "thumbs";
    carousel.append(controls, thumbnails, state);

    const dots = slides.map((slide, index) => {
      const dot = document.createElement("button");
      dot.type = "button";
      dot.className = "dot";
      dot.setAttribute("aria-label", labels.labelImage.replace("{n}", index + 1).replace("{total}", count));
      dot.append(Object.assign(document.createElement("span"), { className: "fill" }));
      dot.tabIndex = -1;
      dot.addEventListener("click", () => show(index));
      rail.append(dot);
      slide.setAttribute("role", "group");
      slide.setAttribute("aria-roledescription", labels.labelSlide);
      slide.setAttribute("aria-label", `${index + 1} / ${count}`);
      return dot;
    });

    const thumbnailButtons = slides.map((slide, index) => {
      const button = document.createElement("button");
      button.type = "button";
      button.className = "thumb";
      button.setAttribute("aria-label", slide.querySelector("figcaption").textContent);
      const picture = document.createElement("img");
      picture.src = slide.dataset.thumb;
      picture.alt = "";
      picture.width = 320;
      picture.height = 180;
      picture.loading = "lazy";
      button.append(picture);
      button.addEventListener("click", () => show(index));
      thumbnails.append(button);
      return button;
    });

    stage.setAttribute("role", "group");
    stage.setAttribute("aria-roledescription", labels.labelCarousel);
    stage.setAttribute("aria-label", labels.labelScreenshots);
    stage.tabIndex = 0;
    carousel.classList.add("enhanced", "settling");

    function refresh() {
      const animated = !(calmRequested || savingData) || !manualPause;
      const held = hovered || manualPause || !visible || document.hidden;
      carousel.dataset.playing = String(animated);
      carousel.dataset.held = String(held);
      toggle.textContent = manualPause ? "▶" : "❚❚";
      toggle.setAttribute("aria-pressed", String(manualPause));
      toggle.setAttribute("aria-label", manualPause ? labels.labelPlay : labels.labelPause);
      if ((calmRequested || savingData) && manualPause) {
        state.textContent = labels.labelCalm;
      } else {
        state.textContent = hovered && !manualPause ? labels.labelHeld : "";
      }
    }

    function show(index) {
      current = (index + count) % count;
      slides.forEach((slide, position) => {
        slide.classList.toggle("on", position === current);
        slide.classList.toggle("past", position < current);
      });
      dots.forEach((dot, position) => {
        dot.classList.toggle("done", position < current);
        dot.classList.toggle("now", position === current);
        dot.setAttribute("aria-current", String(position === current));
      });
      thumbnailButtons.forEach((button, position) => {
        button.setAttribute("aria-current", String(position === current));
      });
      const active = thumbnailButtons[current];
      if (thumbnails.scrollWidth > thumbnails.clientWidth) {
        thumbnails.scrollTo({
          left: active.offsetLeft - (thumbnails.clientWidth - active.clientWidth) / 2,
          behavior: calmRequested ? "auto" : "smooth",
        });
      }
      refresh();
    }

    function hold(value) {
      hovered = value;
      refresh();
    }

    carousel.addEventListener("animationend", (event) => {
      if (event.animationName === "carousel-fill") {
        show(current + 1);
      }
    });
    previous.addEventListener("click", () => show(current - 1));
    next.addEventListener("click", () => show(current + 1));
    toggle.addEventListener("click", () => {
      manualPause = !manualPause;
      refresh();
    });
    stage.addEventListener("keydown", (event) => {
      if (event.key === "ArrowRight") {
        event.preventDefault();
        show(current + 1);
      } else if (event.key === "ArrowLeft") {
        event.preventDefault();
        show(current - 1);
      }
    });
    carousel.addEventListener("mouseenter", () => hold(true));
    carousel.addEventListener("mouseleave", () => hold(false));
    carousel.addEventListener("focusin", () => hold(true));
    carousel.addEventListener("focusout", () => hold(false));

    let startX = null;
    let swiped = false;
    stage.addEventListener("pointerdown", (event) => {
      startX = event.clientX;
      swiped = false;
      hold(true);
    });
    stage.addEventListener("pointerup", (event) => {
      if (startX !== null && Math.abs(event.clientX - startX) > 40) {
        swiped = true;
        show(current + (event.clientX < startX ? 1 : -1));
      }
      startX = null;
      if (event.pointerType === "touch") {
        hold(false);
      }
    });
    stage.addEventListener("click", (event) => {
      if (swiped) {
        event.preventDefault();
        event.stopImmediatePropagation();
        swiped = false;
      }
    }, true);

    if ("IntersectionObserver" in window) {
      new IntersectionObserver((entries) => {
        visible = entries[0].isIntersecting;
        refresh();
      }, { threshold: 0.4 }).observe(carousel);
    }
    document.addEventListener("visibilitychange", refresh);
    show(0);
    requestAnimationFrame(() => requestAnimationFrame(() => carousel.classList.remove("settling")));
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
runCarousels();
enlargeScreenshotsOnClick();
showLatestVersion();
