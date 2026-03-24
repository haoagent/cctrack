// cctrack — Minimal Dashboard
(function () {
  'use strict';

  var S = { snap: null, stats: null, ti: 0, tab: 'activity',
    theme: localStorage.getItem('cctrack-theme') || 'dark',
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
    renderHero(); renderTabs(S.snap.teams); renderSessions(t); renderRight(t); renderFeed();
  }

  function renderHero() {
    if (S.stats) {
      $('hero-cost', '$'+S.stats.today.cost_usd.toFixed(2));
    } else if (S.snap && S.snap.teams && S.snap.teams[S.ti]) {
      var cost=0;
      (S.snap.teams[S.ti].agents||[]).forEach(function(a){ if(a.tokens) cost+=ecost(a.tokens); });
      $('hero-cost', '$'+cost.toFixed(2));
    }
  }

  function renderTabs(teams) {
    var el=document.getElementById('team-tabs');
    el.innerHTML = teams.map(function(t,i){
      var on = (t.agents||[]).some(function(a){return a.status==='Active';});
      var nm = t.name.replace(/^session:/,'').toUpperCase();
      return '<button class="tab'+(i===S.ti?' active':'')+'" data-i="'+i+'"><span class="dot '+(on?'on':'off')+'"></span>'+esc(nm)+'</button>';
    }).join('');
    el.querySelectorAll('.tab').forEach(function(b){ b.onclick=function(){ S.ti=parseInt(b.dataset.i,10); render(); if(S.stats){renderCharts();renderCap();} }; });
  }

  function renderSessions(t) {
    var isAll=t.name==='all';
    $('sessions-title', isAll?'Sessions':'Agents');
    var ag=t.agents||[], tb=document.getElementById('sessions-body'), em=document.getElementById('sessions-empty');
    if(!ag.length){ tb.innerHTML=''; em.style.display='block'; return; }
    em.style.display='none';
    tb.innerHTML = ag.map(function(a){
      var s=(a.status||'unknown').toLowerCase(), tok=a.tokens?ttok(a.tokens):0, cost=a.tokens?ecost(a.tokens):0;
      var m=a.model?smodel(a.model):'', sub=a.sub_agent_count&&a.sub_agent_count>0?'<span class="badge">'+a.sub_agent_count+'</span>':'';
      var displayName = a.name.length > 40 ? a.name.slice(0, 37) + '...' : a.name;
      return '<tr><td title="'+esc(a.name)+'">'+esc(displayName)+sub+'</td><td><span class="dot-s '+s+'"></span><span class="st '+s+'">'+cap1(a.status)+'</span></td><td class="dim" style="font-size:12px">'+esc(m)+'</td><td class="r tok">'+(tok>0?fmtTok(tok):'\u2014')+'</td><td class="r cost">'+(cost>0?'$'+cost.toFixed(2):'\u2014')+'</td></tr>';
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
    var s=S.stats, ps=[s.today,s.this_week,s.this_month,s.total];
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

  function renderActivity(el,evts,isAll) {
    var f=evts.filter(function(e){return e.tool_name!=='startup_scan';});
    if(!f.length){ el.innerHTML='<div class="empty">No activity</div>'; return; }
    el.innerHTML=f.slice(-80).map(function(e){
      var tm=fmtTime(e.timestamp), tl=e.tool_name||'?', tc='tool-'+tl.toLowerCase(), sm=e.summary||'', dr=e.duration_ms?e.duration_ms+'ms':'';
      var ag=''; if(isAll&&e.cwd){var p=e.cwd.split('/');ag=p[p.length-1]||'';}
      return '<div class="fi"><span class="fi-t">'+esc(tm)+'</span>'+(ag?'<span class="fi-a">'+esc(ag)+'</span>':'')+'<span class="fi-tool '+tc+'">'+esc(tl)+'</span><span class="fi-s">'+esc(trunc(sm,120))+'</span>'+(dr?'<span class="fi-d">'+dr+'</span>':'')+'</div>';
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

  // Cap
  function renderCap() {
    if(!S.stats||!S.stats.cap) return;
    var c=S.stats.cap, cur=c.current;
    var planLabel = {pro:'Pro',max5:'Max 5x',max20:'Max 20x'}[c.plan] || c.plan;
    $('cap-plan', planLabel);
    $('cap-total',fmtTok(c.cap_per_window));
    $('cap-waste',fmtTok(c.total_waste));
    if(cur){
      var p=cur.utilization_pct;
      $('cap-used',fmtTok(cur.output_tokens));
      $('cap-pct',p.toFixed(0)+'%');
      var f=document.getElementById('cap-fill');
      if(f){f.style.width=Math.min(p,100)+'%'; f.className='cap-fill'+(p>=90?' full':p>=70?' high':'');}
    } else {
      $('cap-used','0'); $('cap-pct','—');
      var f=document.getElementById('cap-fill'); if(f){f.style.width='0%';f.className='cap-fill';}
    }
  }

  // Charts
  function renderCharts() {
    if(!S.stats) return;
    var dk=S.theme==='dark', tc=dk?'#52525b':'#a1a1aa', gc=dk?'rgba(255,255,255,0.04)':'rgba(0,0,0,0.04)';
    Chart.defaults.color=tc; Chart.defaults.borderColor=gc;
    var d=S.stats.daily||[];
    // Token
    if(S.ch.tok) S.ch.tok.destroy();
    var ctx=document.getElementById('token-chart');
    if(ctx&&d.length) S.ch.tok=new Chart(ctx,{type:'line',data:{labels:d.map(function(x){return x.date.slice(5);}),datasets:[
      {label:'Input',data:d.map(function(x){return x.input_tokens;}),borderColor:dk?'#3b82f6':'#2563eb',backgroundColor:dk?'rgba(59,130,246,0.06)':'rgba(37,99,235,0.06)',fill:true,tension:0.4,pointRadius:2,borderWidth:1.5},
      {label:'Output',data:d.map(function(x){return x.output_tokens;}),borderColor:dk?'#a78bfa':'#7c3aed',backgroundColor:dk?'rgba(167,139,250,0.06)':'rgba(124,58,237,0.06)',fill:true,tension:0.4,pointRadius:2,borderWidth:1.5},
      {label:'Cache',data:d.map(function(x){return x.cache_tokens;}),borderColor:dk?'#06b6d4':'#0891b2',backgroundColor:dk?'rgba(6,182,212,0.04)':'rgba(8,145,178,0.04)',fill:true,tension:0.4,pointRadius:2,borderWidth:1.5}
    ]},options:co(gc,function(v){return fmtTok(v);})});
    // Cost
    if(S.ch.cost) S.ch.cost.destroy();
    var ctx2=document.getElementById('cost-chart');
    if(ctx2&&d.length) S.ch.cost=new Chart(ctx2,{type:'bar',data:{labels:d.map(function(x){return x.date.slice(5);}),datasets:[{data:d.map(function(x){return Math.round(x.cost_usd*100)/100;}),backgroundColor:dk?'rgba(99,102,241,0.5)':'rgba(99,102,241,0.6)',borderColor:dk?'#6366f1':'#4f46e5',borderWidth:1,borderRadius:2,barPercentage:0.7}]},options:co(gc,function(v){return '$'+v;},true)});
    // Project
    if(S.ch.proj) S.ch.proj.destroy();
    var ctx3=document.getElementById('project-chart'), bp=S.stats.by_project||[];
    if(ctx3&&bp.length){var tp=bp.slice(0,6),cl=['#6366f1','#8b5cf6','#a78bfa','#3b82f6','#06b6d4','#22c55e'];
      S.ch.proj=new Chart(ctx3,{type:'bar',data:{labels:tp.map(function(p){return p.label;}),datasets:[{data:tp.map(function(p){return Math.round(p.cost_usd*100)/100;}),backgroundColor:cl.slice(0,tp.length),borderWidth:0,borderRadius:3,barPercentage:0.5}]},options:{indexAxis:'y',responsive:true,maintainAspectRatio:false,plugins:{legend:{display:false},tooltip:{callbacks:{label:function(c){return '$'+c.raw.toFixed(2);}}}},scales:{x:{grid:{color:gc},ticks:{font:{size:10},callback:function(v){return '$'+v;}}},y:{grid:{display:false},ticks:{font:{size:11}}}}}});
    }
  }

  function co(gc,fmt,noLeg){
    return {responsive:true,maintainAspectRatio:false,interaction:{mode:'index',intersect:false},plugins:{legend:noLeg?{display:false}:{position:'top',labels:{boxWidth:10,padding:8,font:{size:11}}},tooltip:{callbacks:{label:function(c){return (c.dataset.label||'')+' '+fmt(c.raw);}}}},scales:{x:{grid:{display:false},ticks:{font:{size:10},maxRotation:0,maxTicksLimit:8}},y:{grid:{color:gc},ticks:{font:{size:10},callback:fmt}}}};
  }

  // Helpers
  function ttok(t){return t?(t.input_tokens||0)+(t.output_tokens||0)+(t.cache_read_tokens||0)+(t.cache_create_5m_tokens||0)+(t.cache_create_1h_tokens||0):0;}
  function ecost(t){if(!t)return 0;if(t.cost_usd&&t.cost_usd>0)return t.cost_usd;return(t.input_tokens||0)/1e6*5+(t.output_tokens||0)/1e6*25+(t.cache_read_tokens||0)/1e6*0.5+(t.cache_create_5m_tokens||0)/1e6*6.25+(t.cache_create_1h_tokens||0)/1e6*10;}
  function fmtTok(n){if(n>=1e9)return(n/1e9).toFixed(1)+'B';if(n>=1e6)return(n/1e6).toFixed(1)+'M';if(n>=1e3)return(n/1e3).toFixed(0)+'K';return String(n||0);}
  function smodel(m){var l=m.toLowerCase();return l.indexOf('opus')>=0?'opus':l.indexOf('sonnet')>=0?'sonnet':l.indexOf('haiku')>=0?'haiku':m.length>10?m.slice(-8):m;}
  function fmtTime(ts){if(!ts)return'--:--';try{return new Date(ts).toLocaleTimeString('en-US',{hour12:false});}catch(e){return'--:--';}}
  function cap1(s){return s?s.charAt(0).toUpperCase()+s.slice(1).toLowerCase():'';}
  function trunc(s,n){return s&&s.length>n?s.slice(0,n)+'...':s||'';}
  function $(id,v){var e=document.getElementById(id);if(e)e.textContent=v;}
  function esc(s){if(!s)return'';var d=document.createElement('div');d.appendChild(document.createTextNode(String(s)));return d.innerHTML;}
})();
