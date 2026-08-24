const copyButtons = document.querySelectorAll("[data-copy-target]");

const menuToggle = document.querySelector("[data-menu-toggle]");
const siteNavigation = document.querySelector("[data-site-nav]");
const mobileNavigation = window.matchMedia("(max-width: 680px)");

function setMenuOpen(open, { restoreFocus = false } = {}) {
  if (!menuToggle || !siteNavigation) return;
  const nextOpen = Boolean(open && mobileNavigation.matches);
  menuToggle.setAttribute("aria-expanded", String(nextOpen));
  siteNavigation.dataset.open = String(nextOpen);
  document.body.classList.toggle("mobile-menu-open", nextOpen);
  if (restoreFocus) menuToggle.focus();
}

menuToggle?.addEventListener("click", () => {
  setMenuOpen(menuToggle.getAttribute("aria-expanded") !== "true");
});

siteNavigation?.querySelectorAll("a").forEach((link) => {
  link.addEventListener("click", () => setMenuOpen(false));
});

mobileNavigation.addEventListener("change", () => setMenuOpen(false));
setMenuOpen(false);

const productDemo = document.querySelector("[data-product-demo]");
const reducedMotion = window.matchMedia("(prefers-reduced-motion: reduce)");
if (productDemo && !reducedMotion.matches) {
  productDemo.play().catch(() => {
    // Browser autoplay policy may require the user to press Play.
  });
}
reducedMotion.addEventListener("change", (event) => {
  if (!productDemo) return;
  if (event.matches) {
    productDemo.pause();
  }
});

copyButtons.forEach((button) => {
  button.addEventListener("click", async () => {
    const target = document.getElementById(button.dataset.copyTarget);
    if (!target) return;
    const text = target.textContent.trim();
    try {
      await navigator.clipboard.writeText(text);
      const original = button.textContent;
      button.textContent = "Copied";
      window.setTimeout(() => {
        button.textContent = original;
      }, 1400);
    } catch {
      button.textContent = "Select";
    }
  });
});

const lightbox = document.querySelector(".lightbox");
const lightboxImage = lightbox?.querySelector("img");
const closeLightbox = lightbox?.querySelector(".lightbox-close");

document.querySelectorAll("[data-full]").forEach((button) => {
  button.addEventListener("click", () => {
    if (!lightbox || !lightboxImage) return;
    lightboxImage.src = button.dataset.full;
    lightboxImage.alt = button.querySelector("img")?.alt || "Screenshot preview";
    lightbox.hidden = false;
  });
});

function hideLightbox() {
  if (!lightbox || !lightboxImage) return;
  lightbox.hidden = true;
  lightboxImage.removeAttribute("src");
}

closeLightbox?.addEventListener("click", hideLightbox);
lightbox?.addEventListener("click", (event) => {
  if (event.target === lightbox) hideLightbox();
});
document.addEventListener("keydown", (event) => {
  if (event.key === "Escape") {
    hideLightbox();
    if (menuToggle?.getAttribute("aria-expanded") === "true") {
      setMenuOpen(false, { restoreFocus: true });
    }
  }
});

document.addEventListener("click", (event) => {
  if (menuToggle?.getAttribute("aria-expanded") !== "true") return;
  if (event.target instanceof Element && event.target.closest(".site-header")) return;
  setMenuOpen(false);
});

document.querySelectorAll(".mobile-rail").forEach((rail) => {
  rail.addEventListener("keydown", (event) => {
    if (!mobileNavigation.matches || !["ArrowLeft", "ArrowRight"].includes(event.key)) return;
    event.preventDefault();
    rail.scrollBy({
      left: event.key === "ArrowRight" ? rail.clientWidth * 0.82 : rail.clientWidth * -0.82,
      behavior: reducedMotion.matches ? "auto" : "smooth",
    });
  });
});

const filterInput = document.getElementById("doc-filter");
const docSections = [...document.querySelectorAll(".doc-section")];

filterInput?.addEventListener("input", () => {
  const query = filterInput.value.trim().toLowerCase();
  docSections.forEach((section) => {
    const text = section.textContent.toLowerCase();
    section.hidden = query.length > 0 && !text.includes(query);
  });
});
