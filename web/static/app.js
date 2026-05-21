(function () {
  window.showDialog = function ({ message, ok = 'OK', cancel = null, danger = false, onOk = null }) {
    const overlay   = document.getElementById('confirm-overlay');
    const msgEl     = document.getElementById('confirmMessage');
    const okBtn     = document.getElementById('confirmOk');
    const cancelBtn = document.getElementById('confirmCancel');
    if (!overlay) return;

    msgEl.textContent = message;
    okBtn.textContent = ok;
    okBtn.className = danger ? 'btn-sm btn-danger-confirm' : 'btn-sm';
    cancelBtn.hidden = !cancel;
    if (cancel) cancelBtn.textContent = cancel;
    overlay.hidden = false;
    (cancel ? cancelBtn : okBtn).focus();

    function cleanup() {
      overlay.hidden = true;
      okBtn.removeEventListener('click', handleOk);
      cancelBtn.removeEventListener('click', handleCancel);
      overlay.removeEventListener('click', handleOverlay);
      document.removeEventListener('keydown', handleKey);
    }
    function handleOk()      { cleanup(); if (onOk) onOk(); }
    function handleCancel()  { cleanup(); }
    function handleOverlay(e) { if (e.target === overlay) handleCancel(); }
    function handleKey(e)    { if (e.key === 'Escape') handleCancel(); }

    okBtn.addEventListener('click', handleOk);
    cancelBtn.addEventListener('click', handleCancel);
    overlay.addEventListener('click', handleOverlay);
    document.addEventListener('keydown', handleKey);
  };

  // Shortcuts overlay
  const overlay  = document.getElementById('shortcuts-overlay');
  const closeBtn = document.getElementById('shortcutsClose');

  function openShortcuts()  { if (overlay) overlay.hidden = false; }
  function closeShortcuts() { if (overlay) overlay.hidden = true; }

  if (closeBtn) closeBtn.addEventListener('click', closeShortcuts);
  if (overlay)  overlay.addEventListener('click', e => { if (e.target === overlay) closeShortcuts(); });

  // Keyboard navigation for the runs table
  let focusedRow = -1;
  const rows = () => Array.from(document.querySelectorAll('.runs-table tbody tr:not(.table-empty):not(.load-more-row)'));

  function focusRow(idx) {
    const all = rows();
    if (all.length === 0) return;
    if (focusedRow >= 0 && focusedRow < all.length) all[focusedRow].classList.remove('row-focused');
    focusedRow = Math.max(0, Math.min(idx, all.length - 1));
    const row = all[focusedRow];
    row.classList.add('row-focused');
    row.scrollIntoView({ block: 'nearest' });
  }

  function openFocusedRow() {
    const all = rows();
    if (focusedRow < 0 || focusedRow >= all.length) return;
    const link = all[focusedRow].querySelector('a');
    if (link) link.click();
  }

  function compareSelected() {
    const checked = Array.from(document.querySelectorAll('.runs-table input[type=checkbox]:checked'))
      .map(cb => cb.closest('tr'))
      .filter(Boolean)
      .map(tr => tr.dataset.runId)
      .filter(Boolean);
    if (checked.length < 2) return;
    const [a, b] = checked;
    const parts = window.location.pathname.split('/').filter(Boolean);
    if (parts.length >= 2) {
      window.location.href = `/${parts[0]}/${parts[1]}/compare?a=${a}&b=${b}`;
    }
  }

  async function deleteSelected() {
    const trs = Array.from(document.querySelectorAll('.runs-table tbody tr')).filter(tr => {
      const cb = tr.querySelector('input[type=checkbox]');
      return cb && cb.checked && tr.dataset.runId;
    });
    if (trs.length === 0) return;

    const count = trs.length;
    window.showDialog({
      message: `Delete ${count} run${count > 1 ? 's' : ''}? This cannot be undone.`,
      ok: 'Delete',
      cancel: 'Cancel',
      danger: true,
      onOk: async () => {
        const results = await Promise.all(
          trs.map(tr =>
            fetch('/v1/runs/' + tr.dataset.runId, { method: 'DELETE' })
              .then(r => ({ tr, ok: r.ok || r.status === 204 }))
              .catch(() => ({ tr, ok: false }))
          )
        );
        results.filter(r => r.ok).forEach(({ tr }) => tr.remove());
        updateSelectionUI();
        const failed = results.filter(r => !r.ok).length;
        if (failed > 0) {
          window.showDialog({ message: `${failed} deletion${failed > 1 ? 's' : ''} failed.` });
        }
      }
    });
  }

  document.addEventListener('keydown', e => {
    const tag       = document.activeElement ? document.activeElement.tagName : '';
    const inputType = (document.activeElement && document.activeElement.type) || '';
    const inInput   = (tag === 'INPUT' && inputType !== 'checkbox') || tag === 'TEXTAREA' || tag === 'SELECT';

    if (e.key === '?') { e.preventDefault(); openShortcuts(); return; }

    if (e.key === 'Escape') {
      closeShortcuts();
      if (inInput) document.activeElement.blur();
      return;
    }

    if (inInput) return;

    if      (e.key === 'j')     { e.preventDefault(); focusRow(focusedRow + 1); }
    else if (e.key === 'k')     { e.preventDefault(); focusRow(focusedRow - 1); }
    else if (e.key === 'Enter') { e.preventDefault(); openFocusedRow(); }
    else if (e.key === '/')     { e.preventDefault(); const f = document.getElementById('filterInput'); if (f) { f.focus(); f.select(); } }
    else if (e.key === 'c')     { e.preventDefault(); compareSelected(); }
    else if (e.key === 'd')     { e.preventDefault(); deleteSelected(); }
  });

  const checkAll     = document.getElementById('checkAll');
  const compareHint  = document.getElementById('compareHint');
  const deleteSelBtn = document.getElementById('deleteSelectedBtn');

  function updateSelectionUI() {
    const n = document.querySelectorAll('.runs-table input[type=checkbox]:checked').length;
    if (compareHint)  compareHint.hidden  = n < 2;
    if (deleteSelBtn) deleteSelBtn.hidden = n < 1;
  }

  if (checkAll) {
    checkAll.addEventListener('change', () => {
      document.querySelectorAll('.runs-table tbody input[type=checkbox]')
        .forEach(cb => { cb.checked = checkAll.checked; });
      updateSelectionUI();
    });
  }

  if (deleteSelBtn) deleteSelBtn.addEventListener('click', deleteSelected);

  document.addEventListener('change', e => {
    if (e.target.closest('.runs-table') && e.target.type === 'checkbox') {
      updateSelectionUI();
    }
  });
})();
