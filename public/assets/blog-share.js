(() => {
  const button = document.getElementById('share-article');
  const fallback = document.getElementById('share-fallback');
  const input = document.getElementById('share-url');
  const status = document.getElementById('share-status');
  const copy = document.getElementById('copy-share-url');
  if (!button || !fallback || !input || !status || !copy) return;

  const url = new URL(input.value, window.location.href).href;
  input.value = url;
  button.hidden = false;
  fallback.hidden = true;
  copy.hidden = false;

  const showFallback = () => {
    fallback.hidden = false;
    fallback.open = true;
    input.focus();
    input.select();
    status.textContent = 'Copiez ce lien pour partager l’article.';
  };
  const copyLink = async () => {
    try {
      await navigator.clipboard.writeText(url);
      status.textContent = 'Lien copié';
    } catch (_) {
      showFallback();
    }
  };
  copy.addEventListener('click', copyLink);
  button.addEventListener('click', async () => {
    button.disabled = true;
    status.textContent = '';
    try {
      if (typeof navigator.share === 'function') {
        try {
          await navigator.share({ title: document.querySelector('article h1').textContent, url });
          return;
        } catch (error) {
          if (error.name === 'AbortError') return;
        }
      }
      await copyLink();
    } finally {
      button.disabled = false;
    }
  });
})();
