// Autocomplete for the search box. Neleto loads this as an ES module and calls
// `init({ id, data, element })` once per component instance on the page, passing
// the component's root element. We attach a debounced suggestion dropdown that
// queries the plugin's public /search/suggest endpoint.

export function init({ element }) {
    const form = element.matches("form.ss-bar") ? element : element.querySelector("form.ss-bar");
    if (!form) return;
    const input = form.querySelector('input[name="q"]');
    if (!input || input.dataset.ssAuto) return;
    input.dataset.ssAuto = "1";
    input.setAttribute("autocomplete", "off");

    const box = document.createElement("ul");
    box.className = "ss-bar__suggest";
    box.setAttribute("role", "listbox");
    box.hidden = true;
    form.appendChild(box);

    let timer;
    let controller;
    let items = [];
    let active = -1;

    const esc = (s) =>
        (s ?? "").replace(/[&<>"]/g, (c) => ({ "&": "&amp;", "<": "&lt;", ">": "&gt;", '"': "&quot;" }[c]));

    const close = () => {
        box.hidden = true;
        box.innerHTML = "";
        items = [];
        active = -1;
    };

    const highlight = () => {
        [...box.children].forEach((li, i) => li.classList.toggle("is-active", i === active));
    };

    const render = (suggestions) => {
        items = suggestions || [];
        if (!items.length) return close();
        box.innerHTML = items
            .map((s, i) => `<li class="ss-bar__suggest-item" role="option" data-i="${i}">${esc(s.title)}</li>`)
            .join("");
        box.hidden = false;
        active = -1;
    };

    const fetchSuggest = async (q) => {
        if (controller) controller.abort();
        controller = new AbortController();
        try {
            const res = await fetch("/search/suggest?q=" + encodeURIComponent(q), {
                signal: controller.signal,
                headers: { Accept: "application/json" },
            });
            if (!res.ok) return close();
            render(await res.json());
        } catch (e) {
            /* aborted or offline — leave the box as-is */
        }
    };

    const go = (i) => {
        if (items[i] && items[i].url) window.location.href = items[i].url;
    };

    input.addEventListener("input", () => {
        const q = input.value.trim();
        clearTimeout(timer);
        if (q.length < 2) return close();
        timer = setTimeout(() => fetchSuggest(q), 150);
    });

    input.addEventListener("keydown", (e) => {
        if (box.hidden) return;
        if (e.key === "ArrowDown") {
            e.preventDefault();
            active = Math.min(active + 1, items.length - 1);
            highlight();
        } else if (e.key === "ArrowUp") {
            e.preventDefault();
            active = Math.max(active - 1, 0);
            highlight();
        } else if (e.key === "Enter" && active >= 0) {
            e.preventDefault();
            go(active);
        } else if (e.key === "Escape") {
            close();
        }
    });

    box.addEventListener("mousedown", (e) => {
        const li = e.target.closest(".ss-bar__suggest-item");
        if (li) {
            e.preventDefault();
            go(Number(li.dataset.i));
        }
    });

    document.addEventListener("click", (e) => {
        if (!form.contains(e.target)) close();
    });
}
