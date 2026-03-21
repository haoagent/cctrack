// cctrack Web Dashboard — SSE Client
(function() {
  'use strict';

  const eventSource = new EventSource('/api/sse');
  let selectedTeamIndex = 0; // 0 = ALL
  let latestSnapshot = null;

  eventSource.onmessage = function(event) {
    try {
      latestSnapshot = JSON.parse(event.data);
      renderAll();
    } catch (e) {
      console.error('Failed to parse SSE data:', e);
    }
  };

  eventSource.onerror = function() {
    console.warn('SSE connection lost, reconnecting...');
  };

  function renderAll() {
    if (!latestSnapshot || !latestSnapshot.teams || latestSnapshot.teams.length === 0) {
      renderEmpty();
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

    // Bind click events
    bar.querySelectorAll('.tab').forEach(function(btn) {
      btn.onclick = function() {
        selectedTeamIndex = parseInt(btn.dataset.index, 10);
        renderAll();
      };
    });
  }

  function renderTeam(team) {
    // Dynamic labels: ALL tab = "Sessions", team tab = "Agents"
    var isAll = team.name === 'all';
    var label = isAll ? 'Sessions' : 'Agents';
    document.getElementById('agent-label').textContent = label;
    document.getElementById('agents-panel-title').textContent = label;
    document.getElementById('agents-empty-title').textContent = isAll ? 'No sessions' : 'No agents';

    // Header stats
    document.getElementById('agent-count').textContent = team.agents ? team.agents.length : 0;

    var todoDone = team.todos ? team.todos.filter(function(t) { return t.status === 'completed'; }).length : 0;
    var todoTotal = team.todos ? team.todos.length : 0;
    document.getElementById('todo-progress').textContent = todoDone + '/' + todoTotal;
    document.getElementById('event-count').textContent = team.tool_events ? team.tool_events.length : 0;

    var costUsd = team.metrics ? team.metrics.estimated_cost_usd : 0;
    document.getElementById('total-cost').textContent = '$' + costUsd.toFixed(2);

    renderAgents(team.agents || []);
    renderTodos(team.todos || []);
    renderActivity(team.tool_events || []);
    renderMessages(team.messages || []);
  }

  function renderEmpty() {
    document.getElementById('tab-bar').innerHTML = '';
    document.getElementById('agent-count').textContent = '0';
    document.getElementById('task-progress').textContent = '0/0';
    document.getElementById('event-count').textContent = '0';
    document.getElementById('total-cost').textContent = '$0.00';
    document.getElementById('agents-body').innerHTML = '';
    document.getElementById('agents-empty').style.display = 'block';
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

  function renderActivity(events) {
    var feed = document.getElementById('activity-feed');
    var empty = document.getElementById('activity-empty');

    if (events.length === 0) {
      feed.innerHTML = '';
      empty.style.display = 'block';
      return;
    }
    empty.style.display = 'none';

    // Show last 50 events, most recent first
    var recent = events.slice(-50).reverse();
    feed.innerHTML = recent.map(function(ev) {
      var time = formatTime(ev.timestamp);
      var tool = ev.tool_name || '?';
      var toolClass = 'tool-' + tool.toLowerCase();
      var summary = ev.summary || '';
      var duration = ev.duration_ms ? ' ' + ev.duration_ms + 'ms' : '';

      return '<div class="feed-item">' +
        '<span class="time">' + esc(time) + '</span>' +
        '<span class="' + toolClass + '" style="min-width:50px">' + esc(tool) + '</span>' +
        '<span>' + esc(truncate(summary, 80)) + '</span>' +
        '<span class="time">' + esc(duration) + '</span>' +
        '</div>';
    }).join('');
  }

  function renderMessages(messages) {
    var feed = document.getElementById('messages-feed');
    var empty = document.getElementById('messages-empty');

    // Filter out idle notifications
    var filtered = messages.filter(function(m) {
      return m.msg_type !== 'idle_notification';
    });

    if (filtered.length === 0) {
      feed.innerHTML = '';
      empty.style.display = 'block';
      return;
    }
    empty.style.display = 'none';

    // Show last 50 messages
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

  function estimateCost(tokens) {
    var input = (tokens.input_tokens + tokens.cache_create_tokens) / 1000000 * 15;
    var output = tokens.output_tokens / 1000000 * 75;
    var cache = tokens.cache_read_tokens / 1000000 * 1.5;
    return input + output + cache;
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
