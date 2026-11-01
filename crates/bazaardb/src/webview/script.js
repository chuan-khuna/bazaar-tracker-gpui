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
})();
