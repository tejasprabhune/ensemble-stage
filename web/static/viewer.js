// Trace viewer for Stage. Polls /v1/runs/{id}/events?since={seq} for live runs.

(function () {
  const runId = window.STAGE_RUN_ID;

  const els = {
    slider:       document.getElementById('slider'),
    playBtn:      document.getElementById('playBtn'),
    prevBtn:      document.getElementById('prevBtn'),
    nextBtn:      document.getElementById('nextBtn'),
    tickLabel:    document.getElementById('tickLabel'),
    messages:     document.getElementById('messagesPanel'),
    tools:        document.getElementById('toolsPanel'),
    diff:         document.getElementById('diffPanel'),
    timeline:     document.getElementById('timelineStage'),
    chat:         document.getElementById('chatStage'),
    chatFeed:     document.getElementById('chatFeed'),
    tabTimeline:  document.getElementById('tabTimeline'),
    tabChat:      document.getElementById('tabChat'),
    traceLabel:   document.getElementById('traceLabel'),
    actorsData:   document.getElementById('actorsData'),
    predicates:   document.getElementById('predicatesData'),
    hiddenState:  document.getElementById('hiddenStateData'),
  };

  let events = [];
  let actors = [];
  let actorKinds = {};
  let actorMeta = {};
  let position = 0;
  let playTimer = null;
  let pollTimer = null;
  let stuckToTail = true;
  let view = 'timeline';
  let lastSeq = -1;
  let runStatus = 'running';

  async function load() {
    if (!runId) { showError('no run ID on page'); return; }
    try {
      const res = await fetch(`/v1/runs/${runId}`);
      if (!res.ok) { showError(`run fetch failed: ${res.status}`); return; }
      const run = await res.json();
      runStatus = run.status || 'running';
    } catch (err) {
      showError('could not load run: ' + err.message);
      return;
    }

    await fetchEvents();
    setPosition(events.length > 0 ? events.length - 1 : 0);

    if (runStatus === 'running' || runStatus === 'queued') {
      startPolling();
    }
  }

  async function fetchEvents() {
    try {
      const url = `/v1/runs/${runId}/events?since=${lastSeq}`;
      const res = await fetch(url);
      if (!res.ok) return;
      const data = await res.json();
      if (!Array.isArray(data) || data.length === 0) return;

      for (const ev of data) {
        events.push(convertEvent(ev));
        if (ev.sequence_number > lastSeq) lastSeq = ev.sequence_number;
      }

      actors = collectActors(events);
      actorKinds = inferActorKinds(events);
      actorMeta = collectActorMeta(events);
      updateSidebar(events);
      els.slider.max = String(Math.max(events.length - 1, 0));

      if (els.traceLabel) {
        els.traceLabel.textContent = `${events.length} events  ·  ${actors.length} actors`;
      }
    } catch (_) {}
  }

  // Stage API stores the full serialized Event struct as ev.payload:
  //   ev.payload.actor        - actor ID
  //   ev.payload.tick         - tick number
  //   ev.payload.ts_ms        - timestamp
  //   ev.payload.payload      - the actual EventPayload (kind, name, text, args, diff, ...)
  // We flatten this into the viewer's internal format.
  function convertEvent(ev) {
    const outer = ev.payload || {};
    const inner = outer.payload || {};
    const kind = ev.kind || inner.kind || '';
    return {
      tick: ev.sequence_number,
      actor: outer.actor || '',
      tsMs: outer.ts_ms,
      payload: Object.assign({}, inner, { kind }),
    };
  }

  function startPolling() {
    if (pollTimer) return;
    pollTimer = setInterval(async () => {
      await fetchEvents();
      if (events.length === 0) return;

      try {
        const res = await fetch(`/v1/runs/${runId}`);
        if (res.ok) {
          const run = await res.json();
          runStatus = run.status || runStatus;
          if (runStatus !== 'running' && runStatus !== 'queued') {
            clearInterval(pollTimer);
            pollTimer = null;
          }
        }
      } catch (_) {}

      if (stuckToTail) {
        setPosition(events.length - 1);
      } else {
        els.slider.max = String(Math.max(events.length - 1, 0));
        els.tickLabel.textContent = `${position + 1} / ${events.length}`;
      }
    }, 2000);
  }

  function collectActors(evs) {
    const seen = new Set();
    const order = [];
    for (const e of evs) {
      if (e.actor && !seen.has(e.actor)) {
        seen.add(e.actor);
        order.push(e.actor);
      }
    }
    return order;
  }

  function inferActorKinds(evs) {
    const kinds = {};
    for (const e of evs) {
      if (!e.actor) continue;
      const k = e.payload && e.payload.kind;
      if (k === 'agent_message' || k === 'tool_call' || k === 'tool_result') {
        kinds[e.actor] = 'agent';
      } else if (k === 'user_message' && !kinds[e.actor]) {
        kinds[e.actor] = 'user';
      }
    }
    return kinds;
  }

  // Parse user_spawned / agent_spawned events from system notes to collect
  // model, persona, hidden_state per actor.
  function collectActorMeta(evs) {
    const meta = {};
    for (const e of evs) {
      if (e.payload.kind !== 'system') continue;
      const note = e.payload.note;
      if (typeof note !== 'string' || !note.startsWith('{')) continue;
      let obj;
      try { obj = JSON.parse(note); } catch { continue; }
      const k = obj.kind;
      if ((k === 'user_spawned' || k === 'agent_spawned') && obj.actor_id) {
        meta[obj.actor_id] = {
          kind: k === 'user_spawned' ? 'user' : 'agent',
          model: obj.model,
          persona: obj.persona,
          hidden_state: obj.hidden_state,
          tools: obj.tools,
        };
      }
    }
    return meta;
  }

  function updateSidebar(evs) {
    updateActorsSidebar();
    updatePredicatesSidebar(evs);
    updateHiddenStateSidebar();
  }

  function updateActorsSidebar() {
    if (!els.actorsData) return;
    const allActors = new Set([...actors, ...Object.keys(actorMeta)]);
    if (allActors.size === 0) return;
    const rows = [];
    for (const id of allActors) {
      const m = actorMeta[id] || {};
      const kind = m.kind || actorKinds[id] || 'unknown';
      const badge = `<span class="actor-kind-badge ${kind}">${esc(kind)}</span>`;
      const detail = [m.model, m.persona].filter(Boolean).map(esc).join(' / ');
      rows.push(`<dt>${esc(id)} ${badge}</dt><dd class="mono">${detail || '—'}</dd>`);
    }
    els.actorsData.innerHTML = rows.join('');
    els.actorsData.classList.remove('muted');
  }

  function updatePredicatesSidebar(evs) {
    if (!els.predicates) return;
    for (const e of evs) {
      if (e.payload.kind !== 'system') continue;
      const note = e.payload.note;
      if (typeof note !== 'string' || !note.includes('"grader"')) continue;
      let obj;
      try { obj = JSON.parse(note); } catch { continue; }
      if (obj.kind !== 'grader' || !obj.scores) continue;
      const rows = [];
      for (const [k, v] of Object.entries(obj.scores)) {
        const cls = v >= 1.0 ? 'score-pass' : v <= 0.0 ? 'score-fail' : 'score-partial';
        rows.push(`<div class="${cls}">${esc(k)}: ${Number(v).toFixed(2)}</div>`);
      }
      els.predicates.innerHTML = rows.join('');
      els.predicates.classList.remove('muted');
      return;
    }
  }

  function updateHiddenStateSidebar() {
    if (!els.hiddenState) return;
    const rows = [];
    for (const [id, m] of Object.entries(actorMeta)) {
      if (!m.hidden_state || typeof m.hidden_state !== 'object') continue;
      const entries = Object.entries(m.hidden_state);
      if (entries.length === 0) continue;
      rows.push(`<div class="hs-actor">${esc(id)}</div>`);
      for (const [k, v] of entries) {
        rows.push(`<div class="hs-row"><span class="hs-key">${esc(k)}</span> <span class="hs-val">${esc(str(v))}</span></div>`);
      }
    }
    if (rows.length > 0) {
      els.hiddenState.innerHTML = rows.join('');
      els.hiddenState.classList.remove('muted');
    }
  }

  function setPosition(p) {
    position = clamp(p, 0, Math.max(events.length - 1, 0));
    els.slider.value = String(position);
    stuckToTail = position === Math.max(events.length - 1, 0);
    els.tickLabel.textContent = `${position + 1} / ${events.length}`;
    render();
  }

  function clamp(v, lo, hi) { return v < lo ? lo : v > hi ? hi : v; }

  function render() {
    const upTo = events.slice(0, position + 1);
    if (view === 'timeline') {
      renderMessages(upTo);
      renderTools(upTo);
      renderDiff(upTo);
    } else {
      renderChat(upTo);
    }
  }

  function renderMessages(slice) {
    const cols = {};
    for (const a of actors) cols[a] = [];
    for (const e of slice) {
      const k = e.payload && e.payload.kind;
      if (k !== 'user_message' && k !== 'agent_message') continue;
      const who = e.actor || '?';
      (cols[who] = cols[who] || []).push({ kind: k, text: e.payload.text || '', tick: e.tick });
    }
    const html = ['<h4>actors</h4>'];
    for (const a of actors) {
      const msgs = cols[a] || [];
      html.push(`<div class="actor-col">`);
      html.push(`<div class="actor-msg" style="border-color:var(--rule)"><div class="who">${esc(a)}</div></div>`);
      for (const m of msgs.slice(-5)) {
        html.push(`<div class="actor-msg"><div class="who">${esc(a)} | tick ${m.tick}</div><div class="text">${esc(m.text)}</div></div>`);
      }
      html.push(`</div>`);
    }
    if (actors.length === 0) html.push('<div class="who">no actors yet</div>');
    els.messages.innerHTML = html.join('');
    highlight(els.messages);
  }

  function renderTools(slice) {
    const html = ['<h4>tool calls</h4>'];
    for (const e of slice) {
      const k = e.payload && e.payload.kind;
      const seed = e.payload && e.payload.seed === true;
      const seedTag = seed ? ' <span class="seed-tag">[seed]</span>' : '';
      const cls = seed ? 'tool-line seeded' : 'tool-line';
      if (k === 'tool_call') {
        html.push(`<div class="${cls}"><div><span class="name">${esc(e.payload.name || '?')}</span>${seedTag} <span class="who">[call] ${esc(e.actor || 'system')} | tick ${e.tick}</span></div><div class="args">${renderArgs(e.payload.args)}</div></div>`);
      } else if (k === 'tool_result') {
        html.push(`<div class="${cls}"><div><span class="name">${esc(e.payload.name || '?')}</span>${seedTag} <span class="who">[result]</span></div><div class="res">${renderResult(e.payload.result)}</div></div>`);
      }
    }
    if (html.length === 1) html.push(`<div class="who">no tool activity yet</div>`);
    els.tools.innerHTML = html.join('');
    highlight(els.tools);
  }

  function renderArgs(args) {
    if (args == null || typeof args !== 'object') return esc(str(args));
    const parts = [];
    for (const [k, v] of Object.entries(args)) {
      if (typeof v === 'string' && v.length > 80) {
        parts.push(`<details class="arg-long"><summary><code>${esc(k)}</code> <span class="who">(${v.length} chars)</span></summary><pre><code>${esc(v)}</code></pre></details>`);
      } else {
        parts.push(`<div><code>${esc(k)}</code> = ${esc(str(v))}</div>`);
      }
    }
    return parts.join('');
  }

  function renderResult(result) {
    if (result && typeof result === 'object') {
      const summary = result.summary || (result.effect && result.effect.summary);
      if (typeof summary === 'string') return `<div class="res-summary">${escMulti(summary)}</div>`;
    }
    return esc(compactJson(result, 300));
  }

  function renderDiff(slice) {
    const rows = [];
    for (const e of slice) {
      const k = e.payload && e.payload.kind;
      if (k !== 'state_diff' || !e.payload.diff) continue;
      // diff is an array of {table, field, old, new, row_id} entries
      const diffs = Array.isArray(e.payload.diff) ? e.payload.diff : [e.payload.diff];
      for (const d of diffs) {
        rows.push({
          who: e.actor || '?',
          table: d.table || '',
          field: d.field || '',
          old: str(d.old),
          new: str(d.new),
        });
      }
    }
    const html = ['<h4>state changes</h4>'];
    if (rows.length === 0) {
      html.push(`<div class="who">no state changes yet</div>`);
    } else {
      html.push('<table class="diff"><thead><tr><th>actor</th><th>table</th><th>field</th><th>old</th><th>new</th></tr></thead><tbody>');
      for (const r of rows.slice(-12)) {
        html.push(`<tr><td>${esc(r.who)}</td><td>${esc(r.table)}</td><td>${esc(r.field)}</td><td>${esc(r.old)}</td><td>${esc(r.new)}</td></tr>`);
      }
      html.push('</tbody></table>');
    }
    els.diff.innerHTML = html.join('');
  }

  function renderChat(slice) {
    const html = [];
    for (const e of slice) {
      const p = e.payload || {};
      const k = p.kind;
      const who = e.actor || '';
      const side = actorKinds[who] === 'user' ? 'user' : 'agent';
      if (k === 'user_message') {
        html.push(bubble(who, p.text || '', 'user', e.tick));
      } else if (k === 'agent_message') {
        html.push(bubble(who, p.text || '', 'agent', e.tick));
      } else if (k === 'tool_call') {
        html.push(`<div class="chat-tool ${side === 'user' ? 'right' : 'left'}"><span class="chat-tool-tag">[tool]</span> <strong>${esc(who)}</strong> called <code>${esc(p.name || '?')}</code><div class="chat-tool-args">${renderArgs(p.args)}</div></div>`);
      } else if (k === 'tool_result') {
        html.push(`<div class="chat-tool ${side === 'user' ? 'right' : 'left'}"><span class="chat-tool-tag">[result]</span> <code>${esc(p.name || '?')}</code><div class="chat-tool-res">${renderResult(p.result)}</div></div>`);
      } else if (k === 'cost') {
        html.push(`<div class="chat-note">[cost] ${esc(who)} +${esc(str(p.amount))} ${esc(String(p.unit || ''))} (total ${esc(str(p.running_total))})</div>`);
      } else if (k === 'state_diff' && p.diff) {
        const diffs = Array.isArray(p.diff) ? p.diff : [p.diff];
        for (const d of diffs) {
          html.push(`<div class="chat-note">[state] ${esc(`${d.table || ''}${d.table && d.field ? '.' : ''}${d.field || ''}: ${str(d.old)} → ${str(d.new)}`)}</div>`);
        }
      } else if (k === 'system' && p.note) {
        let obj = null;
        if (typeof p.note === 'string' && p.note.startsWith('{')) {
          try { obj = JSON.parse(p.note); } catch { }
        }
        if (obj && (obj.kind === 'user_spawned' || obj.kind === 'agent_spawned' || obj.kind === 'grader')) {
          // skip spawn/grader notes in chat view - they're in the sidebar
        } else {
          const text = (obj && obj.note) || p.note;
          if (text) html.push(`<div class="chat-note">${esc(text)}</div>`);
        }
      }
    }
    if (html.length === 0) html.push('<div class="chat-note">no events yet</div>');
    els.chatFeed.innerHTML = html.join('');
    highlight(els.chatFeed);
    els.chatFeed.scrollTop = els.chatFeed.scrollHeight;
  }

  function bubble(who, text, side, tick) {
    const sc = side === 'user' ? 'right' : 'left';
    return `<div class="chat-row ${sc}"><div class="chat-bubble ${sc}"><div class="chat-meta">${esc(who)} | tick ${tick}</div><div class="chat-text">${renderMarkdown(text)}</div></div></div>`;
  }

  function renderMarkdown(text) {
    if (text == null) return '';
    const parts = [];
    const re = /```([a-zA-Z0-9_+\-]*)\n([\s\S]*?)```/g;
    let last = 0, m;
    while ((m = re.exec(text)) !== null) {
      parts.push(renderInline(text.slice(last, m.index)));
      const lang = m[1] || '';
      parts.push(`<pre${lang ? ` data-lang="${esc(lang)}"` : ''}><code${lang ? ` class="language-${esc(lang)}"` : ''}>${esc(m[2])}</code></pre>`);
      last = re.lastIndex;
    }
    parts.push(renderInline(text.slice(last)));
    return parts.join('');
  }

  function renderInline(text) {
    if (!text) return '';
    return esc(text).replace(/`([^`]+)`/g, (_m, body) => `<code>${body}</code>`).replaceAll('\n', '<br>');
  }

  function highlight(root) {
    const hljs = window.hljs;
    if (!hljs || typeof hljs.highlightElement !== 'function') return;
    for (const el of (root || document).querySelectorAll('pre > code')) {
      try { hljs.highlightElement(el); } catch (_) {}
    }
  }

  function compactJson(v, max) {
    if (v == null) return '';
    let s; try { s = JSON.stringify(v); } catch (_) { s = String(v); }
    return s && s.length > max ? s.slice(0, max) + '...' : (s || '');
  }

  function str(v) {
    if (v == null) return '';
    if (typeof v === 'string') return v;
    try { return JSON.stringify(v); } catch (_) { return String(v); }
  }

  function esc(s) {
    return String(s).replaceAll('&', '&amp;').replaceAll('<', '&lt;').replaceAll('>', '&gt;');
  }

  function escMulti(s) { return esc(s).replaceAll('\n', '<br>'); }

  function showError(msg) {
    if (els.messages) els.messages.innerHTML = `<h4>actors</h4><div class="who">${esc(msg)}</div>`;
    if (els.tools)    els.tools.innerHTML    = '<h4>tool calls</h4>';
    if (els.diff)     els.diff.innerHTML     = '<h4>state changes</h4>';
    if (els.chatFeed) els.chatFeed.innerHTML = `<div class="chat-note">${esc(msg)}</div>`;
  }

  els.slider.addEventListener('input',  () => setPosition(Number(els.slider.value)));
  els.prevBtn.addEventListener('click', () => setPosition(position - 1));
  els.nextBtn.addEventListener('click', () => setPosition(position + 1));
  els.playBtn.addEventListener('click', () => {
    if (playTimer) {
      clearInterval(playTimer); playTimer = null; els.playBtn.textContent = 'play'; return;
    }
    els.playBtn.textContent = 'pause';
    if (position >= events.length - 1) setPosition(0);
    playTimer = setInterval(() => {
      if (position >= events.length - 1) {
        clearInterval(playTimer); playTimer = null; els.playBtn.textContent = 'play'; return;
      }
      setPosition(position + 1);
    }, 1000);
  });

  if (els.tabTimeline && els.tabChat) {
    els.tabTimeline.addEventListener('click', () => switchView('timeline'));
    els.tabChat.addEventListener('click',     () => switchView('chat'));
  }

  function switchView(next) {
    view = next;
    if (els.tabTimeline) els.tabTimeline.classList.toggle('active', next === 'timeline');
    if (els.tabChat)     els.tabChat.classList.toggle('active', next === 'chat');
    if (els.timeline)    els.timeline.style.display = next === 'timeline' ? '' : 'none';
    if (els.chat)        els.chat.style.display     = next === 'chat' ? '' : 'none';
    render();
  }

  load();
})();
