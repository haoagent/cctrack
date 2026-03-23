// cctrack Web Dashboard — SSE Client
(function() {
  'use strict';

  var selectedTeamIndex = 0;
  var latestSnapshot = null;
  var statsData = null;

  // SSE with auto-reconnection
  function connectSSE() {
    var es = new EventSource('/api/sse');
    es.onmessage = function(event) {
      try {
        latestSnapshot = JSON.parse(event.data);
        renderAll();
      } catch (e) {
        console.error('Failed to parse SSE data:', e);
      }
    };
    es.onerror = function() {
      es.close();
      setTimeout(connectSSE, 3000);
    };
  }
  connectSSE();

  // Fetch stats periodically (every 60s)
  function fetchStats() {
    fetch('/api/stats')
      .then(function(r) { return r.json(); })
      .then(function(data) { statsData = data; renderAll(); })
      .catch(function() {});
  }
  fetchStats();
  setInterval(fetchStats, 60000);

  function renderAll() {
    if (!latestSnapshot || !latestSnapshot.teams || latestSnapshot.teams.length === 0) {
      return;
    }
    renderTabBar(latestSnapshot.teams);
    var team = latestSnapshot.teams[selectedTeamIndex] || latestSnapshot.teams[0];
    renderTeam(team);
  }

  function renderTabBar(teams) {
    var bar = document.getElementById('tab-bar');
    bar.innerHTML = teams.map(function(t, i) {
      var isSelected = i === selectedTeamIndex;
      var hasActive = t.agents && t.agents.some(function(a) { return a.status === 'Active'; });
      var isAll = i === 0;
      var cls = 'tab' + (isSelected ? ' tab-selected' : '') + (isAll ? ' tab-all' : '');
      var dot = hasActive ? '\u25cf' : '\u25cb';
      var dotCls = hasActive ? 'dot-active' : 'dot-idle';
      return '<button class="' + cls + '" data-index="' + i + '">' +
        '<span class="' + dotCls + '">' + dot + '</span> ' +
        esc(t.name.toUpperCase()) +
        '</button>';
    }).join('');

    bar.querySelectorAll('.tab').forEach(function(btn) {
      btn.onclick = function() {
        selectedTeamIndex = parseInt(btn.dataset.index, 10);
        renderAll();
      };
    });
  }

  function renderTeam(team) {
    var isAll = team.name === 'all';

    // Filter agents first, then use for everything
    var agents = team.agents || [];
    if (isAll) {
      agents = agents.filter(function(a) { return a.status !== 'Shutdown'; });
    }

    // Dynamic labels
    var label = isAll ? 'Sessions' : 'Agents';
    document.getElementById('agent-label').textContent = label;
    document.getElementById('agents-panel-title').textContent = label;
    document.getElementById('agents-empty-title').textContent = isAll ? 'No sessions' : 'No agents';

    // Header stats
    document.getElementById('agent-count').textContent = agents.length;
    var todoDone = team.todos ? team.todos.filter(function(t) { return t.status === 'completed'; }).length : 0;
    var todoTotal = team.todos ? team.todos.length : 0;
    document.getElementById('todo-progress').textContent = todoDone + '/' + todoTotal;
    document.getElementById('event-count').textContent = team.tool_events ? team.tool_events.length : 0;
    var costUsd = team.metrics ? team.metrics.estimated_cost_usd : 0;
    document.getElementById('total-cost').textContent = '$' + costUsd.toFixed(2);

    renderAgents(agents);

    // ALL tab: show Stats; Team tab: show Todos
    var statsPanel = document.getElementById('stats-panel');
    var todosPanel = document.getElementById('todos-panel');
    if (isAll) {
      statsPanel.style.display = '';
      todosPanel.style.display = 'none';
      renderStats(statsData);
    } else {
      statsPanel.style.display = 'none';
      todosPanel.style.display = '';
      renderTodos(team.todos || []);
    }

    renderActivity(team.tool_events || [], isAll);

    // ALL tab: hide messages panel
    var messagesPanel = document.getElementById('messages-panel');
    if (isAll) {
      messagesPanel.style.display = 'none';
    } else {
      messagesPanel.style.display = '';
      renderMessages(team.messages || []);
    }
  }

  function renderAgents(agents) {
    var tbody = document.getElementById('agents-body');
    var empty = document.getElementById('agents-empty');

    if (agents.length === 0) {
      tbody.innerHTML = '';
      empty.style.display = 'block';
      return;
    }
    empty.style.display = 'none';

    tbody.innerHTML = agents.map(function(agent) {
      var name = agent.name || '\u2014';
      var status = agent.status || 'Unknown';
      var statusClass = status.toLowerCase();
      var tokens = agent.tokens ? (agent.tokens.input_tokens + agent.tokens.output_tokens + agent.tokens.cache_read_tokens + agent.tokens.cache_create_tokens) : 0;
      var tokensStr = tokens > 0 ? formatTokens(tokens) : '\u2014';
      var cost = agent.tokens ? estimateCost(agent.tokens) : 0;
      var costStr = cost > 0 ? '$' + cost.toFixed(2) : '\u2014';

      return '<tr>' +
        '<td>' + esc(name) + '</td>' +
        '<td><span class="status-dot ' + statusClass + '"></span>' +
        '<span class="status-' + statusClass + '">' + esc(status) + '</span></td>' +
        '<td style="color:var(--text-muted);text-align:right">' + tokensStr + '</td>' +
        '<td style="color:var(--green-bright);text-align:right">' + costStr + '</td>' +
        '</tr>';
    }).join('');
  }

  function renderStats(stats) {
    var body = document.getElementById('stats-body');
    if (!stats) {
      body.innerHTML = '<tr><td colspan="4" style="color:var(--text-muted);text-align:center">Loading...</td></tr>';
      return;
    }

    var periods = [stats.today, stats.this_week, stats.this_month, stats.total];
    var html = periods.map(function(p) {
      var isBold = p.label === 'Total';
      var style = isBold ? 'font-weight:600' : '';
      return '<tr style="' + style + '">' +
        '<td>' + esc(p.label) + '</td>' +
        '<td style="text-align:right">' + p.sessions + '</td>' +
        '<td style="text-align:right;color:var(--text-muted)">' + formatTokens(p.total_tokens) + '</td>' +
        '<td style="text-align:right;color:var(--green-bright)">$' + p.cost_usd.toFixed(2) + '</td>' +
        '</tr>';
    }).join('');

    if (stats.by_project && stats.by_project.length > 0) {
      html += '<tr><td colspan="4" style="font-weight:600;padding-top:12px;border-bottom:none">By Project</td></tr>';
      html += stats.by_project.slice(0, 8).map(function(p) {
        return '<tr>' +
          '<td>' + esc(p.label) + '</td>' +
          '<td style="text-align:right">' + p.sessions + '</td>' +
          '<td style="text-align:right;color:var(--text-muted)">' + formatTokens(p.total_tokens) + '</td>' +
          '<td style="text-align:right;color:var(--green-bright)">$' + p.cost_usd.toFixed(2) + '</td>' +
          '</tr>';
      }).join('');
    }

    body.innerHTML = html;
  }

  function renderTodos(todos) {
    var tbody = document.getElementById('todos-body');
    var empty = document.getElementById('todos-empty');

    if (todos.length === 0) {
      tbody.innerHTML = '';
      empty.style.display = 'block';
      return;
    }
    empty.style.display = 'none';

    tbody.innerHTML = todos.map(function(todo) {
      var status = todo.status || 'pending';
      var symbol = taskSymbol(status);
      var label = taskLabel(status);
      var display = (status === 'in_progress' && todo.active_form) ? todo.active_form : todo.content;

      return '<tr>' +
        '<td class="task-' + status + '">' + symbol + ' ' + esc(label) + '</td>' +
        '<td>' + esc(display) + '</td>' +
        '</tr>';
    }).join('');
  }

  function renderActivity(events, isAll) {
    var feed = document.getElementById('activity-feed');
    var empty = document.getElementById('activity-empty');

    if (events.length === 0) {
      feed.innerHTML = '';
      empty.style.display = 'block';
      return;
    }
    empty.style.display = 'none';

    var recent = events.slice(-50); // chronological, newest at bottom (tail style)
    feed.innerHTML = recent.map(function(ev) {
      var time = formatTime(ev.timestamp);
      var tool = ev.tool_name || '?';
      var toolClass = 'tool-' + tool.toLowerCase();
      var summary = ev.summary || '';
      var duration = ev.duration_ms ? ' ' + ev.duration_ms + 'ms' : '';

      // Show session name from cwd (like TUI does)
      var sessionLabel = '';
      if (isAll && ev.cwd) {
        var parts = ev.cwd.split('/');
        sessionLabel = parts[parts.length - 1] || '';
      }

      return '<div class="feed-item">' +
        '<span class="time">' + esc(time) + '</span>' +
        (sessionLabel ? '<span class="session-label">' + esc(sessionLabel) + '</span>' : '') +
        '<span class="' + toolClass + '" style="min-width:50px">' + esc(tool) + '</span>' +
        '<span class="feed-summary">' + esc(truncate(summary, 80)) + '</span>' +
        '<span class="time">' + esc(duration) + '</span>' +
        '</div>';
    }).join('');
  }

  function renderMessages(messages) {
    var feed = document.getElementById('messages-feed');
    var empty = document.getElementById('messages-empty');

    var filtered = messages.filter(function(m) {
      return m.msg_type !== 'idle_notification';
    });

    if (filtered.length === 0) {
      feed.innerHTML = '';
      empty.style.display = 'block';
      return;
    }
    empty.style.display = 'none';

    var recent = filtered.slice(-50);
    feed.innerHTML = recent.map(function(msg) {
      var time = formatTime(msg.timestamp);
      var from = msg.from || '?';
      var to = msg.to || '?';
      var summary = msg.summary || msg.text || '';

      return '<div class="feed-item">' +
        '<span class="time">' + esc(time) + '</span>' +
        '<span style="color:var(--cyan)">' + esc(from) + '</span>' +
        '<span class="arrow">\u2192</span>' +
        '<span style="color:var(--blue-bright)">' + esc(to) + '</span>' +
        '<span>' + esc(truncate(summary, 100)) + '</span>' +
        '</div>';
    }).join('');
  }

  // Helpers
  function formatTokens(n) {
    if (n >= 1000000) return (n / 1000000).toFixed(1) + 'M';
    if (n >= 1000) return (n / 1000).toFixed(0) + 'K';
    return String(n);
  }

  // Opus 4.6 pricing: $5/$25 input/output, cache_write $6.25, cache_read $0.50 per MTok
  function estimateCost(tokens) {
    var input = tokens.input_tokens / 1000000 * 5;
    var output = tokens.output_tokens / 1000000 * 25;
    var cacheWrite = tokens.cache_create_tokens / 1000000 * 6.25;
    var cacheRead = tokens.cache_read_tokens / 1000000 * 0.50;
    return input + output + cacheWrite + cacheRead;
  }

  function taskSymbol(status) {
    switch (status) {
      case 'completed': return '\u2713';
      case 'in_progress': return '\u25cf';
      case 'pending': return '\u25cb';
      case 'blocked': return '\u2298';
      default: return '?';
    }
  }

  function taskLabel(status) {
    switch (status) {
      case 'in_progress': return 'running';
      case 'completed': return 'done';
      default: return status;
    }
  }

  function formatTime(ts) {
    if (!ts) return '--:--:--';
    try {
      var d = new Date(ts);
      return d.toLocaleTimeString('en-US', { hour12: false });
    } catch (e) {
      return '--:--:--';
    }
  }

  function truncate(s, max) {
    if (!s) return '';
    return s.length > max ? s.substring(0, max) + '...' : s;
  }

  function esc(s) {
    if (!s) return '';
    var div = document.createElement('div');
    div.appendChild(document.createTextNode(String(s)));
    return div.innerHTML;
  }
})();
