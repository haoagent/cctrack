// cctrack Premium Web Dashboard
(function () {
  'use strict';

  var state = {
    snapshot: null,
    stats: null,
    teamIndex: 0,
    bottomTab: 'activity',
    theme: localStorage.getItem('cctrack-theme') || 'dark',
    charts: { token: null, cost: null, project: null }
  };

  document.documentElement.setAttribute('data-theme', state.theme);
  updateThemeIcon();

  // ─── SSE ───
  function connectSSE() {
    var es = new EventSource('/api/sse');
    es.onmessage = function (e) {
      try { state.snapshot = JSON.parse(e.data); renderAll(); }
      catch (err) { console.error('SSE parse error:', err); }
    };
    es.onerror = function () { es.close(); setTimeout(connectSSE, 3000); };
  }
  connectSSE();

  // ─── Stats ───
  function fetchStats() {
    fetch('/api/stats')
      .then(function (r) { return r.json(); })
      .then(function (d) { state.stats = d; renderStats(); renderCharts(); })
      .catch(function () {});
  }
  fetchStats();
  setInterval(fetchStats, 60000);

  // ─── Theme ───
  document.getElementById('theme-toggle').onclick = function () {
    state.theme = state.theme === 'dark' ? 'light' : 'dark';
    document.documentElement.setAttribute('data-theme', state.theme);
    localStorage.setItem('cctrack-theme', state.theme);
    updateThemeIcon();
    renderCharts();
  };

  function updateThemeIcon() {
    var el = document.getElementById('theme-icon');
    if (el) el.innerHTML = state.theme === 'dark' ? '&#9790;' : '&#9728;';
  }

  // ─── Panel Tabs ───
  document.querySelectorAll('.panel-tab').forEach(function (btn) {
    btn.onclick = function () {
      document.querySelectorAll('.panel-tab').forEach(function (b) { b.classList.remove('active'); });
      btn.classList.add('active');
      state.bottomTab = btn.dataset.tab;
      renderBottomPanel();
    };
  });

  // ─── Render All ───
  function renderAll() {
    if (!state.snapshot || !state.snapshot.teams || !state.snapshot.teams.length) return;
    if (state.teamIndex >= state.snapshot.teams.length) state.teamIndex = 0;
    var team = state.snapshot.teams[state.teamIndex];

    renderHeader(team);
    renderTeamTabs(state.snapshot.teams);
    renderSessions(team);
    renderRightPanel(team);
    renderBottomPanel();
  }

  // ─── Header ───
  function renderHeader(team) {
    var agents = team.agents || [];
    var activeCount = agents.filter(function (a) { return a.status === 'Active'; }).length;
    var isAll = team.name === 'all';

    setText('stat-active', activeCount);
    setText('stat-sessions', agents.length);
    setText('stat-sessions-label', isAll ? 'sessions' : 'agents');

    // Active dot glow
    var dot = document.getElementById('stat-active-dot');
    if (dot) { dot.className = 'stat-mini-dot' + (activeCount > 0 ? ' has-active' : ''); }

    var totalTokens = 0, totalCost = 0, totalCache = 0, totalInput = 0;
    agents.forEach(function (a) {
      if (a.tokens) {
        totalTokens += tokenTotal(a.tokens);
        totalCost += estimateCost(a.tokens);
        totalCache += (a.tokens.cache_read_tokens || 0);
        totalInput += (a.tokens.input_tokens || 0) + (a.tokens.cache_read_tokens || 0);
      }
    });
    setText('stat-tokens', formatTokens(totalTokens));
    setText('stat-cost', '$' + totalCost.toFixed(2));

    // Cache hit rate
    var cacheRate = totalInput > 0 ? Math.round(totalCache / totalInput * 100) : 0;
    setText('stat-cache', cacheRate + '%');

    // Today's cost from stats
    if (state.stats && state.stats.today) {
      setText('stat-today-cost', 'today $' + state.stats.today.cost_usd.toFixed(2));
    }
  }

  // ─── Team Tabs ───
  function renderTeamTabs(teams) {
    var el = document.getElementById('team-tabs');
    el.innerHTML = teams.map(function (t, i) {
      var active = i === state.teamIndex;
      var hasActive = (t.agents || []).some(function (a) { return a.status === 'Active'; });
      var dotCls = hasActive ? 'active' : 'idle';
      var name = t.name.replace(/^session:/, '').toUpperCase();
      return '<button class="team-tab' + (active ? ' active' : '') + '" data-i="' + i + '">' +
        '<span class="dot ' + dotCls + '"></span>' + esc(name) + '</button>';
    }).join('');

    el.querySelectorAll('.team-tab').forEach(function (btn) {
      btn.onclick = function () {
        state.teamIndex = parseInt(btn.dataset.i, 10);
        renderAll();
        if (state.stats) { renderStats(); renderCharts(); }
      };
    });
  }

  // ─── Sessions Table ───
  function renderSessions(team) {
    var isAll = team.name === 'all';
    setText('sessions-title', isAll ? 'Sessions' : 'Agents');

    var agents = team.agents || [];
    var tbody = document.getElementById('sessions-body');
    var empty = document.getElementById('sessions-empty');

    if (agents.length === 0) {
      tbody.innerHTML = '';
      empty.style.display = 'block';
      return;
    }
    empty.style.display = 'none';

    tbody.innerHTML = agents.map(function (a) {
      var status = (a.status || 'unknown').toLowerCase();
      var tokens = a.tokens ? tokenTotal(a.tokens) : 0;
      var cost = a.tokens ? estimateCost(a.tokens) : 0;
      var model = a.model ? shortModel(a.model) : '';
      var subBadge = a.sub_agent_count && a.sub_agent_count > 0
        ? '<span class="badge">' + a.sub_agent_count + ' sub</span>' : '';

      return '<tr class="clickable">' +
        '<td>' + esc(a.name) + subBadge + '</td>' +
        '<td><span class="status-dot ' + status + '"></span><span class="status-text ' + status + '">' + esc(capitalize(a.status)) + '</span></td>' +
        '<td class="right muted model-badge">' + esc(model) + '</td>' +
        '<td class="right mono token-hero">' + (tokens > 0 ? formatTokens(tokens) : '\u2014') + '</td>' +
        '<td class="right mono cost-hero">' + (cost > 0 ? '$' + cost.toFixed(2) : '\u2014') + '</td>' +
        '</tr>';
    }).join('');
  }

  // ─── Right Panel: Stats or Todos ───
  function renderRightPanel(team) {
    var isAll = team.name === 'all';
    var statsTable = document.getElementById('stats-table');
    var todosTable = document.getElementById('todos-table');
    var empty = document.getElementById('right-empty');

    if (isAll) {
      setText('right-title', 'Stats');
      statsTable.style.display = '';
      todosTable.style.display = 'none';
      empty.style.display = 'none';
      renderStats();
    } else {
      setText('right-title', 'Todos');
      statsTable.style.display = 'none';
      todosTable.style.display = '';
      renderTodos(team.todos || []);
    }
  }

  // ─── Stats Table ───
  function renderStats() {
    var body = document.getElementById('stats-body');
    if (!state.stats) {
      body.innerHTML = '<tr><td colspan="4" class="muted" style="text-align:center">Loading...</td></tr>';
      return;
    }

    var s = state.stats;
    var periods = [s.today, s.this_week, s.this_month, s.total];
    var html = periods.map(function (p) {
      var cls = p.label === 'Total' ? ' bold' : '';
      return '<tr>' +
        '<td class="' + cls + '">' + esc(p.label) + '</td>' +
        '<td class="right muted">' + p.sessions + '</td>' +
        '<td class="right mono token-hero">' + formatTokens(p.total_tokens) + '</td>' +
        '<td class="right mono cost-hero">' + '$' + p.cost_usd.toFixed(2) + '</td>' +
        '</tr>';
    }).join('');

    if (s.by_project && s.by_project.length > 0) {
      html += '<tr><td colspan="4" class="bold" style="padding-top:12px;border-bottom:none">By Project</td></tr>';
      html += s.by_project.slice(0, 8).map(function (p) {
        return '<tr>' +
          '<td class="muted">' + esc(p.label) + '</td>' +
          '<td class="right muted">' + p.sessions + '</td>' +
          '<td class="right mono token-hero">' + formatTokens(p.total_tokens) + '</td>' +
          '<td class="right mono cost-hero">$' + p.cost_usd.toFixed(2) + '</td>' +
          '</tr>';
      }).join('');
    }
    body.innerHTML = html;
  }

  // ─── Todos ───
  function renderTodos(todos) {
    var body = document.getElementById('todos-body');
    var empty = document.getElementById('right-empty');

    if (todos.length === 0) {
      body.innerHTML = '';
      empty.textContent = 'No active todos';
      empty.style.display = 'block';
      return;
    }
    empty.style.display = 'none';

    body.innerHTML = todos.map(function (t) {
      var status = t.status || 'pending';
      var sym = { completed: '\u2713', in_progress: '\u25cf', pending: '\u25cb', blocked: '\u2298' }[status] || '?';
      var label = { completed: 'done', in_progress: 'running', pending: 'pending', blocked: 'blocked' }[status] || status;
      var text = (status === 'in_progress' && t.active_form) ? t.active_form : t.content;

      return '<tr>' +
        '<td class="task-' + status + '">' + sym + ' ' + esc(label) + '</td>' +
        '<td>' + esc(text) + '</td>' +
        '</tr>';
    }).join('');
  }

  // ─── Bottom Panel ───
  function renderBottomPanel() {
    if (!state.snapshot || !state.snapshot.teams) return;
    var team = state.snapshot.teams[state.teamIndex] || state.snapshot.teams[0];
    var isAll = team.name === 'all';
    var el = document.getElementById('panel-content');

    switch (state.bottomTab) {
      case 'activity': renderActivity(el, team.tool_events || [], isAll); break;
      case 'messages': renderMessages(el, team.messages || []); break;
    }
  }

  function renderActivity(el, events, isAll) {
    // Filter out startup_scan noise
    var filtered = events.filter(function (e) { return e.tool_name !== 'startup_scan'; });
    if (filtered.length === 0) {
      el.innerHTML = '<div class="empty-state">No activity yet. Run <code>cctrack hooks install</code> to enable.</div>';
      return;
    }
    var recent = filtered.slice(-100);
    el.innerHTML = recent.map(function (ev) {
      var time = formatTime(ev.timestamp);
      var tool = ev.tool_name || '?';
      var toolCls = 'tool-' + tool.toLowerCase();
      var summary = ev.summary || '';
      var dur = ev.duration_ms ? ev.duration_ms + 'ms' : '';
      var agent = '';
      if (isAll && ev.cwd) {
        var parts = ev.cwd.split('/');
        agent = parts[parts.length - 1] || '';
      }

      return '<div class="feed-item">' +
        '<span class="feed-time">' + esc(time) + '</span>' +
        (agent ? '<span class="feed-agent">' + esc(agent) + '</span>' : '') +
        '<span class="feed-tool ' + toolCls + '">' + esc(tool) + '</span>' +
        '<span class="feed-summary">' + esc(truncate(summary, 120)) + '</span>' +
        (dur ? '<span class="feed-duration">' + dur + '</span>' : '') +
        '</div>';
    }).join('');
    el.scrollTop = el.scrollHeight;
  }

  function renderMessages(el, messages) {
    var filtered = messages.filter(function (m) { return m.msg_type !== 'idle_notification'; });
    if (filtered.length === 0) {
      el.innerHTML = '<div class="empty-state">No messages yet</div>';
      return;
    }
    el.innerHTML = filtered.slice(-50).map(function (m) {
      var time = formatTime(m.timestamp);
      var summary = m.summary || m.text || '';
      return '<div class="feed-item">' +
        '<span class="feed-time">' + esc(time) + '</span>' +
        '<span class="msg-from">' + esc(m.from) + '</span>' +
        '<span class="msg-arrow">\u2192</span>' +
        '<span class="msg-to">' + esc(m.to) + '</span>' +
        '<span class="msg-text">' + esc(truncate(summary, 100)) + '</span>' +
        '</div>';
    }).join('');
    el.scrollTop = el.scrollHeight;
  }

  // ─── Charts ───
  function renderCharts() {
    if (!state.stats) return;
    var isDark = state.theme === 'dark';
    var textColor = isDark ? '#94a3b8' : '#64748b';
    var gridColor = isDark ? 'rgba(255,255,255,0.05)' : 'rgba(0,0,0,0.06)';
    Chart.defaults.color = textColor;
    Chart.defaults.borderColor = gridColor;

    renderTokenChart(state.stats.daily || [], gridColor, isDark);
    renderCostChart(state.stats.daily || [], gridColor, isDark);
    renderProjectChart(state.stats.by_project || [], isDark);
  }

  function renderTokenChart(daily, gridColor, isDark) {
    if (state.charts.token) state.charts.token.destroy();
    var ctx = document.getElementById('token-chart');
    if (!ctx || daily.length === 0) return;

    state.charts.token = new Chart(ctx, {
      type: 'line',
      data: {
        labels: daily.map(function (d) { return d.date.slice(5); }),
        datasets: [
          { label: 'Input', data: daily.map(function (d) { return d.input_tokens; }),
            borderColor: isDark ? '#60a5fa' : '#2563eb', backgroundColor: isDark ? 'rgba(96,165,250,0.08)' : 'rgba(37,99,235,0.08)',
            fill: true, tension: 0.4, pointRadius: 2, pointHoverRadius: 4, borderWidth: 2 },
          { label: 'Output', data: daily.map(function (d) { return d.output_tokens; }),
            borderColor: isDark ? '#a78bfa' : '#7c3aed', backgroundColor: isDark ? 'rgba(167,139,250,0.08)' : 'rgba(124,58,237,0.08)',
            fill: true, tension: 0.4, pointRadius: 2, pointHoverRadius: 4, borderWidth: 2 },
          { label: 'Cache', data: daily.map(function (d) { return d.cache_tokens; }),
            borderColor: isDark ? '#22d3ee' : '#0891b2', backgroundColor: isDark ? 'rgba(34,211,238,0.06)' : 'rgba(8,145,178,0.06)',
            fill: true, tension: 0.4, pointRadius: 2, pointHoverRadius: 4, borderWidth: 2 }
        ]
      },
      options: chartOptions(gridColor, function (v) { return formatTokens(v); })
    });
  }

  function renderCostChart(daily, gridColor, isDark) {
    if (state.charts.cost) state.charts.cost.destroy();
    var ctx = document.getElementById('cost-chart');
    if (!ctx || daily.length === 0) return;

    state.charts.cost = new Chart(ctx, {
      type: 'bar',
      data: {
        labels: daily.map(function (d) { return d.date.slice(5); }),
        datasets: [{
          label: 'Cost',
          data: daily.map(function (d) { return Math.round(d.cost_usd * 100) / 100; }),
          backgroundColor: isDark ? 'rgba(99,102,241,0.5)' : 'rgba(99,102,241,0.6)',
          borderColor: isDark ? '#6366f1' : '#4f46e5',
          borderWidth: 1, borderRadius: 3, barPercentage: 0.7
        }]
      },
      options: chartOptions(gridColor, function (v) { return '$' + v; }, true)
    });
  }

  function renderProjectChart(projects, isDark) {
    if (state.charts.project) state.charts.project.destroy();
    var ctx = document.getElementById('project-chart');
    if (!ctx || projects.length === 0) return;

    var top = projects.slice(0, 6);
    var colors = isDark
      ? ['#6366f1', '#8b5cf6', '#a78bfa', '#60a5fa', '#22d3ee', '#34d399']
      : ['#4f46e5', '#7c3aed', '#8b5cf6', '#2563eb', '#0891b2', '#059669'];

    state.charts.project = new Chart(ctx, {
      type: 'bar',
      data: {
        labels: top.map(function (p) { return p.label; }),
        datasets: [{
          data: top.map(function (p) { return Math.round(p.cost_usd * 100) / 100; }),
          backgroundColor: colors.slice(0, top.length),
          borderWidth: 0, borderRadius: 4, barPercentage: 0.6
        }]
      },
      options: {
        indexAxis: 'y',
        responsive: true, maintainAspectRatio: false,
        plugins: {
          legend: { display: false },
          tooltip: { callbacks: { label: function (c) { return '$' + c.raw.toFixed(2); } } }
        },
        scales: {
          x: { grid: { color: isDark ? 'rgba(255,255,255,0.05)' : 'rgba(0,0,0,0.06)' }, ticks: { font: { size: 10 }, callback: function (v) { return '$' + v; } } },
          y: { grid: { display: false }, ticks: { font: { size: 11 } } }
        }
      }
    });
  }

  function chartOptions(gridColor, tickFmt, noLegend) {
    return {
      responsive: true, maintainAspectRatio: false,
      interaction: { mode: 'index', intersect: false },
      plugins: {
        legend: noLegend ? { display: false } : { position: 'top', labels: { boxWidth: 12, padding: 10, font: { size: 11 } } },
        tooltip: { callbacks: { label: function (c) { return c.dataset.label + ': ' + tickFmt(c.raw); } } }
      },
      scales: {
        x: { grid: { display: false }, ticks: { font: { size: 10 }, maxRotation: 0, maxTicksLimit: 10 } },
        y: { grid: { color: gridColor }, ticks: { font: { size: 10 }, callback: tickFmt } }
      }
    };
  }

  // ─── Helpers ───
  function tokenTotal(t) {
    if (!t) return 0;
    return (t.input_tokens || 0) + (t.output_tokens || 0) + (t.cache_read_tokens || 0) +
      (t.cache_create_5m_tokens || 0) + (t.cache_create_1h_tokens || 0);
  }

  function estimateCost(t) {
    if (!t) return 0;
    if (t.cost_usd && t.cost_usd > 0) return t.cost_usd;
    return (t.input_tokens || 0) / 1e6 * 5 + (t.output_tokens || 0) / 1e6 * 25 +
      (t.cache_read_tokens || 0) / 1e6 * 0.5 + (t.cache_create_5m_tokens || 0) / 1e6 * 6.25 +
      (t.cache_create_1h_tokens || 0) / 1e6 * 10;
  }

  function formatTokens(n) {
    if (n >= 1e9) return (n / 1e9).toFixed(1) + 'B';
    if (n >= 1e6) return (n / 1e6).toFixed(1) + 'M';
    if (n >= 1e3) return (n / 1e3).toFixed(0) + 'K';
    return String(n || 0);
  }

  function shortModel(m) {
    var l = m.toLowerCase();
    if (l.indexOf('opus') >= 0) return 'opus';
    if (l.indexOf('sonnet') >= 0) return 'sonnet';
    if (l.indexOf('haiku') >= 0) return 'haiku';
    return m.length > 10 ? m.slice(-8) : m;
  }

  function formatTime(ts) {
    if (!ts) return '--:--';
    try { return new Date(ts).toLocaleTimeString('en-US', { hour12: false }); }
    catch (e) { return '--:--'; }
  }

  function capitalize(s) { return s ? s.charAt(0).toUpperCase() + s.slice(1).toLowerCase() : ''; }
  function truncate(s, max) { return s && s.length > max ? s.slice(0, max) + '...' : (s || ''); }
  function setText(id, v) { var el = document.getElementById(id); if (el) el.textContent = v; }
  function esc(s) {
    if (!s) return '';
    var d = document.createElement('div');
    d.appendChild(document.createTextNode(String(s)));
    return d.innerHTML;
  }
})();
