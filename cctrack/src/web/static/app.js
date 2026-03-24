// cctrack Premium Web Dashboard
(function () {
  'use strict';

  // ─── State ───
  var state = {
    snapshot: null,
    stats: null,
    teamIndex: 0,
    bottomTab: 'activity',
    theme: localStorage.getItem('cctrack-theme') || 'dark',
    charts: { token: null, cost: null, project: null }
  };

  // Apply saved theme
  document.documentElement.setAttribute('data-theme', state.theme);
  updateThemeIcon();

  // ─── SSE Connection ───
  function connectSSE() {
    var es = new EventSource('/api/sse');
    es.onmessage = function (e) {
      try {
        state.snapshot = JSON.parse(e.data);
        renderAll();
      } catch (err) { console.error('SSE parse error:', err); }
    };
    es.onerror = function () { es.close(); setTimeout(connectSSE, 3000); };
  }
  connectSSE();

  // ─── Stats Fetch ───
  function fetchStats() {
    fetch('/api/stats')
      .then(function (r) { return r.json(); })
      .then(function (d) { state.stats = d; renderCharts(); renderHeader(); })
      .catch(function () {});
  }
  fetchStats();
  setInterval(fetchStats, 60000);

  // ─── Theme Toggle ───
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

  // ─── Panel Tab Switching ───
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
    if (!state.snapshot || !state.snapshot.teams || state.snapshot.teams.length === 0) return;
    var teams = state.snapshot.teams;
    if (state.teamIndex >= teams.length) state.teamIndex = 0;

    renderHeader();
    renderTeamTabs(teams);
    renderSidebar(teams[state.teamIndex]);
    renderBottomPanel();
  }

  // ─── Header Stats ───
  function renderHeader() {
    var snap = state.snapshot;
    if (!snap || !snap.teams || !snap.teams.length) return;
    var team = snap.teams[state.teamIndex] || snap.teams[0];
    var agents = team.agents || [];

    var activeCount = agents.filter(function (a) { return a.status === 'Active'; }).length;
    setText('stat-active', activeCount);
    setText('stat-sessions', agents.length);

    var totalTokens = 0;
    var totalCost = 0;
    agents.forEach(function (a) {
      if (a.tokens) {
        totalTokens += tokenTotal(a.tokens);
        totalCost += estimateCost(a.tokens);
      }
    });
    setText('stat-tokens', formatTokens(totalTokens));
    setText('stat-cost', '$' + totalCost.toFixed(2));
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
        renderCharts();
      };
    });
  }

  // ─── Sidebar ───
  function renderSidebar(team) {
    var isAll = team.name === 'all';
    setText('sidebar-title', isAll ? 'Sessions' : 'Agents');

    var agents = team.agents || [];
    var el = document.getElementById('agent-list');

    if (agents.length === 0) {
      el.innerHTML = '<div class="empty-state">No sessions</div>';
      return;
    }

    el.innerHTML = agents.map(function (a) {
      var dotCls = (a.status || 'unknown').toLowerCase();
      var cost = a.tokens ? estimateCost(a.tokens) : 0;
      var costStr = cost > 0 ? '$' + cost.toFixed(2) : '';
      var model = a.model ? shortModel(a.model) : '';
      var subCount = a.sub_agent_count && a.sub_agent_count > 0 ? ' (' + a.sub_agent_count + ')' : '';

      return '<div class="agent-item">' +
        '<div class="agent-dot ' + dotCls + '"></div>' +
        '<div class="agent-info">' +
          '<div class="agent-name">' + esc(a.name) + esc(subCount) + '</div>' +
          '<div class="agent-meta">' +
            (model ? '<span class="model">' + esc(model) + '</span>' : '') +
            '<span>' + formatTokens(tokenTotal(a.tokens)) + '</span>' +
          '</div>' +
        '</div>' +
        (costStr ? '<div class="agent-cost">' + costStr + '</div>' : '') +
        '</div>';
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
      case 'todos': renderTodos(el, team.todos || []); break;
      case 'messages': renderMessages(el, team.messages || []); break;
    }
  }

  function renderActivity(el, events, isAll) {
    if (events.length === 0) {
      el.innerHTML = '<div class="empty-state">No activity yet</div>';
      return;
    }
    var recent = events.slice(-80);
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
        '<span class="feed-summary">' + esc(truncate(summary, 100)) + '</span>' +
        (dur ? '<span class="feed-duration">' + dur + '</span>' : '') +
        '</div>';
    }).join('');

    // Auto-scroll to bottom
    el.scrollTop = el.scrollHeight;
  }

  function renderTodos(el, todos) {
    if (todos.length === 0) {
      el.innerHTML = '<div class="empty-state">No active todos</div>';
      return;
    }
    el.innerHTML = todos.map(function (t) {
      var status = t.status || 'pending';
      var sym = { completed: '\u2713', in_progress: '\u25cf', pending: '\u25cb', blocked: '\u2298' }[status] || '?';
      var label = { completed: 'done', in_progress: 'running', pending: 'pending', blocked: 'blocked' }[status] || status;
      var text = (status === 'in_progress' && t.active_form) ? t.active_form : t.content;
      return '<div class="todo-item">' +
        '<span class="todo-status ' + status + '">' + sym + ' ' + esc(label) + '</span>' +
        '<span class="todo-content">' + esc(text) + '</span>' +
        '</div>';
    }).join('');
  }

  function renderMessages(el, messages) {
    var filtered = messages.filter(function (m) { return m.msg_type !== 'idle_notification'; });
    if (filtered.length === 0) {
      el.innerHTML = '<div class="empty-state">No messages yet</div>';
      return;
    }
    var recent = filtered.slice(-50);
    el.innerHTML = recent.map(function (m) {
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

    renderTokenChart(state.stats.daily || [], textColor, gridColor, isDark);
    renderCostChart(state.stats.daily || [], textColor, gridColor, isDark);
    renderProjectChart(state.stats.by_project || [], isDark);
  }

  function renderTokenChart(daily, textColor, gridColor, isDark) {
    if (state.charts.token) state.charts.token.destroy();
    var ctx = document.getElementById('token-chart');
    if (!ctx) return;

    var labels = daily.map(function (d) { return d.date.slice(5); }); // MM-DD
    state.charts.token = new Chart(ctx, {
      type: 'line',
      data: {
        labels: labels,
        datasets: [
          {
            label: 'Input',
            data: daily.map(function (d) { return d.input_tokens; }),
            borderColor: isDark ? '#60a5fa' : '#2563eb',
            backgroundColor: isDark ? 'rgba(96,165,250,0.1)' : 'rgba(37,99,235,0.1)',
            fill: true, tension: 0.4, pointRadius: 0, borderWidth: 2
          },
          {
            label: 'Output',
            data: daily.map(function (d) { return d.output_tokens; }),
            borderColor: isDark ? '#a78bfa' : '#7c3aed',
            backgroundColor: isDark ? 'rgba(167,139,250,0.1)' : 'rgba(124,58,237,0.1)',
            fill: true, tension: 0.4, pointRadius: 0, borderWidth: 2
          },
          {
            label: 'Cache',
            data: daily.map(function (d) { return d.cache_tokens; }),
            borderColor: isDark ? '#22d3ee' : '#0891b2',
            backgroundColor: isDark ? 'rgba(34,211,238,0.08)' : 'rgba(8,145,178,0.08)',
            fill: true, tension: 0.4, pointRadius: 0, borderWidth: 2
          }
        ]
      },
      options: {
        responsive: true, maintainAspectRatio: false,
        interaction: { mode: 'index', intersect: false },
        plugins: {
          legend: { position: 'top', labels: { boxWidth: 12, padding: 12, font: { size: 11 } } },
          tooltip: {
            callbacks: {
              label: function (ctx) { return ctx.dataset.label + ': ' + formatTokens(ctx.raw); }
            }
          }
        },
        scales: {
          x: { grid: { display: false }, ticks: { font: { size: 10 }, maxRotation: 0 } },
          y: {
            grid: { color: gridColor },
            ticks: {
              font: { size: 10 },
              callback: function (v) { return formatTokens(v); }
            }
          }
        }
      }
    });
  }

  function renderCostChart(daily, textColor, gridColor, isDark) {
    if (state.charts.cost) state.charts.cost.destroy();
    var ctx = document.getElementById('cost-chart');
    if (!ctx) return;

    var labels = daily.map(function (d) { return d.date.slice(5); });
    state.charts.cost = new Chart(ctx, {
      type: 'bar',
      data: {
        labels: labels,
        datasets: [{
          label: 'Cost',
          data: daily.map(function (d) { return Math.round(d.cost_usd * 100) / 100; }),
          backgroundColor: isDark ? 'rgba(99,102,241,0.6)' : 'rgba(99,102,241,0.7)',
          borderColor: isDark ? '#6366f1' : '#4f46e5',
          borderWidth: 1, borderRadius: 3, barPercentage: 0.7
        }]
      },
      options: {
        responsive: true, maintainAspectRatio: false,
        plugins: {
          legend: { display: false },
          tooltip: { callbacks: { label: function (ctx) { return '$' + ctx.raw.toFixed(2); } } }
        },
        scales: {
          x: { grid: { display: false }, ticks: { font: { size: 10 }, maxRotation: 0 } },
          y: {
            grid: { color: gridColor },
            ticks: { font: { size: 10 }, callback: function (v) { return '$' + v; } }
          }
        }
      }
    });
  }

  function renderProjectChart(projects, isDark) {
    if (state.charts.project) state.charts.project.destroy();
    var ctx = document.getElementById('project-chart');
    if (!ctx) return;

    var top = projects.slice(0, 6);
    var colors = isDark
      ? ['#6366f1', '#8b5cf6', '#a78bfa', '#60a5fa', '#22d3ee', '#34d399']
      : ['#4f46e5', '#7c3aed', '#8b5cf6', '#2563eb', '#0891b2', '#059669'];

    state.charts.project = new Chart(ctx, {
      type: 'doughnut',
      data: {
        labels: top.map(function (p) { return p.label; }),
        datasets: [{
          data: top.map(function (p) { return Math.round(p.cost_usd * 100) / 100; }),
          backgroundColor: colors.slice(0, top.length),
          borderWidth: 0, hoverOffset: 4
        }]
      },
      options: {
        responsive: true, maintainAspectRatio: false,
        cutout: '60%',
        plugins: {
          legend: {
            position: 'right',
            labels: { boxWidth: 10, padding: 8, font: { size: 11 } }
          },
          tooltip: { callbacks: { label: function (ctx) { return ctx.label + ': $' + ctx.raw.toFixed(2); } } }
        }
      }
    });
  }

  // ─── Helpers ───
  function tokenTotal(tokens) {
    if (!tokens) return 0;
    return (tokens.input_tokens || 0) + (tokens.output_tokens || 0) +
      (tokens.cache_read_tokens || 0) + (tokens.cache_create_5m_tokens || 0) +
      (tokens.cache_create_1h_tokens || 0);
  }

  function estimateCost(tokens) {
    if (!tokens) return 0;
    if (tokens.cost_usd && tokens.cost_usd > 0) return tokens.cost_usd;
    var i = (tokens.input_tokens || 0) / 1e6 * 5;
    var o = (tokens.output_tokens || 0) / 1e6 * 25;
    var cr = (tokens.cache_read_tokens || 0) / 1e6 * 0.5;
    var cw = (tokens.cache_create_5m_tokens || 0) / 1e6 * 6.25;
    var cw1h = (tokens.cache_create_1h_tokens || 0) / 1e6 * 10;
    return i + o + cr + cw + cw1h;
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
    try { return new Date(ts).toLocaleTimeString('en-US', { hour12: false, hour: '2-digit', minute: '2-digit', second: '2-digit' }); }
    catch (e) { return '--:--'; }
  }

  function truncate(s, max) { return s && s.length > max ? s.slice(0, max) + '...' : (s || ''); }
  function setText(id, v) { var el = document.getElementById(id); if (el) el.textContent = v; }
  function esc(s) {
    if (!s) return '';
    var d = document.createElement('div');
    d.appendChild(document.createTextNode(String(s)));
    return d.innerHTML;
  }
})();
