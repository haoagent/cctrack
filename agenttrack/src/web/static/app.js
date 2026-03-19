// AgentTrack Web Dashboard — SSE Client
(function() {
  'use strict';

  const eventSource = new EventSource('/api/sse');
  let reconnectTimer = null;

  eventSource.onmessage = function(event) {
    try {
      const snapshot = JSON.parse(event.data);
      if (snapshot.teams && snapshot.teams.length > 0) {
        renderTeam(snapshot.teams[0]);
      } else {
        renderEmpty();
      }
    } catch (e) {
      console.error('Failed to parse SSE data:', e);
    }
  };

  eventSource.onerror = function() {
    console.warn('SSE connection lost, reconnecting...');
  };

  function renderTeam(team) {
    // Header stats
    document.getElementById('team-name').textContent = team.name || '—';
    document.getElementById('agent-count').textContent = team.agents ? team.agents.length : 0;

    const completed = team.tasks ? team.tasks.filter(t => t.status === 'completed').length : 0;
    const total = team.tasks ? team.tasks.length : 0;
    document.getElementById('task-progress').textContent = completed + '/' + total;
    document.getElementById('event-count').textContent = team.tool_events ? team.tool_events.length : 0;

    renderAgents(team.agents || []);
    renderTasks(team.tasks || []);
    renderActivity(team.tool_events || []);
    renderMessages(team.messages || []);
  }

  function renderEmpty() {
    document.getElementById('team-name').textContent = '—';
    document.getElementById('agent-count').textContent = '0';
    document.getElementById('task-progress').textContent = '0/0';
    document.getElementById('event-count').textContent = '0';
    document.getElementById('agents-body').innerHTML = '';
    document.getElementById('agents-empty').style.display = 'block';
  }

  function renderAgents(agents) {
    const tbody = document.getElementById('agents-body');
    const empty = document.getElementById('agents-empty');

    if (agents.length === 0) {
      tbody.innerHTML = '';
      empty.style.display = 'block';
      return;
    }
    empty.style.display = 'none';

    tbody.innerHTML = agents.map(function(agent) {
      const config = agent.config || agent;
      const name = config.name || '—';
      const model = shortenModel(config.model || '');
      const status = agent.status || 'Unknown';
      const statusClass = status.toLowerCase();
      return '<tr>' +
        '<td>' + esc(name) + '</td>' +
        '<td style="color:var(--text-muted)">' + esc(model) + '</td>' +
        '<td><span class="status-dot ' + statusClass + '"></span>' +
        '<span class="status-' + statusClass + '">' + esc(status) + '</span></td>' +
        '</tr>';
    }).join('');
  }

  function renderTasks(tasks) {
    const tbody = document.getElementById('tasks-body');
    const empty = document.getElementById('tasks-empty');

    if (tasks.length === 0) {
      tbody.innerHTML = '';
      empty.style.display = 'block';
      return;
    }
    empty.style.display = 'none';

    tbody.innerHTML = tasks.map(function(task) {
      const id = task.id || '?';
      const status = task.status || 'pending';
      const subject = task.subject || '—';
      const blocked = task.blocked_by && task.blocked_by.length > 0;
      const displayStatus = blocked ? 'blocked' : status;
      const symbol = taskSymbol(displayStatus);
      const suffix = blocked ? ' (by #' + task.blocked_by.join(',#') + ')' : '';

      return '<tr>' +
        '<td>' + esc(id) + '</td>' +
        '<td class="task-' + displayStatus + '">' + symbol + ' ' + esc(displayStatus) + esc(suffix) + '</td>' +
        '<td>' + esc(subject) + '</td>' +
        '</tr>';
    }).join('');
  }

  function renderActivity(events) {
    const feed = document.getElementById('activity-feed');
    const empty = document.getElementById('activity-empty');

    if (events.length === 0) {
      feed.innerHTML = '';
      empty.style.display = 'block';
      return;
    }
    empty.style.display = 'none';

    // Show last 50 events
    const recent = events.slice(-50);
    feed.innerHTML = recent.map(function(ev) {
      const time = formatTime(ev.timestamp);
      const tool = ev.tool_name || '?';
      const toolClass = 'tool-' + tool.toLowerCase();
      const summary = ev.input_summary || '';
      const agent = ev.agent_name || 'unknown';

      return '<div class="feed-item">' +
        '<span class="time">' + esc(time) + '</span>' +
        '<span class="' + toolClass + '" style="min-width:50px">' + esc(tool) + '</span>' +
        '<span style="color:var(--text-muted)">[' + esc(agent) + ']</span> ' +
        '<span>' + esc(summary) + '</span>' +
        '</div>';
    }).join('');

    feed.scrollTop = feed.scrollHeight;
  }

  function renderMessages(messages) {
    const feed = document.getElementById('messages-feed');
    const empty = document.getElementById('messages-empty');

    // Filter out idle notifications
    const filtered = messages.filter(function(m) {
      return m.msg_type !== 'IdleNotification';
    });

    if (filtered.length === 0) {
      feed.innerHTML = '';
      empty.style.display = 'block';
      return;
    }
    empty.style.display = 'none';

    // Show last 50 messages
    const recent = filtered.slice(-50);
    feed.innerHTML = recent.map(function(msg) {
      const time = formatTime(msg.timestamp);
      const from = msg.from || '?';
      const to = msg.to || '?';
      const summary = msg.summary || msg.text || '';

      return '<div class="feed-item">' +
        '<span class="time">' + esc(time) + '</span>' +
        '<span style="color:var(--cyan)">' + esc(from) + '</span>' +
        '<span class="arrow">\u2192</span>' +
        '<span style="color:var(--blue-bright)">' + esc(to) + '</span>' +
        '<span>' + esc(truncate(summary, 100)) + '</span>' +
        '</div>';
    }).join('');

    feed.scrollTop = feed.scrollHeight;
  }

  // Helpers
  function shortenModel(model) {
    if (model.includes('opus')) return 'opus';
    if (model.includes('sonnet')) return 'sonnet';
    if (model.includes('haiku')) return 'haiku';
    return model;
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

  function formatTime(ts) {
    if (!ts) return '--:--:--';
    try {
      const d = new Date(ts);
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
