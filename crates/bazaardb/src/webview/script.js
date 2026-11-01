// Added to each Bazaar DB page the web view loads. It copies the replies to the watched API paths
// to the app; the page's own requests go out and come back exactly as they would without it.
(() => {
  const paths = __PATHS__;
  const watched = (url) => {
    try { return paths.includes(new URL(String(url), location.href).pathname); } catch (_) { return false; }
  };
  const send = (url, status, body) => {
    try {
      window.ipc.postMessage(JSON.stringify({ url: new URL(String(url), location.href).href, status, body }));
    } catch (_) {}
  };
  const fetch = window.fetch;
  window.fetch = async function (input, init) {
    const response = await fetch.call(this, input, init);
    const url = typeof input === 'string' || input instanceof URL ? input : (input && input.url) || '';
    if (watched(url)) response.clone().text().then((body) => send(url, response.status, body), () => {});
    return response;
  };
  const open = XMLHttpRequest.prototype.open;
  XMLHttpRequest.prototype.open = function (method, url, ...rest) {
    if (watched(url)) this.addEventListener('load', () => send(url, this.status, this.responseText));
    return open.call(this, method, url, ...rest);
  };

  // Asked for by the player from an item's tile: when the address ends in `__FIND__<name>`, type the
  // name into the page's own item search, as the player would, and pick the suggestion with exactly
  // that name. The page then asks for the item's odds itself. How it went is sent to the app, which
  // shows the page to the player when this did not work.
  const FIND = '__FIND__';
  if (location.hash.startsWith(FIND)) {
    const wanted = decodeURIComponent(location.hash.slice(FIND.length));
    const key = (text) => String(text).toLowerCase().replace(/[^\p{L}\p{N}]/gu, '');
    const report = (outcome) => {
      try { window.ipc.postMessage(JSON.stringify({ find: wanted, outcome })); } catch (_) {}
    };
    const started = Date.now();
    let typed = false;
    const timer = setInterval(() => {
      const input = document.querySelector('input[placeholder^="Search for an item"]');
      if (!typed) {
        if (input) {
          const setValue = Object.getOwnPropertyDescriptor(HTMLInputElement.prototype, 'value').set;
          setValue.call(input, wanted);
          input.dispatchEvent(new Event('input', { bubbles: true }));
          typed = true;
        } else if (Date.now() - started > 15000) {
          clearInterval(timer);
          report('no-search-box');
        }
        return;
      }
      const match = [...document.querySelectorAll('div[data-type="item"]')].find((option) => key(option.textContent) === key(wanted));
      if (match) {
        clearInterval(timer);
        match.click();
        report('picked');
      } else if (Date.now() - started > 15000) {
        clearInterval(timer);
        report('no-match');
      }
    }, 250);
  }
})();
