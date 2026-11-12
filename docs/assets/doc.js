// Shared behaviour for the HTML documents under docs/. Load it as a classic deferred script, so it also
// works from file://:
//   <script defer src="../assets/doc.js"></script>
// and keep the pre-paint theme snippet in <head> (see docs/documents/advisor-message-flow.html).
//
// - Theme switcher in the topbar's [data-doc-controls]: Auto (monitor), Moon (moon), Dawn (sun).
// - Code blocks (<pre><code class="language-x">): a header with the language and a copy button,
//   highlighted by Shiki; one button cycles Rosé Pine, Catppuccin and Everforest.
// - Diagrams (<pre class="mermaid">): drawn by Pele, with Expand opening a zoomable view.
// - Table of contents from main's h2[id] and h3[id], into <nav class="toc">, sticky with the section
//   being read marked; under 1024px a folding list at the top of main instead.
// - Glossary: <a class="term" href="#g-x"> shows the card <div class="term-card" id="g-x"> (its <dt>
//   and <dd>) when hovered or focused; a click goes to the card and lights it up.

(() => {
  const SHIKI_URL = "https://esm.sh/shiki@4.5.0";
  const PELE_URL = "https://cdn.jsdelivr.net/npm/pele@1.10.0/dist/pele.global.js";
  const root = document.documentElement;
  const store = {
    get(key) { try { return localStorage.getItem(key); } catch { return null; } },
    set(key, value) { try { value == null ? localStorage.removeItem(key) : localStorage.setItem(key, value); } catch {} },
  };

  // ── Icons (Lucide shapes; stroke is the text colour) ──
  const ICONS = {
    monitor: '<rect x="2" y="3" width="20" height="14" rx="2"/><path d="M8 21h8M12 17v4"/>',
    moon: '<path d="M12 3a6 6 0 0 0 9 9 9 9 0 1 1-9-9Z"/>',
    sun: '<circle cx="12" cy="12" r="4"/><path d="M12 2v2M12 20v2M4.93 4.93l1.41 1.41M17.66 17.66l1.41 1.41M2 12h2M20 12h2M6.34 17.66l-1.41 1.41M19.07 4.93l-1.41 1.41"/>',
    code: '<path d="m16 18 6-6-6-6M8 6l-6 6 6 6"/>',
    copy: '<rect x="9" y="9" width="13" height="13" rx="2"/><path d="M5 15H4a2 2 0 0 1-2-2V4a2 2 0 0 1 2-2h9a2 2 0 0 1 2 2v1"/>',
    check: '<path d="M20 6 9 17l-5-5"/>',
    expand: '<path d="M15 3h6v6M9 21H3v-6M21 3l-7 7M3 21l7-7"/>',
    zoomIn: '<circle cx="11" cy="11" r="8"/><path d="m21 21-4.3-4.3M11 8v6M8 11h6"/>',
    zoomOut: '<circle cx="11" cy="11" r="8"/><path d="m21 21-4.3-4.3M8 11h6"/>',
    reset: '<path d="M3 12a9 9 0 1 0 9-9 9.75 9.75 0 0 0-6.74 2.74L3 8"/><path d="M3 3v5h5"/>',
    close: '<path d="M18 6 6 18M6 6l12 12"/>',
  };
  const icon = (name) => `<svg viewBox="0 0 24 24" aria-hidden="true">${ICONS[name]}</svg>`;
  const button = (name, label, attrs = "", cls = "icon-button") =>
    `<button class="${cls}" type="button" title="${label}" aria-label="${label}" ${attrs}>${icon(name)}</button>`;
  const escapeHtml = (text) => text.replace(/[&<>"]/g, (c) => ({ "&": "&amp;", "<": "&lt;", ">": "&gt;", '"': "&quot;" })[c]);

  // ── Theme: Auto follows the system; Moon and Dawn are kept for next time ──
  function mountThemeSwitcher(box) {
    const group = document.createElement("div");
    group.className = "group";
    group.setAttribute("role", "group");
    group.setAttribute("aria-label", "Theme");
    group.innerHTML =
      button("monitor", "Auto", 'data-theme-choice="auto"') +
      button("moon", "Moon", 'data-theme-choice="moon"') +
      button("sun", "Dawn", 'data-theme-choice="dawn"');
    const show = (choice) =>
      group.querySelectorAll("[data-theme-choice]").forEach((b) => b.setAttribute("aria-pressed", String(b.dataset.themeChoice === choice)));
    show(root.dataset.theme || "auto");
    group.addEventListener("click", (event) => {
      const choice = event.target.closest("[data-theme-choice]")?.dataset.themeChoice;
      if (!choice) return;
      if (choice === "auto") delete root.dataset.theme;
      else root.dataset.theme = choice;
      store.set("docs-theme", choice === "auto" ? null : choice);
      show(choice);
    });
    box.append(group);
  }

  // ── Code blocks: Shiki, light and dark variants at once so the page theme needs no repaint ──
  const CODE_THEMES = [
    { name: "Rosé Pine", light: "rose-pine-dawn", dark: "rose-pine-moon" },
    { name: "Catppuccin", light: "catppuccin-latte", dark: "catppuccin-mocha" },
    { name: "Everforest", light: "everforest-light", dark: "everforest-dark" },
  ];
  let codeTheme = Math.max(0, CODE_THEMES.findIndex((t) => t.name === store.get("docs-code-theme")));
  let shiki;

  function wrapCodeBlocks() {
    for (const code of document.querySelectorAll("main pre > code[class^='language-']")) {
      const pre = code.parentElement;
      const lang = code.className.replace("language-", "");
      const block = document.createElement("div");
      block.className = "code-block";
      block.dataset.lang = lang;
      block.dataset.source = code.textContent;
      block.innerHTML = `<div class="code-head"><span>${lang}</span>${button("copy", "Copy", "data-copy")}</div>`;
      pre.replaceWith(block);
      block.append(pre);
    }
  }

  async function paintCode() {
    const blocks = document.querySelectorAll(".code-block");
    if (!blocks.length) return;
    try {
      shiki ??= await import(SHIKI_URL);
    } catch (error) {
      console.warn("Shiki could not load; code stays plain.", error);
      return;
    }
    const theme = CODE_THEMES[codeTheme];
    for (const block of blocks) {
      const options = { themes: { light: theme.light, dark: theme.dark }, defaultColor: false };
      let html;
      try {
        html = await shiki.codeToHtml(block.dataset.source, { ...options, lang: block.dataset.lang });
      } catch {
        html = await shiki.codeToHtml(block.dataset.source, { ...options, lang: "text" });
      }
      block.querySelector("pre")?.remove();
      block.insertAdjacentHTML("beforeend", html);
    }
  }

  function mountCodeThemeButton(box) {
    if (!document.querySelector(".code-block")) return;
    const holder = document.createElement("div");
    const label = () => `Code theme: ${CODE_THEMES[codeTheme].name}`;
    holder.innerHTML = button("code", label(), "", "icon-button solo");
    const btn = holder.firstElementChild;
    btn.addEventListener("click", async () => {
      codeTheme = (codeTheme + 1) % CODE_THEMES.length;
      store.set("docs-code-theme", CODE_THEMES[codeTheme].name);
      btn.title = label();
      btn.setAttribute("aria-label", label());
      await paintCode();
    });
    box.append(btn);
  }

  document.addEventListener("click", async (event) => {
    const btn = event.target.closest("[data-copy]");
    if (!btn) return;
    try {
      await navigator.clipboard.writeText(btn.closest(".code-block").dataset.source);
      btn.innerHTML = icon("check");
      setTimeout(() => (btn.innerHTML = icon("copy")), 1200);
    } catch {}
  });

  // ── Diagrams: Pele draws each to fit; Expand opens it zoomable in one shared dialog ──
  let pelePromise;
  const loadPele = () =>
    (pelePromise ??= new Promise((resolve, reject) => {
      if (window.pele) return resolve(window.pele);
      const script = document.createElement("script");
      script.src = PELE_URL;
      script.onload = () => resolve(window.pele);
      script.onerror = reject;
      document.head.append(script);
    }));

  function viewer() {
    let dialog = document.getElementById("doc-viewer");
    if (dialog) return dialog;
    dialog = document.createElement("dialog");
    dialog.className = "viewer";
    dialog.id = "doc-viewer";
    dialog.setAttribute("aria-label", "Diagram");
    dialog.innerHTML = `<header><span class="title"></span>${button("zoomOut", "Zoom out", 'data-zoom="out"')}${button("zoomIn", "Zoom in", 'data-zoom="in"')}${button("reset", "Fit to window", 'data-zoom="reset"')}${button("close", "Close", "data-close")}</header><div class="canvas"></div>`;
    document.body.append(dialog);
    const canvas = dialog.querySelector(".canvas");
    dialog.addEventListener("click", (event) => {
      if (event.target === dialog || event.target.closest("[data-close]")) return dialog.close();
      const how = event.target.closest("[data-zoom]")?.dataset.zoom;
      const zoom = dialog.mounted?.zoom;
      if (!how || !zoom) return;
      if (how === "in") zoom.zoomBy(1.25);
      else if (how === "out") zoom.zoomBy(0.8);
      else zoom.reset();
    });
    dialog.addEventListener("close", () => {
      dialog.mounted?.destroy();
      dialog.mounted = null;
      canvas.replaceChildren();
    });
    return dialog;
  }

  async function drawDiagrams() {
    const blocks = [...document.querySelectorAll("pre.mermaid")];
    if (!blocks.length) return;
    let pele;
    try {
      pele = await loadPele();
    } catch (error) {
      console.warn("Pele could not load; diagrams stay as Mermaid text.", error);
      return;
    }
    for (const block of blocks) {
      const source = block.textContent;
      const before = block.previousElementSibling;
      const name = block.dataset.title || (before?.matches("h2, h3") ? before.textContent : "Diagram");
      const figure = document.createElement("div");
      figure.className = "diagram";
      const drawing = document.createElement("div");
      figure.append(drawing);
      block.replaceWith(figure);
      try {
        pele.mount(drawing, source, { zoom: false });
      } catch (error) {
        figure.replaceWith(block);
        console.error("Pele could not draw a diagram", error);
        continue;
      }
      const tools = document.createElement("div");
      tools.className = "tools";
      tools.innerHTML = button("expand", "Expand");
      tools.querySelector("button").addEventListener("click", () => {
        const dialog = viewer();
        dialog.querySelector(".title").textContent = name;
        dialog.showModal();
        dialog.mounted = pele.mount(dialog.querySelector(".canvas"), source, { zoom: { always: true, controls: false, maxScale: 8 } });
      });
      figure.append(tools);
    }
  }

  // ── Table of contents, with the section being read marked ──
  function buildToc() {
    const main = document.querySelector("main");
    const side = document.querySelector("nav.toc");
    if (!main || !side) return;
    const headings = [...main.querySelectorAll("h2[id], h3[id]")];
    if (!headings.length) return;
    const items = headings
      .map((h) => `<li class="${h.tagName === "H3" ? "sub" : ""}"><a href="#${h.id}">${escapeHtml(h.textContent)}</a></li>`)
      .join("");
    side.innerHTML = `<div class="toc-title">Contents</div><ol>${items}</ol>`;
    const mobile = document.createElement("details");
    mobile.className = "toc-mobile";
    mobile.innerHTML = `<summary>Contents</summary><ol>${items}</ol>`;
    (main.querySelector(".meta") ?? main.querySelector("h1"))?.after(mobile);

    const links = new Map([...side.querySelectorAll("a")].map((a) => [a.hash.slice(1), a]));
    let shown = null;
    const mark = () => {
      const line = parseFloat(getComputedStyle(root).scrollPaddingTop) + 24 || 100;
      let current = headings[0];
      for (const h of headings) {
        if (h.getBoundingClientRect().top <= line) current = h;
        else break;
      }
      if (innerHeight + scrollY >= document.documentElement.scrollHeight - 2) current = headings.at(-1);
      if (current === shown) return;
      shown = current;
      links.forEach((a, id) => a.classList.toggle("active", id === current.id));
      // Keep the marked entry in view inside the TOC without scrolling the page.
      const a = links.get(current.id);
      if (a && (a.offsetTop < side.scrollTop || a.offsetTop > side.scrollTop + side.clientHeight - 40)) {
        side.scrollTop = a.offsetTop - side.clientHeight / 3;
      }
    };
    let queued = false;
    addEventListener("scroll", () => {
      if (queued) return;
      queued = true;
      requestAnimationFrame(() => { queued = false; mark(); });
    }, { passive: true });
    mark();
  }

  // ── Glossary: a hover card from the term's card on the page ──
  function glossary() {
    if (!document.querySelector("a.term")) return;
    const pop = document.createElement("div");
    pop.className = "term-pop";
    pop.id = "term-pop";
    pop.setAttribute("role", "tooltip");
    pop.hidden = true;
    document.body.append(pop);
    let hideTimer;

    // The term card a #hash names, or null.
    const cardFor = (hash) => {
      const id = decodeURIComponent(hash.slice(1));
      const card = id ? document.getElementById(id) : null;
      return card && card.classList.contains("term-card") ? card : null;
    };
    const cardOf = (term) => cardFor(term.hash);
    const show = (term) => {
      const card = cardOf(term);
      if (!card) return;
      clearTimeout(hideTimer);
      const source = card.dataset.source;
      pop.innerHTML =
        `<div class="name">${card.querySelector("dt").innerHTML}</div>` +
        `<div class="body">${card.querySelector("dd").innerHTML}</div>` +
        (source ? `<div class="source">${escapeHtml(source)}</div>` : "");
      pop.hidden = false;
      const at = term.getBoundingClientRect();
      const width = pop.offsetWidth;
      const height = pop.offsetHeight;
      const left = Math.min(Math.max(8, at.left), innerWidth - width - 8);
      const below = at.bottom + 8 + height <= innerHeight;
      pop.style.left = `${left}px`;
      pop.style.top = `${below ? at.bottom + 8 : Math.max(8, at.top - height - 8)}px`;
      term.setAttribute("aria-describedby", "term-pop");
    };
    const hide = () => {
      pop.hidden = true;
      document.querySelectorAll("a.term[aria-describedby]").forEach((t) => t.removeAttribute("aria-describedby"));
    };
    const later = () => {
      clearTimeout(hideTimer);
      hideTimer = setTimeout(hide, 80);
    };

    document.addEventListener("pointerover", (event) => {
      const term = event.target.closest("a.term");
      if (term) show(term);
    });
    document.addEventListener("pointerout", (event) => {
      const term = event.target.closest("a.term");
      if (term && !term.contains(event.relatedTarget)) later();
    });
    document.addEventListener("focusin", (event) => {
      const term = event.target.closest?.("a.term");
      if (term) show(term);
    });
    document.addEventListener("focusout", (event) => {
      if (event.target.closest?.("a.term")) later();
    });
    document.addEventListener("keydown", (event) => { if (event.key === "Escape") hide(); });
    addEventListener("scroll", hide, { passive: true });

    // Going to a card (a click on a term, or a #g-… link) lights the card up for a moment.
    const flash = () => {
      const card = cardFor(location.hash);
      if (!card) return;
      card.classList.remove("flash");
      void card.offsetWidth;
      card.classList.add("flash");
      setTimeout(() => card.classList.remove("flash"), 1600);
    };
    addEventListener("hashchange", () => { hide(); flash(); });
    flash();
  }

  // ── Start ──
  function start() {
    const box = document.querySelector("[data-doc-controls]");
    wrapCodeBlocks();
    if (box) {
      mountThemeSwitcher(box);
      mountCodeThemeButton(box);
    }
    buildToc();
    glossary();
    drawDiagrams();
    paintCode();
  }
  if (document.readyState === "loading") document.addEventListener("DOMContentLoaded", start);
  else start();
})();
