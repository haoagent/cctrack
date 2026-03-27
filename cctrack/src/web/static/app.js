// cctrack — Minimal Dashboard
(function () {
  'use strict';

  var S = { snap: null, stats: null, ti: 0, tab: 'activity',
    theme: localStorage.getItem('cctrack-theme') || 'light',
    ch: { tok: null, cost: null, proj: null } };

  document.documentElement.setAttribute('data-theme', S.theme);
  themeIcon();

  // SSE
  (function sse() {
    var es = new EventSource('/api/sse');
    es.onmessage = function (e) { try { S.snap = JSON.parse(e.data); render(); } catch(x){} };
    es.onerror = function () { es.close(); setTimeout(sse, 3000); };
  })();

  // Stats
  function fetchStats() {
    fetch('/api/stats').then(function(r){return r.json();}).then(function(d){ S.stats=d; renderCharts(); renderCap(); renderHero(); }).catch(function(){});
  }
  fetchStats(); setInterval(fetchStats, 60000);

  // Theme
  document.getElementById('theme-toggle').onclick = function () {
    S.theme = S.theme === 'dark' ? 'light' : 'dark';
    document.documentElement.setAttribute('data-theme', S.theme);
    localStorage.setItem('cctrack-theme', S.theme);
    themeIcon(); renderCharts();
  };
  function themeIcon() { var e=document.getElementById('theme-icon'); if(e) e.innerHTML = S.theme==='dark' ? '&#9790;' : '&#9728;'; }

  // Panel tabs
  document.querySelectorAll('.ptab').forEach(function(b){
    b.onclick = function(){ document.querySelectorAll('.ptab').forEach(function(x){x.classList.remove('active');}); b.classList.add('active'); S.tab=b.dataset.tab; renderFeed(); };
  });

  // ─── Render ───
  function render() {
    if (!S.snap || !S.snap.teams || !S.snap.teams.length) return;
    if (S.ti >= S.snap.teams.length) S.ti = 0;
    var t = S.snap.teams[S.ti];
    var isAll = t.name === 'all';
    renderHero(); renderTabs(S.snap.teams); renderSessions(t); renderRight(t); renderFeed();
    // Charts + cap only on ALL tab
    document.querySelectorAll('.chart-card').forEach(function(el) { el.style.display = isAll ? '' : 'none'; });
    var capEl = document.getElementById('cap-inline');
    if (capEl) capEl.style.display = isAll ? '' : 'none';
  }

  function renderHero() {
    var t = S.snap && S.snap.teams && S.snap.teams[S.ti];
    if (!t) return;
    var isAll = t.name === 'all';
    var nameEl = document.getElementById('hero-name');

    if (isAll && S.stats) {
      $('hero-cost', '$' + S.stats.today.cost_usd.toFixed(2));
      $('hero-label', 'today');
      if (nameEl) { nameEl.style.display = 'none'; nameEl.textContent = ''; }
    } else {
      var parent = (t.agents || []).find(function(a) { return a.agent_type !== 'subagent'; });
      var cost = parent && parent.tokens ? ecost(parent.tokens) : 0;
      $('hero-cost', '$' + cost.toFixed(2));
      $('hero-label', 'session cost');
      // Show full session name
      var fullName = t.name.replace(/^session:/, '');
      if (nameEl) { nameEl.textContent = fullName; nameEl.style.display = fullName ? '' : 'none'; }
    }
  }

  function tabLabel(name) {
    // "all" → "ALL"
    if (name === 'all') return 'ALL';
    // "session:cctrack: 这个项目..." → extract project name before ":"
    var n = name.replace(/^session:/, '');
    // Take just the project part (before first ": ")
    var colon = n.indexOf(': ');
    if (colon > 0) n = n.substring(0, colon);
    // Truncate
    if (n.length > 16) n = n.substring(0, 14) + '..';
    return n.toUpperCase();
  }

  function renderTabs(teams) {
    var el=document.getElementById('team-tabs');
    el.innerHTML = teams.map(function(t,i){
      var on = (t.agents||[]).some(function(a){return a.status==='Active';});
      var isAll = i === 0;
      var nm = tabLabel(t.name);
      var count = (t.agents||[]).length;
      var badge = isAll ? '' : '<span class="tab-count">'+count+'</span>';
      return '<button class="tab'+(i===S.ti?' active':'')+(isAll?' tab-all':'')+'" data-i="'+i+'"><span class="dot '+(on?'on':'off')+'"></span>'+esc(nm)+badge+'</button>';
    }).join('');
    el.querySelectorAll('.tab').forEach(function(b){ b.onclick=function(){ S.ti=parseInt(b.dataset.i,10); render(); if(S.stats){renderCharts();renderCap();} }; });
  }

  function renderSessions(t) {
    var isAll=t.name==='all';
    var ag=t.agents||[];
    var active=ag.filter(function(a){return(a.status||'').toLowerCase()==='active';}).length;
    var label=(isAll?'Sessions':'Agents')+' ('+active+'/'+ag.length+')';
    $('sessions-title', label);
    var ag=t.agents||[], tb=document.getElementById('sessions-body'), em=document.getElementById('sessions-empty');
    // Dynamic header: Cost first (most important), Tokens secondary
    var thead = tb.parentElement.querySelector('thead tr');
    if(thead) thead.innerHTML = '<th>Name</th><th>Status</th><th>Model</th><th class="r">Cost</th>';
    if(!ag.length){ tb.innerHTML=''; em.style.display='block'; return; }
    em.style.display='none';
    tb.innerHTML = ag.map(function(a){
      var s=(a.status||'unknown').toLowerCase(), tok=a.tokens?ttok(a.tokens):0, cost=a.tokens?ecost(a.tokens):0;
      var m=a.model?smodel(a.model):'', sub=a.sub_agent_count&&a.sub_agent_count>0?'<span class="badge">'+a.sub_agent_count+'</span>':'';
      return '<tr><td class="name" title="'+esc(a.name)+'">'+esc(a.name)+sub+'</td><td><span class="dot-s '+s+'"></span><span class="st '+s+'">'+cap1(a.status)+'</span></td><td><span class="model-label model-'+m+'">'+esc(m||'\u2014')+'</span></td><td class="r cost">'+(cost>0?'$'+cost.toFixed(2):'\u2014')+'</td></tr>';
    }).join('');
  }

  function renderRight(t) {
    var isAll=t.name==='all';
    document.getElementById('stats-table').style.display = isAll?'':'none';
    document.getElementById('todos-table').style.display = isAll?'none':'';
    $('right-title', isAll?'Stats':'Todos');
    if(isAll) renderStats(); else renderTodos(t.todos||[]);
  }

  function renderStats() {
    var b=document.getElementById('stats-body');
    if(!S.stats){ b.innerHTML='<tr><td colspan="4" class="dim" style="text-align:center">Loading...</td></tr>'; return; }
    var s=S.stats;
    // Skip rows that are identical to Total (redundant)
    var ps=[s.today,s.this_week,s.this_month,s.total].filter(function(p,i,arr){
      if(i===arr.length-1) return true; // always show Total
      return p.cost_usd !== arr[arr.length-1].cost_usd || p.sessions !== arr[arr.length-1].sessions;
    });
    var h=ps.map(function(p){ var bl=p.label==='Total'?' bold':'';
      return '<tr><td class="'+bl+'">'+esc(p.label)+'</td><td class="r dim">'+p.sessions+'</td><td class="r tok">'+fmtTok(p.total_tokens)+'</td><td class="r cost">$'+p.cost_usd.toFixed(2)+'</td></tr>';
    }).join('');
    if(s.by_project&&s.by_project.length){
      h+='<tr><td colspan="4" class="bold" style="padding-top:10px;border-bottom:none;font-size:11px;color:var(--text-3)">By Project</td></tr>';
      h+=s.by_project.slice(0,8).map(function(p){ return '<tr><td class="dim">'+esc(p.label)+'</td><td class="r dim">'+p.sessions+'</td><td class="r tok">'+fmtTok(p.total_tokens)+'</td><td class="r cost">$'+p.cost_usd.toFixed(2)+'</td></tr>'; }).join('');
    }
    b.innerHTML=h;
  }

  function renderTodos(todos) {
    var b=document.getElementById('todos-body'), em=document.getElementById('right-empty');
    if(!todos.length){ b.innerHTML=''; em.textContent='No active todos'; em.style.display='block'; return; }
    em.style.display='none';
    b.innerHTML=todos.map(function(t){ var s=t.status||'pending', sym={completed:'\u2713',in_progress:'\u25cf',pending:'\u25cb',blocked:'\u2298'}[s]||'?', lb={completed:'done',in_progress:'running',pending:'pending',blocked:'blocked'}[s]||s, tx=(s==='in_progress'&&t.active_form)?t.active_form:t.content;
      return '<tr><td class="ts-'+s+'">'+sym+' '+esc(lb)+'</td><td>'+esc(tx)+'</td></tr>';
    }).join('');
  }

  // Feed
  function renderFeed() {
    if(!S.snap||!S.snap.teams) return;
    var t=S.snap.teams[S.ti]||S.snap.teams[0], el=document.getElementById('panel-content');
    if(S.tab==='activity') renderActivity(el,t.tool_events||[],t.name==='all');
    else renderMessages(el,t.messages||[]);
  }

  function shortTool(name) {
    if (!name) return '?';
    // mcp__Claude_in_Chrome__computer → Chrome.computer
    // mcp__Claude_Preview__preview_screenshot → Preview.screenshot
    if (name.startsWith('mcp__')) {
      var parts = name.slice(5).split('__');
      if (parts.length >= 2) {
        var svc = parts[0].replace('Claude_in_Chrome','Chrome').replace('Claude_Preview','Preview');
        return svc + '.' + parts.slice(1).join('.');
      }
    }
    return name;
  }

  function renderActivity(el,evts,isAll) {
    var f=evts.filter(function(e){return e.tool_name!=='startup_scan';});
    if(!f.length){ el.innerHTML='<div class="empty">No activity</div>'; return; }
    el.innerHTML=f.slice(-80).map(function(e){
      var rawTool=e.tool_name||'?', tl=shortTool(rawTool), tc='tool-'+rawTool.toLowerCase(), sm=e.summary||'', dr=e.duration_ms?e.duration_ms+'ms':'', tm=fmtTime(e.timestamp);
      // Only show agent column if multiple agents
      var multiAgent = isAll && f.reduce(function(s,x){s[x.agent_name]=1;return s;},{});
      var showAgent = isAll && Object.keys(multiAgent||{}).length > 1;
      var ag=''; if(showAgent&&e.cwd){var p=e.cwd.split('/');ag=p[p.length-1]||'';}
      // Use summary if available, otherwise show shortened tool name
      var displaySm = sm || tl;
      return '<div class="fi"><span class="fi-t">'+esc(tm)+'</span>'+(ag?'<span class="fi-a">'+esc(ag)+'</span>':'')+'<span class="fi-tool '+tc+'">'+esc(tl)+'</span><span class="fi-s">'+esc(trunc(displaySm,120))+'</span>'+(dr?'<span class="fi-d">'+dr+'</span>':'')+'</div>';
    }).join('');
    el.scrollTop=el.scrollHeight;
  }

  function renderMessages(el,msgs) {
    var f=msgs.filter(function(m){return m.msg_type!=='idle_notification';});
    if(!f.length){ el.innerHTML='<div class="empty">No messages</div>'; return; }
    el.innerHTML=f.slice(-50).map(function(m){
      var tm=fmtTime(m.timestamp), sm=m.summary||m.text||'';
      return '<div class="fi"><span class="fi-t">'+esc(tm)+'</span><span class="msg-from">'+esc(m.from)+'</span><span class="msg-arr">\u2192</span><span class="msg-to">'+esc(m.to)+'</span><span class="msg-txt">'+esc(trunc(sm,100))+'</span></div>';
    }).join('');
    el.scrollTop=el.scrollHeight;
  }

  // Cap — OAuth usage from Anthropic API
  var capData = null;
  function fetchCap() {
    fetch('/api/cap').then(function(r){return r.json();}).then(function(d){
      capData = d;
      renderCap();
    }).catch(function(){});
  }
  document.getElementById('cap-connect').onclick = function(e) {
    e.preventDefault();
    fetchCap();
  };
  // Auto-try on load
  fetchCap();

  function renderCap() {
    var el = document.getElementById('cap-inline');
    if (!capData || !capData.ok) {
      if (capData && capData.error) {
        var msg = capData.error;
        if (msg.indexOf('expired') >= 0 || msg.indexOf('Refresh') >= 0 || msg.indexOf('error sending') >= 0 || msg.indexOf('401') >= 0) {
          msg = 'Run claude /login to refresh token';
        } else if (msg.indexOf('No Claude Code') >= 0) {
          msg = 'Run claude /login first';
        }
        el.innerHTML = '<span class="cap-err">' + esc(msg) + '</span> <a href="#" class="cap-retry" id="cap-retry-btn">Retry</a>';
        var rb = document.getElementById('cap-retry-btn');
        if (rb) rb.onclick = function(e) { e.preventDefault(); fetchCap(); };
      }
      return;
    }
    var s = capData.session, w = capData.weekly;
    var plan = capData.plan || '';
    // Format plan name
    var planLabel = plan.indexOf('max_20x') >= 0 ? 'Max 20x' : plan.indexOf('max') >= 0 ? 'Max 5x' : plan.indexOf('pro') >= 0 ? 'Pro' : plan;
    var h = '<span class="cap-plan-label">' + esc(planLabel) + '</span>';
    if (s) {
      var sp = Math.round(s.used_pct);
      var cls = sp >= 90 ? ' full' : sp >= 70 ? ' high' : '';
      var reset = s.resets_at ? fmtReset(s.resets_at) : '';
      h += '<span class="cap-seg"><span class="cap-lbl">5h</span><div class="cap-track"><div class="cap-fill' + cls + '" style="width:' + Math.min(sp, 100) + '%"></div></div><span class="cap-pct mono">' + sp + '%</span>' + (reset ? '<span class="cap-reset">' + reset + '</span>' : '') + '</span>';
    }
    if (w) {
      var wp = Math.round(w.used_pct);
      var wcls = wp >= 90 ? ' full' : wp >= 70 ? ' high' : '';
      h += '<span class="cap-seg"><span class="cap-lbl">7d</span><div class="cap-track"><div class="cap-fill' + wcls + '" style="width:' + Math.min(wp, 100) + '%"></div></div><span class="cap-pct mono">' + wp + '%</span></span>';
    }
    el.innerHTML = h;
    // Auto-refresh every 60s
    if (!S.capInterval) S.capInterval = setInterval(fetchCap, 60000);
  }

  function fmtReset(iso) {
    try {
      var d = new Date(iso), now = new Date();
      var diff = d - now;
      if (diff <= 0) return 'reset';
      var h = Math.floor(diff / 3600000), m = Math.floor((diff % 3600000) / 60000);
      return h + 'h ' + m + 'm';
    } catch(e) { return ''; }
  }

  // Charts
  function renderCharts() {
    if(!S.stats) return;
    var dk=S.theme==='dark', tc=dk?'#52525b':'#a1a1aa', gc=dk?'rgba(255,255,255,0.04)':'rgba(0,0,0,0.04)';
    Chart.defaults.color=tc; Chart.defaults.borderColor=gc;
    var d=S.stats.daily||[];
    // Token — stacked bar with distinct colors
    if(S.ch.tok) S.ch.tok.destroy();
    var ctx=document.getElementById('token-chart');
    // Compute cache hit rate
    var totalCache=0,totalInput=0;
    d.forEach(function(x){totalCache+=(x.cache_tokens||0);totalInput+=(x.input_tokens||0)+(x.cache_tokens||0)+(x.cache_write_tokens||0);});
    var hitRate=totalInput>0?Math.round(totalCache/totalInput*100):0;
    var titleEl=document.getElementById('token-chart-title');
    if(titleEl)titleEl.innerHTML='Token Usage <span class="subtitle">30d</span> <span class="subtitle" style="margin-left:8px">Cache Hit '+hitRate+'%</span>';
    if(ctx&&d.length) S.ch.tok=new Chart(ctx,{type:'bar',data:{labels:d.map(function(x){return x.date.slice(5);}),datasets:[
      {label:'Cache Read',data:d.map(function(x){return x.cache_tokens||0;}),backgroundColor:dk?'rgba(193,95,60,0.06)':'rgba(193,95,60,0.05)',borderColor:dk?'rgba(193,95,60,0.4)':'rgba(193,95,60,0.3)',borderWidth:1,borderDash:[3,3],borderRadius:2,order:3},
      {label:'Input',data:d.map(function(x){return x.input_tokens||0;}),backgroundColor:dk?'rgba(193,95,60,0.25)':'rgba(193,95,60,0.2)',borderRadius:2,order:2},
      {label:'Output',data:d.map(function(x){return x.output_tokens||0;}),backgroundColor:dk?'rgba(193,95,60,0.65)':'rgba(193,95,60,0.55)',borderRadius:2,order:1}
    ]},options:{responsive:true,maintainAspectRatio:false,interaction:{mode:'index',intersect:false},plugins:{legend:{position:'top',labels:{boxWidth:10,padding:8,font:{size:11}}},tooltip:{callbacks:{label:function(c){return (c.dataset.label||'')+' '+fmtTok(c.raw);}}}},scales:{x:{stacked:true,grid:{display:false},ticks:{font:{size:10},maxRotation:0,maxTicksLimit:8}},y:{stacked:true,grid:{color:gc},ticks:{font:{size:10},callback:function(v){return fmtTok(v);}}}}}});
    // Cost
    if(S.ch.cost) S.ch.cost.destroy();
    var ctx2=document.getElementById('cost-chart');
    if(ctx2&&d.length) S.ch.cost=new Chart(ctx2,{type:'bar',data:{labels:d.map(function(x){return x.date.slice(5);}),datasets:[{data:d.map(function(x){return Math.round(x.cost_usd*100)/100;}),backgroundColor:dk?'rgba(193,95,60,0.4)':'rgba(193,95,60,0.35)',borderColor:dk?'#C15F3C':'#C15F3C',borderWidth:1,borderRadius:2,barPercentage:0.7}]},options:co(gc,function(v){return '$'+v;},true)});
  }

  function co(gc,fmt,noLeg){
    return {responsive:true,maintainAspectRatio:false,interaction:{mode:'index',intersect:false},plugins:{legend:noLeg?{display:false}:{position:'top',labels:{boxWidth:10,padding:8,font:{size:11}}},tooltip:{callbacks:{label:function(c){return (c.dataset.label||'')+' '+fmt(c.raw);}}}},scales:{x:{grid:{display:false},ticks:{font:{size:10},maxRotation:0,maxTicksLimit:8}},y:{grid:{color:gc},ticks:{font:{size:10},callback:fmt}}}};
  }

  // Helpers
  function ttok(t){return t?(t.input_tokens||0)+(t.output_tokens||0)+(t.cache_read_tokens||0)+(t.cache_create_5m_tokens||0)+(t.cache_create_1h_tokens||0):0;}
  function ecost(t){if(!t)return 0;if(t.cost_usd&&t.cost_usd>0)return t.cost_usd;return(t.input_tokens||0)/1e6*3+(t.output_tokens||0)/1e6*15+(t.cache_read_tokens||0)/1e6*0.3+((t.cache_create_5m_tokens||0)+(t.cache_create_1h_tokens||0))/1e6*3.75;}
  function fmtTok(n){if(n>=1e9)return(n/1e9).toFixed(1)+'B';if(n>=1e6)return(n/1e6).toFixed(1)+'M';if(n>=1e3)return(n/1e3).toFixed(0)+'K';return String(n||0);}
  function smodel(m){var l=m.toLowerCase();return l.indexOf('opus')>=0?'opus':l.indexOf('sonnet')>=0?'sonnet':l.indexOf('haiku')>=0?'haiku':m.length>10?m.slice(-8):m;}
  function fmtTime(ts){if(!ts)return'--:--';try{return new Date(ts).toLocaleTimeString('en-US',{hour12:false});}catch(e){return'--:--';}}
  function cap1(s){return s?s.charAt(0).toUpperCase()+s.slice(1).toLowerCase():'';}
  function trunc(s,n){return s&&s.length>n?s.slice(0,n)+'...':s||'';}
  function $(id,v){var e=document.getElementById(id);if(e)e.textContent=v;}
  function esc(s){if(!s)return'';var d=document.createElement('div');d.appendChild(document.createTextNode(String(s)));return d.innerHTML;}
})();
