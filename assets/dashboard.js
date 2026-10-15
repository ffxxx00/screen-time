(() => {
  let chart, latest, rowKey = '', rowViews = [], selectedHour = null, openAppName = null, viewMode = 'day', selectedWeekDay = null;
  const usageHelp=document.querySelector('.usage-help');
  const positionUsageHelp=()=>{
    if(!usageHelp.open || !window.document?.documentElement)return;
    const panel=usageHelp.querySelector('.help-content'),anchor=usageHelp.getBoundingClientRect();
    panel.style.left=`${Math.max(8,Math.min(anchor.right-panel.offsetWidth,document.documentElement.clientWidth-panel.offsetWidth-8))}px`;
    panel.style.top=`${Math.max(8,Math.min(anchor.bottom+8,innerHeight-panel.offsetHeight-8))}px`;
  };
  usageHelp.addEventListener('toggle',positionUsageHelp);
  window.addEventListener('resize',positionUsageHelp);
  window.addEventListener('scroll',positionUsageHelp);
  let helpPinned=false, helpCloseTimer;
  const showUsageHelp=()=>{clearTimeout(helpCloseTimer);usageHelp.open=true;positionUsageHelp();};
  const hideUsageHelp=()=>{clearTimeout(helpCloseTimer);helpPinned=false;usageHelp.open=false;};
  usageHelp.addEventListener('pointerenter',event=>{if(event.pointerType!=='touch')showUsageHelp();});
  usageHelp.addEventListener('pointerleave',()=>{
    helpCloseTimer=setTimeout(()=>{if(!helpPinned && !usageHelp.contains(document.activeElement))hideUsageHelp();},160);
  });
  usageHelp.querySelector('summary').addEventListener('focus',showUsageHelp);
  usageHelp.querySelector('summary').addEventListener('click',event=>{
    event.preventDefault();
    if(helpPinned)hideUsageHelp();else {helpPinned=true;showUsageHelp();}
  });
  document.addEventListener('pointerdown',event=>{if(!usageHelp.contains(event.target))hideUsageHelp();});
  document.addEventListener('keydown',event=>{if(event.key==='Escape' && usageHelp.open){usageHelp.querySelector('summary').focus();hideUsageHelp();}});
  document.addEventListener('focusin',event=>{if(!usageHelp.contains(event.target))hideUsageHelp();});
  window.addEventListener('pagehide',()=>clearTimeout(helpCloseTimer));
  const palette = ['#6688b8', '#8da7c9', '#9c91bc', '#79aaa4', '#b6a0b2'];
  const otherColor='#aab7c5';
  const currentLabels = () => viewMode === 'week' ? latest?.week?.labels || [] : latest?.labels || [];
  const isFutureWeekDay = index => latest?.week?.days[index]>latest?.today;
  const usedDailyHour = hour => latest?.hourly.some(values=>values[hour]>0);
  const navigateBars = (indices,current,key) => {
    if(!indices.length) return null;
    const position=indices.indexOf(current);
    if(key==='Home') return indices[0];
    if(key==='End') return indices[indices.length-1];
    if(key==='ArrowRight') return indices[Math.min(indices.length-1,position+1)];
    if(key==='ArrowLeft') return indices[position<0?indices.length-1:Math.max(0,position-1)];
    return null;
  };
  const stableColors = new Map();
  const color = name => { if(!stableColors.has(name)) stableColors.set(name,palette[stableColors.size % palette.length]); return stableColors.get(name); };
  // Identity is retained within this document unless two visible apps collide.
  // The five simultaneous categories always take distinct slots; gray is reserved.
  const prepareColors=labels=>{
    const used=new Set();
    labels.slice(0,5).forEach(name=>{
      let shade=stableColors.get(name);
      if(!palette.includes(shade) || used.has(shade))shade=palette.find(candidate=>!used.has(candidate));
      stableColors.set(name,shade);used.add(shade);
    });
  };
  let guideColor = '#dedee5', selectedBandColor = 'rgba(0,109,224,.08)';
  const quarterGuides = {id:'quarterGuides',beforeDatasetsDraw(owner){
    const {ctx,chartArea,scales:{x}}=owner;
    const selectedBand=viewMode==='week'?selectedWeekDay:selectedHour;
    if(selectedBand !== null){
      const width=Math.abs(x.getPixelForValue(1)-x.getPixelForValue(0));
      ctx.save(); ctx.fillStyle=selectedBandColor;
      ctx.fillRect(x.getPixelForValue(selectedBand)-width/2,chartArea.top,width,chartArea.bottom-chartArea.top);
      ctx.restore();
    }
    if(viewMode==='week') return;
    ctx.save(); ctx.strokeStyle=guideColor; ctx.lineWidth=1; ctx.setLineDash([2,5]);
    for(const hour of [0,6,12,18]){
      const position=Math.round(x.getPixelForValue(hour))+.5;
      ctx.beginPath(); ctx.moveTo(position,chartArea.top); ctx.lineTo(position,chartArea.bottom); ctx.stroke();
    }
    ctx.restore();
  }};
  const roundedBars = {id:'roundedBars',beforeDatasetsDraw(owner){
    const {ctx,chartArea}=owner;
    ctx.save();ctx.beginPath();
    const metas=owner.getSortedVisibleDatasetMetas();
    owner.data.labels.forEach((_,index)=>{
      const bars=metas.filter(meta=>owner.data.datasets[meta.index].data[index]>0)
        .map(meta=>meta.data[index].getProps(['x','y','base','width'],true));
      if(!bars.length) return;
      const top=Math.max(chartArea.top,Math.min(...bars.map(bar=>Math.min(bar.y,bar.base))));
      const bottom=Math.min(chartArea.bottom,Math.max(...bars.map(bar=>Math.max(bar.y,bar.base))));
      const {x,width}=bars[0],height=bottom-top;
      if(height<=0 || width<=0) return;
      // Round the full column, even when its topmost application is a tiny sliver.
      ctx.roundRect(x-width/2,top,width,height,Math.min(4,width/2,height/2));
    });
    ctx.clip();
  },afterDatasetsDraw(owner){owner.ctx.restore();}};
  const appNames = {msedge:'Microsoft Edge',chrome:'Google Chrome',firefox:'Mozilla Firefox',explorer:'Explorador de archivos',code:'Visual Studio Code',notepad:'Bloc de notas',winword:'Microsoft Word',excel:'Microsoft Excel',powerpnt:'Microsoft PowerPoint',outlook:'Microsoft Outlook',teams:'Microsoft Teams',discord:'Discord',spotify:'Spotify'};
  const appName = name => {
    const metadata=latest?.applications?.[name];
    const executable=metadata?.name || (name.startsWith('path:')?name.split(/[\\/]/).pop():name);
    const process = executable.replace(/\.exe$/i, '');
    const display=appNames[process.toLowerCase()] || (/^[a-z][a-z\d _-]*$/.test(process) ? process[0].toUpperCase()+process.slice(1) : process);
    return metadata?.suffix?`${display} · ${metadata.suffix}`:display;
  };
  const duration = minutes => {
    const total = Math.max(0, Math.floor(Number(minutes) + 1e-8));
    const hours = Math.floor(total / 60), mins = total % 60;
    if (Number(minutes)>0 && Number(minutes)<1) return '<1 min';
    return hours ? `${hours} h ${String(mins).padStart(2,'0')} min` : `${mins} min`;
  };
  const preciseDuration = minutes => duration(minutes);
  const dailyAxis = () => ({
    min:0,max:60,
    ticks:{count:3,stepSize:30,callback:value=>`${Math.round(value)} min`}
  });
  const weeklyAxis = totals => {
    // Each bar covers a day. Use whole-hour intervals and a minimum of two hours.
    const halfHours = Math.max(120,...totals)/120;
    const stepHours = [1,2,3,4,6,8,10,12].find(hours=>hours>=halfHours) ?? Math.ceil(halfHours/6)*6;
    return {min:0,max:stepHours*120,ticks:{count:3,stepSize:stepHours*60,callback:value=>`${Math.round(value/60)} h`}};
  };
  const hourRange = hour => `${String(hour).padStart(2,'0')}:00–${String(hour+1).padStart(2,'0')}:00`;
  const dayName = date => new Intl.DateTimeFormat('es-AR',{weekday:'long',day:'numeric',month:'long'}).format(new Date(`${date}T12:00:00`));
  const shortDay = date => new Intl.DateTimeFormat('es-AR',{weekday:'short'}).format(new Date(`${date}T12:00:00`)).replace('.','');
  const weekRange = week => {
    const start=new Date(`${week.start}T12:00:00`), end=new Date(`${week.end}T12:00:00`);
    const first=new Intl.DateTimeFormat('es-AR',{day:'numeric',month:'short'}).format(start);
    const last=new Intl.DateTimeFormat('es-AR',{day:'numeric',month:'short',year:'numeric'}).format(end);
    return `${first} – ${last}`;
  };
  const el = (tag, cls, text) => { const node = document.createElement(tag); node.className = cls; if(text !== undefined) node.textContent = text; return node; };
  const tooltip = el('div','usage-tooltip');
  tooltip.hidden = true; tooltip.setAttribute('role','tooltip'); tooltip.id = 'usage-tooltip';
  document.body.appendChild(tooltip);
  let activeHour=-1, hideTimer, concealTimer;
  let tooltipAnchor={x:0,y:0};
  const tooltipList=el('div','tooltip-list');
  const tooltipHint=el('span','tooltip-hint');
  const positionTooltip=()=>{
    const margin=8, gap=12, width=tooltip.offsetWidth, height=tooltip.offsetHeight;
    const left=tooltipAnchor.x-width/2;
    const top=tooltipAnchor.y-height-gap>=margin ? tooltipAnchor.y-height-gap : tooltipAnchor.y+gap;
    tooltip.style.left=`${Math.max(margin,Math.min(left,innerWidth-width-margin))}px`;
    tooltip.style.top=`${Math.max(margin,Math.min(top,innerHeight-height-margin))}px`;
  };
  const clearTooltipTimers = () => {
    clearTimeout(hideTimer); clearTimeout(concealTimer);
    hideTimer=concealTimer=undefined;
  };
  const hideTooltip = () => {
    clearTooltipTimers(); activeHour=-1;
    tooltip.classList.remove('is-visible'); tooltip.hidden=true;
  };
  const scheduleTooltipHide = () => {
    // Repeated mouse events must not restart a pending exit.
    if(tooltip.hidden || hideTimer!==undefined || concealTimer!==undefined) return;
    hideTimer=setTimeout(()=>{
      hideTimer=undefined;
      tooltip.classList.remove('is-visible');
      concealTimer=setTimeout(()=>{
        concealTimer=undefined;
        if(!tooltip.classList.contains('is-visible')) { tooltip.hidden=true; activeHour=-1; }
      },140);
    },50);
  };
  const showTooltip = (hour,force=false) => {
    if(!latest) return;
    if(viewMode==='week' && isFutureWeekDay(hour)) { hideTooltip(); return; }
    if(!force && activeHour===hour && tooltip.classList.contains('is-visible')) {clearTooltipTimers();positionTooltip();return;}
    const labels=currentLabels(), series=viewMode==='week'?latest.week.daily:latest.hourly;
    const entries = labels.map((name,i)=>({name, minutes:series[i][hour]}))
      .filter(entry=>entry.minutes>0).sort((a,b)=>b.minutes-a.minutes);
    if (!entries.length && viewMode!=='week') { scheduleTooltipHide(); return; }
    clearTooltipTimers();
    const sameHour=activeHour===hour; activeHour=hour;
    const header=el('div','tooltip-header');
    const heading = el('strong','tooltip-heading',viewMode==='week'?dayName(latest.week.days[hour]):hourRange(hour));
    header.append(heading);
    const rows = entries.slice(0,4).map(entry=>{
      const row=el('div','tooltip-row');
      const name=el('span','tooltip-name',appName(entry.name));
      const swatch=el('span','swatch');swatch.setAttribute('aria-hidden','true');
      swatch.style.setProperty('--app-color',labels.indexOf(entry.name)<5?color(entry.name):otherColor);
      row.append(swatch,name,el('span','tooltip-time',duration(entry.minutes))); return row;
    });
    if(!rows.length) rows.push(el('div','tooltip-row',latest.week.measuredDays?.[hour]?'Sin uso registrado':'Sin medición'));
    const scroll = sameHour ? tooltipList.scrollTop : 0;
    tooltipList.replaceChildren(...rows);
    tooltipHint.textContent=`+${Math.max(0,entries.length-4)} más · seleccioná`;
    tooltipHint.hidden=entries.length<=4;
    tooltip.replaceChildren(header,tooltipList,tooltipHint); tooltip.hidden=false;
    tooltipList.scrollTop=scroll;
    positionTooltip();
    // Commit the transparent starting style after removing display:none.
    // Otherwise WebView can coalesce both states and skip the entrance.
    getComputedStyle(tooltip).opacity;
    tooltip.classList.add('is-visible');
  };
  const externalTooltip = ({chart:owner,tooltip: model}) => {
    if (!latest || !model.opacity || !model.dataPoints?.length) { scheduleTooltipHide(); return; }
    const rect=owner.canvas.getBoundingClientRect();
    const index=model.dataPoints[0].dataIndex;
    const tops=(owner.getSortedVisibleDatasetMetas?.() || []).map(meta=>meta.data[index]).filter(bar=>bar && Number.isFinite(bar.y) && bar.y!==bar.base).map(bar=>bar.y);
    tooltipAnchor={x:rect.left+(model.caretX || 0),y:rect.top+(tops.length?Math.min(...tops):(model.caretY || 0))};
    showTooltip(model.dataPoints[0].dataIndex);
  };
  const inspector=document.getElementById('hour-inspector');
  const inspectorApps=document.getElementById('selected-apps');
  const moreHourApps=document.getElementById('more-hour-apps');
  const chartCanvas=document.getElementById('chart');
  let showAllHourApps=false, inspectorKey='';
  const renderSelectedHour=()=>{
    inspector.hidden=viewMode==='week' || selectedHour===null || !latest?.labels.length;
    if(inspector.hidden){inspectorKey='';if(viewMode==='day' && selectedHour!==null)chartCanvas.setAttribute('aria-label',`Uso por hora. ${hourRange(selectedHour)}: sin actividad. Usá las flechas para explorar las horas.`);return;}
    const hour=selectedHour;
    const entries=latest.labels.map((name,i)=>({name,minutes:latest.hourly[i][hour]}))
      .filter(entry=>entry.minutes>0).sort((a,b)=>b.minutes-a.minutes);
    const key=JSON.stringify([hour,showAllHourApps,entries.map(entry=>[entry.name,entry.minutes,color(entry.name)])]);
    if(key===inspectorKey) return;
    inspectorKey=key;
    const total=entries.reduce((sum,entry)=>sum+entry.minutes,0);
    document.getElementById('selected-hour').textContent=hourRange(hour);
    document.getElementById('selected-total').textContent=total ? preciseDuration(total) : 'Sin actividad';
    chartCanvas.setAttribute('aria-label',`Uso por hora. ${hourRange(hour)}: ${total ? preciseDuration(total) : 'sin actividad'}. Usá las flechas para explorar las horas.`);
    inspectorApps.replaceChildren(...(entries.length ? entries.map((entry,i)=>{
      const row=el('div','hour-inspector-app'); row.hidden=i>=3 && !showAllHourApps;
      const name=el('span','hour-inspector-name',appName(entry.name));
      name.style.setProperty('--app-color',color(entry.name));
      row.append(name,el('span','hour-inspector-time',preciseDuration(entry.minutes)));
      return row;
    }) : [el('p','hour-inspector-empty','No hubo actividad registrada en esta hora.')]));
    const extras=Math.max(0,entries.length-3);
    moreHourApps.hidden=!extras;
    if(extras) moreHourApps.textContent=showAllHourApps ? 'Mostrar menos' : `Ver ${extras} ${extras===1?'aplicación más':'aplicaciones más'}`;
  };
  const selectHour=(hour)=>{
    if(!latest?.labels.length || hour<0 || hour>23 || !usedDailyHour(hour)) return;
    if(selectedHour!==hour) showAllHourApps=false;
    selectedHour=hour;
    renderSelectedHour();
    chart?.update('none');
  };
  moreHourApps.onclick=()=>{showAllHourApps=!showAllHourApps;renderSelectedHour();moreHourApps.focus();};
  document.getElementById('clear-hour').onclick=()=>{
    selectedHour=null;showAllHourApps=false;inspectorKey='';inspector.hidden=true;
    chartCanvas.setAttribute('aria-label','Uso por hora. Seleccioná una barra o usá las flechas para explorar las horas.');
    chart?.update('none');chartCanvas.focus();
  };
  chartCanvas.addEventListener('keydown',event=>{
    if(viewMode==='week') {
      if(!latest?.week) return;
      const last=latest.week.days.findIndex(day=>day>latest.today);
      const max=last<0?6:last-1;
      if(max<0) return;
      if(event.key==='Enter' || event.key===' ') {
        if(selectedWeekDay!==null) {event.preventDefault();openWeekDay(selectedWeekDay);} return;
      }
      let next=selectedWeekDay;
      if(event.key==='ArrowRight') next=next===null?0:Math.min(max,next+1);
      else if(event.key==='ArrowLeft') next=next===null?max:Math.max(0,next-1);
      else if(event.key==='Home') next=0;
      else if(event.key==='End') next=max;
      else return;
      event.preventDefault();selectedWeekDay=next;
      const status=latest.week.measuredDays?.[next] ? (latest.week.dailyTotals[next]>0?preciseDuration(latest.week.dailyTotals[next]):'sin uso registrado') : 'sin medición';
      chartCanvas.setAttribute('aria-label',`${dayName(latest.week.days[next])}: ${status}. Presioná Enter para abrir el día.`);
      chart?.update('none');return;
    }
    if(!latest?.labels.length) return;
    const used=Array.from({length:24},(_,hour)=>hour).filter(usedDailyHour);
    const hour=navigateBars(used,selectedHour,event.key);
    if(hour===null) return;
    event.preventDefault();selectHour(hour);
  });
  window.addEventListener('resize',hideTooltip);
  window.addEventListener('scroll',event=>{if(event.target!==tooltipList)hideTooltip();},true);
  document.addEventListener('keydown',event=>{if(event.key==='Escape')hideTooltip();});
  const send = message => window.ipc.postMessage(JSON.stringify(message));
  const select = date => { if (date) send({command:'select-date',date}); };
  const shift = amount => { if (!latest) return; const date = new Date(`${viewMode==='week'?latest.week.start:latest.date}T12:00:00`); date.setDate(date.getDate() + amount*(viewMode==='week'?7:1)); select(`${date.getFullYear()}-${String(date.getMonth()+1).padStart(2,'0')}-${String(date.getDate()).padStart(2,'0')}`); };
  const openWeekDay=index=>{const date=latest?.week?.days[index];if(!date || date>latest.today || date<latest.firstDate)return;viewMode='day';selectedWeekDay=null;openAppName=null;hideTooltip();window.updateUsage(latest);if(date!==latest.date)select(date);};
  const dateTrigger=document.getElementById('date-trigger');
  const calendar=document.getElementById('calendar');
  const calendarDays=document.getElementById('calendar-days');
  const dateFromParts=(year,month,day)=>`${year}-${String(month+1).padStart(2,'0')}-${String(day).padStart(2,'0')}`;
  let calendarMonth, calendarView='days', yearPageStart=0;
  const calendarMonthLabel=document.getElementById('calendar-month-label');
  const calendarYearLabel=document.getElementById('calendar-year-label');
  const calendarYearRange=document.getElementById('calendar-year-range');
  const calendarWeekdays=document.getElementById('calendar-weekdays');
  const calendarPrev=document.getElementById('calendar-prev');
  const calendarNext=document.getElementById('calendar-next');
  const positionCalendar=()=>{
    if(calendar.hidden) return;
    const margin=8, gap=8, anchor=dateTrigger.getBoundingClientRect();
    if(anchor.bottom<0 || anchor.top>innerHeight) {closeCalendar();return;}
    const width=Math.min(320,innerWidth-2*margin);
    calendar.style.width=`${width}px`;
    calendar.style.left=`${Math.max(margin,Math.min(anchor.left+(anchor.width-width)/2,innerWidth-width-margin))}px`;
    calendar.style.maxHeight=`${innerHeight-2*margin}px`;
    const naturalHeight=calendar.scrollHeight;
    const below=innerHeight-anchor.bottom-gap-margin;
    const above=anchor.top-gap-margin;
    const openBelow=below>=naturalHeight || below>=above;
    const available=Math.max(0,openBelow?below:above);
    calendar.style.maxHeight=`${available}px`;
    const height=Math.min(naturalHeight,available);
    calendar.style.top=`${Math.max(margin,Math.min(openBelow?anchor.bottom+gap:anchor.top-gap-height,innerHeight-height-margin))}px`;
  };
  const closeCalendar=(restoreFocus=false)=>{
    calendar.hidden=true;
    dateTrigger.setAttribute('aria-expanded','false');
    if(restoreFocus) dateTrigger.focus();
  };
  const markSelectedDay=button=>{
    calendarDays.querySelector('.is-selected')?.classList.remove('is-selected');
    calendarDays.querySelector('[aria-current="date"]')?.removeAttribute('aria-current');
    button.classList.add('is-selected');button.setAttribute('aria-current','date');
  };
  const renderCalendar=()=>{
    if(!latest) return;
    const hadFocus=calendar.contains(document.activeElement);
    const focusDate=document.activeElement?.dataset.date;
    const year=calendarMonth.getFullYear(), month=calendarMonth.getMonth();
    const firstMonth=latest.firstDate.slice(0,7), lastMonth=latest.today.slice(0,7);
    const firstYear=Number(firstMonth.slice(0,4)), lastYear=Number(lastMonth.slice(0,4));
    const monthKey=dateFromParts(year,month,1).slice(0,7);
    const monthName=new Intl.DateTimeFormat('es-AR',{month:'long'}).format(calendarMonth);
    calendarMonthLabel.textContent=monthName[0].toUpperCase()+monthName.slice(1);
    calendarYearLabel.textContent=String(year);
    calendarYearRange.textContent=`${yearPageStart}–${yearPageStart+11}`;
    calendarMonthLabel.hidden=calendarView!=='days';
    calendarYearLabel.hidden=calendarView==='years';
    calendarYearRange.hidden=calendarView!=='years';
    calendarWeekdays.hidden=calendarView!=='days';
    calendarPrev.setAttribute('aria-label',calendarView==='days'?'Mes anterior':calendarView==='months'?'Año anterior':'Años anteriores');
    calendarNext.setAttribute('aria-label',calendarView==='days'?'Mes siguiente':calendarView==='months'?'Año siguiente':'Años siguientes');
    calendarPrev.disabled=calendarView==='days'?monthKey<=firstMonth:calendarView==='months'?year<=firstYear:yearPageStart<=firstYear;
    calendarNext.disabled=calendarView==='days'?monthKey>=lastMonth:calendarView==='months'?year>=lastYear:yearPageStart+11>=lastYear;
    const cells=[];
    if(calendarView==='days'){
      calendarDays.className='calendar-days';
      const firstWeekday=(new Date(year,month,1,12).getDay()+6)%7;
      const daysInMonth=new Date(year,month+1,0,12).getDate();
      const cellCount=Math.max(35,Math.ceil((firstWeekday+daysInMonth)/7)*7);
      for(let index=0;index<cellCount;index++){
        const date=new Date(year,month,1-firstWeekday+index,12);
        const value=dateFromParts(date.getFullYear(),date.getMonth(),date.getDate());
        const inMonth=date.getMonth()===month;
        const button=el('button','calendar-day',String(date.getDate()));
        button.type='button'; button.disabled=!inMonth || value<latest.firstDate || value>latest.today;
        button.dataset.date=value;
        button.setAttribute('aria-label',new Intl.DateTimeFormat('es-AR',{weekday:'long',day:'numeric',month:'long',year:'numeric'}).format(date));
        if(!inMonth) button.classList.add('is-outside');
        if(inMonth && (viewMode==='week'?value>=latest.week.start && value<=latest.week.end:value===latest.date)) {
          button.classList.add('is-selected');
          if(value===(viewMode==='week'?latest.week.start:latest.date)) button.setAttribute('aria-current','date');
        }
        if(inMonth && value===latest.today) button.classList.add('is-today');
        button.onclick=()=>{select(value);closeCalendar(true);};
        cells.push(button);
      }
    } else if(calendarView==='months'){
      calendarDays.className='calendar-months';
      for(let index=0;index<12;index++){
        const value=dateFromParts(year,index,1).slice(0,7);
        const name=new Intl.DateTimeFormat('es-AR',{month:'long'}).format(new Date(year,index,1,12));
        const button=el('button','calendar-choice',name[0].toUpperCase()+name.slice(1));
        button.type='button'; button.disabled=value<firstMonth || value>lastMonth;
        if(value===latest.date.slice(0,7)) button.classList.add('is-selected');
        button.onclick=()=>{calendarMonth=new Date(year,index,1,12);calendarView='days';renderCalendar();};
        cells.push(button);
      }
    } else {
      calendarDays.className='calendar-years';
      for(let value=yearPageStart;value<yearPageStart+12;value++){
        const button=el('button','calendar-choice',String(value));
        button.type='button'; button.disabled=value<firstYear || value>lastYear;
        if(value===Number(latest.date.slice(0,4))) button.classList.add('is-selected');
        button.onclick=()=>{calendarMonth=new Date(value,month,1,12);calendarView='months';renderCalendar();};
        cells.push(button);
      }
    }
    calendarDays.replaceChildren(...cells);
    const target=cells.find(cell=>!cell.disabled && cell.dataset.date===focusDate) || cells.find(cell=>!cell.disabled && cell.dataset.date===latest.date) || cells.find(cell=>!cell.disabled && cell.classList.contains('is-selected')) || cells.find(cell=>!cell.disabled);
    cells.forEach(cell=>cell.tabIndex=cell===target?0:-1);
    if(hadFocus) target?.focus();
    positionCalendar();
  };
  dateTrigger.onclick=()=>{
    if(!latest) return;
    if(!calendar.hidden) {closeCalendar();return;}
    const [year,month]=latest.date.split('-').map(Number);
    calendarMonth=new Date(year,month-1,1,12);
    calendarView='days';yearPageStart=Math.floor(year/12)*12;
    renderCalendar();calendar.hidden=false;dateTrigger.setAttribute('aria-expanded','true');positionCalendar();calendarDays.querySelector('[tabindex="0"]')?.focus();
  };
  const moveCalendarPeriod=delta=>{
    if(calendarView==='days') calendarMonth.setMonth(calendarMonth.getMonth()+delta);
    else if(calendarView==='months') calendarMonth.setFullYear(calendarMonth.getFullYear()+delta);
    else yearPageStart+=12*delta;
    renderCalendar();
  };
  // Non-modal popover: Tab can leave it; arrows move its single tab stop.
  calendarDays.addEventListener('focusin',event=>{if(event.target.tagName==='BUTTON') calendarDays.querySelectorAll('button').forEach(button=>button.tabIndex=button===event.target?0:-1);});
  calendarDays.addEventListener('keydown',event=>{
    const cells=[...calendarDays.querySelectorAll('button')];
    const index=cells.indexOf(document.activeElement);
    const columns=calendarView==='days'?7:calendarView==='months'?3:4;
    const offset={ArrowRight:1,ArrowLeft:-1,ArrowDown:columns,ArrowUp:-columns}[event.key];
    let next=event.key==='Home'?cells.findIndex(cell=>!cell.disabled):event.key==='End'?cells.findLastIndex(cell=>!cell.disabled):offset!==undefined?index+offset:null;
    if(next===null) return;
    event.preventDefault();
    if(calendarView==='days' && offset!==undefined && document.activeElement?.dataset.date) {
      const date=new Date(`${document.activeElement.dataset.date}T12:00:00`);date.setDate(date.getDate()+offset);
      const value=dateFromParts(date.getFullYear(),date.getMonth(),date.getDate());
      if(value<latest.firstDate || value>latest.today) return;
      if(date.getMonth()!==calendarMonth.getMonth() || date.getFullYear()!==calendarMonth.getFullYear()){calendarMonth=new Date(date.getFullYear(),date.getMonth(),1,12);renderCalendar();}
      calendarDays.querySelector(`[data-date="${value}"]`)?.focus();return;
    }
    while(next>=0 && next<cells.length && cells[next].disabled) next+=offset<0?-1:1;
    cells[next]?.focus();
  });
  // Chromium can run microtasks between focusout and the next focusin.
  // Wait for the actual focus destination instead of closing during that gap.
  document.addEventListener('focusin',event=>{
    if(!calendar.hidden && !calendar.contains(event.target) && event.target!==dateTrigger) closeCalendar();
  });
  calendarPrev.onclick=()=>moveCalendarPeriod(-1);
  calendarNext.onclick=()=>moveCalendarPeriod(1);
  calendarMonthLabel.onclick=()=>{calendarView='months';renderCalendar();};
  calendarYearLabel.onclick=()=>{yearPageStart=Math.floor(calendarMonth.getFullYear()/12)*12;calendarView='years';renderCalendar();};
  document.addEventListener('pointerdown',event=>{if(!calendar.hidden && !event.target.closest('.date-stepper')) closeCalendar();});
  document.addEventListener('keydown',event=>{if(event.key==='Escape' && !calendar.hidden){closeCalendar(true);event.stopPropagation();}});
  window.addEventListener('resize',positionCalendar);
  window.addEventListener('scroll',event=>{if(event.target!==calendar) positionCalendar();},true);
  document.getElementById('prev').onclick = () => shift(-1);
  document.getElementById('next').onclick = () => shift(1);
  document.getElementById('today').onclick = () => {
    if(!latest || (viewMode==='day'?latest.date===latest.today:latest.week.days.includes(latest.today))) return;
    const [year,month]=latest.today.split('-').map(Number);
    calendarMonth=new Date(year,month-1,1,12);calendarView='days';renderCalendar();
    const todayDay=calendarDays.querySelector('.is-today');
    if(todayDay && viewMode==='day') markSelectedDay(todayDay);
    select(latest.today);closeCalendar(true);
  };
  document.getElementById('mode-day').onclick=()=>{if(viewMode==='day')return;viewMode='day';selectedWeekDay=null;openAppName=null;hideTooltip();window.updateUsage(latest);};
  document.getElementById('mode-week').onclick=()=>{if(viewMode==='week')return;viewMode='week';selectedHour=null;selectedWeekDay=null;openAppName=null;hideTooltip();window.updateUsage(latest);};
  document.getElementById('pause-tracking').onclick=()=>window.ipc.postMessage('toggle-pause');
  document.getElementById('settings').onclick = () => window.ipc.postMessage('open-settings');
  window.updateUsage = data => {
    const previousDate=latest?.date;
    const previousLabels=rowViews.map(view=>view.name);
    if (latest && latest.date !== data.date) {
      hideTooltip();selectedHour=null;selectedWeekDay=null;showAllHourApps=false;inspectorKey='';openAppName=null;
      rowViews.forEach(view=>view.setOpen(false));
      chartCanvas.setAttribute('aria-label','Uso por hora. Seleccioná una barra o usá las flechas para explorar las horas.');
    }
    latest = data;
    if(selectedHour!==null && !usedDailyHour(selectedHour)) {selectedHour=null;showAllHourApps=false;inspectorKey='';}
    const week=viewMode==='week' && Boolean(data.week);
    const labels=week?data.week.labels:data.labels;
    const minutes=week?data.week.minutes:data.minutes;
    const series=week?data.week.daily:data.hourly;
    prepareColors(labels);
    document.querySelector('.dashboard').classList.toggle('is-week',week);
    document.getElementById('mode-day').setAttribute('aria-pressed',String(!week));
    document.getElementById('mode-week').setAttribute('aria-pressed',String(week));
    document.documentElement.dataset.theme = data.darkMode ? 'dark' : 'light';
    const style = getComputedStyle(document.documentElement);
    const secondary = style.getPropertyValue('--secondary').trim();
    guideColor = style.getPropertyValue('--grid').trim();
    selectedBandColor = style.getPropertyValue('--selection-band').trim();
    const displayDate=week?weekRange(data.week):new Intl.DateTimeFormat('es-AR',{day:'numeric',month:'short',year:'numeric'}).format(new Date(`${data.date}T12:00:00`));
    document.getElementById('date-display').textContent=displayDate;
    dateTrigger.disabled=false;
    dateTrigger.setAttribute('aria-label',`Elegir ${week?'semana':'fecha'}. ${displayDate}`);
    if(!calendar.hidden && previousDate!==data.date) {
      if(calendarView==='days') {
        const [year,month]=data.date.split('-').map(Number);
        calendarMonth=new Date(year,month-1,1,12);
      }
      renderCalendar();
    }
    const firstWeekStart=(()=>{const first=new Date(`${data.firstDate}T12:00:00`);first.setDate(first.getDate()-(first.getDay()+6)%7);return dateFromParts(first.getFullYear(),first.getMonth(),first.getDate());})();
    document.getElementById('prev').disabled = week?data.week.start<=firstWeekStart:data.date<=data.firstDate;
    document.getElementById('next').disabled = week?data.week.days.includes(data.today):data.date>=data.today;
    document.getElementById('prev').setAttribute('aria-label',week?'Semana anterior':'Día anterior');
    document.getElementById('next').setAttribute('aria-label',week?'Semana siguiente':'Día siguiente');
    document.getElementById('calendar-today').hidden = week?data.week.days.includes(data.today):data.date===data.today;
    document.getElementById('today').textContent=week?'Ir a esta semana':'Ir a hoy';
    if(!calendar.hidden) positionCalendar();
    document.getElementById('date-label').textContent = week?`Promedio diario registrado · ${data.week.averageDays} ${data.week.averageDays===1?'día':'días'}`:data.date === data.today ? 'Hoy' : dayName(data.date);
    document.getElementById('total').textContent = week&&data.week.averageDays===0 || !week&&!data.measured ? '—' : duration(week?data.week.averageMinutes:data.minutes.reduce((sum,value)=>sum+value,0));
    const state=data.trackingState || 'active';
    document.getElementById('tracking-state').textContent=({manual:'Pausado manualmente',system:'Pausado por bloqueo o suspensión',idle:'Pausado por inactividad',waiting:'Esperando una aplicación',active:'Registrando actividad'})[state];
    document.getElementById('pause-tracking').textContent=state==='manual'?'Reanudar':'Pausar';
    const comparison=document.getElementById('week-comparison');
    comparison.hidden=!week;
    const previous=data.week?.previousAverageMinutes;
    if(week){
      if(previous==null || data.week.averageDays===0) comparison.textContent='Sin comparación anterior';
      else if(previous===0) comparison.textContent=data.week.averageMinutes===0?'Sin cambios respecto de la semana pasada':'La semana pasada no hubo uso registrado';
      else {
        const change=(data.week.averageMinutes/previous-1)*100;
        const percent=Math.round(Math.abs(change));
        comparison.textContent=percent===0?'Sin cambios respecto de la semana pasada':`${change>0?'↑':'↓'} ${percent}% ${change>0?'más':'menos'} que la semana pasada`;
      }
      comparison.title='Comparación del promedio diario registrado con el de la semana anterior. Los días sin medición aportan cero.';
    }
    const coverageNote=document.getElementById('coverage-note');
    const hasUnmeasuredDay=week && data.week.days.some((day,index)=>day>=data.firstDate && day<=data.today && !data.week.measuredDays?.[index]);
    const coveredDays=week?data.week.days.filter((day,index)=>day>=data.firstDate && day<=data.today && data.week.measuredDays?.[index]).length:0;
    coverageNote.textContent=week?(data.week.averageDays===0?'Sin seguimiento esta semana.':hasUnmeasuredDay?`Cobertura parcial: ${coveredDays} de ${data.week.averageDays} días`:'Puede haber períodos sin seguimiento.'):(data.measured?'Puede haber períodos sin seguimiento.':'Sin medición en este día.');
    coverageNote.classList.toggle('incomplete',Boolean(hasUnmeasuredDay));
    const method=document.getElementById('coverage-method');
    const description=week?(data.week.averageDays===0?'No hay días de seguimiento en esta semana. Los días anteriores al inicio y los futuros no se incluyen; el promedio no está disponible.':`El total registrado se divide por ${data.week.averageDays} días de calendario de esta semana desde el inicio del seguimiento. Los días sin medición aportan cero al cálculo; no certifican cero uso. Los días anteriores al inicio y los futuros no se incluyen.`):(data.measured?'La aplicación registró actividad en algún momento del día; puede haber períodos sin seguimiento. Una marca diaria no certifica cobertura continua.':'No hay constancia de seguimiento en este día. Esto no certifica cero uso.');
    if(method.textContent!==description)method.textContent=description;
    document.getElementById('chart-heading').textContent=week?`Uso por día${hasUnmeasuredDay?' · — sin medición':''}`:'Tiempo de uso por hora';
    document.getElementById('empty').textContent=week ? data.week.averageDays===0?'Sin seguimiento en esta semana.':data.week.measuredDays?.some(Boolean)?'Sin uso registrado esta semana.':'Sin medición esta semana; el promedio cuenta estos días como cero.' : data.measured?'Sin uso registrado en este día.':'Sin medición en este día.';
    const updatedMinute=data.updated.slice(0,5);
    const updatedLabel=document.getElementById('updated');
    if(updatedLabel.textContent!==updatedMinute){
      updatedLabel.textContent=updatedMinute;
      updatedLabel.dateTime=updatedMinute;
    }
    document.getElementById('app-count').textContent = week?`Total semanal: ${data.week.averageDays===0?'—':duration(data.week.totalMinutes)}`:data.measured?`${labels.length} ${labels.length === 1 ? 'aplicación' : 'aplicaciones'}`:'';
    document.getElementById('empty').hidden = labels.length > 0;
    const key = JSON.stringify([viewMode,labels]);
    let restoreAppHour=null, restoreAppFocus=null;
    const focusedView=rowViews.find(view=>view.row.contains(document.activeElement));
    const focusedName=focusedView?.name;
    const focusedIndex=focusedView?rowViews.indexOf(focusedView):0;
    const focusedHour=focusedView?.detail.contains(document.activeElement)?focusedView.getSelectedHour():null;
    if(rowKey!==key && rowViews.length===labels.length && labels.every(name=>rowViews.some(view=>view.name===name)) && rowKey.startsWith(`["${viewMode}"`)) {
      rowViews=labels.map(name=>rowViews.find(view=>view.name===name));
      document.getElementById('rows').append(...rowViews.map(view=>view.row));
      rowKey=key;
    }
    if(rowKey !== key) {
      const previousIndex=previousLabels?.indexOf(openAppName) ?? -1;
      const previousView=previousIndex>=0 ? rowViews[previousIndex] : null;
      if(previousView){
        restoreAppHour=previousView.getSelectedHour();
        if(previousView.detail.contains(document.activeElement)) restoreAppFocus='hour';
        else if(document.activeElement===previousView.button) restoreAppFocus='button';
      }
      rowKey = key;
      rowViews = labels.map((name,index) => {
        const row = el('li','app-row'), button = el('button','app-button'), icon = el('span','app-icon',appName(name).slice(0,1).toUpperCase());
        icon.style.setProperty('--app-color',color(name)); icon.setAttribute('aria-hidden','true');
        const info=el('div','app-info'), title=el('div','app-name',appName(name));
        const meter=el('div','app-meter'), track=el('div','bar-track'), fill=el('div','bar-fill'), time=el('span','app-time');
        track.setAttribute('aria-hidden','true'); track.appendChild(fill); meter.append(track,time); info.append(title,meter);
        button.type='button'; button.setAttribute('aria-expanded','false'); button.setAttribute('aria-label',`${appName(name)}: ver actividad por ${week?'día':'hora'}`);
        const disclosure=el('span','app-disclosure','⌄');disclosure.setAttribute('aria-hidden','true');button.append(icon,info,disclosure);
        const detail=el('div','app-detail'); detail.hidden=true; detail.id=`app-detail-${index}`; detail.style.setProperty('--app-color',color(name)); button.setAttribute('aria-controls',detail.id);
        const detailTitle=el('p','app-detail-title',week?'Actividad durante la semana':'Actividad durante el día');
        const peak=el('p','app-hour-peak');
        const peakSummary=el('span','app-peak-summary');
        const readout=el('span','app-hour-readout'); readout.hidden=true;
        peak.append(peakSummary,readout);
        const hours=el('div','app-hours'); hours.setAttribute('role','group'); hours.setAttribute('aria-label',`Uso por ${week?'día':'hora'} de ${appName(name)}`);
        const axis=el('div','app-hours-axis');
        if(week) data.week.days.forEach(day=>axis.append(el('span','',shortDay(day))));
        else [0,6,12,18].forEach(hour=>axis.append(el('span','',String(hour).padStart(2,'0'))));
        detail.append(detailTitle,peak,hours,axis);
        let hourly=[],selectedAppHour=null;
        const selectAppHour=hour=>{
          if(week ? isFutureWeekDay(hour) : !(hourly[hour]>0)) return;
          selectedAppHour=hour;
          bars.forEach((bar,i)=>{bar.classList.toggle('is-selected',i===hour);bar.tabIndex=i===hour?0:-1;});
          peakSummary.hidden=true;
          readout.hidden=false;
          const status=week && !latest.week.measuredDays?.[hour]?'Sin medición':hourly[hour]>0?preciseDuration(hourly[hour]):'Sin uso';
          readout.textContent=`${week?dayName(latest.week.days[hour]):hourRange(hour)} · ${status}`;
        };
        const bars=Array.from({length:week?7:24},(_,hour)=>{
          const bar=el('button','app-hour-bar');bar.type='button';bar.tabIndex=hour===0?0:-1;
          const fill=el('span','app-hour-fill');fill.setAttribute('aria-hidden','true');bar.append(fill);
          bar.addEventListener('mouseenter',()=>selectAppHour(hour));
          bar.addEventListener('focus',()=>selectAppHour(hour));
          bar.onclick=()=>selectAppHour(hour);
          bar.addEventListener('keydown',event=>{
            const available=bars.map((bar,index)=>index).filter(index=>!bars[index].disabled);
            const next=navigateBars(available,hour,event.key);
            if(next===null) return;
            event.preventDefault();bars[next].focus();
          });
          hours.append(bar);return bar;
        });
        const setOpen=open=>{
          detail.hidden=!open;button.setAttribute('aria-expanded',String(open));
          button.setAttribute('aria-label',`${appName(name)}, ${time.textContent}: ${open?'ocultar':'ver'} actividad por ${week?'día':'hora'}`);
        };
        button.onclick=()=>{
          const open=openAppName!==name;
          rowViews.forEach(view=>view.setOpen(false));
          openAppName=open?name:null;
          setOpen(open);
        };
        row.append(button,detail);
        return {name,row,button,fill,time,icon,detail,axis,setOpen,getSelectedHour:()=>selectedAppHour,selectHour:selectAppHour,focusHour:hour=>bars[hour].focus(),setHours(values){
          hourly=values;
          if(selectedAppHour!==null && (week?isFutureWeekDay(selectedAppHour):!(hourly[selectedAppHour]>0))) {
            selectedAppHour=null;readout.hidden=true;peakSummary.hidden=false;
            bars.forEach(bar=>bar.classList.remove('is-selected'));
          }
          const maximum=Math.max(...hourly,0);
          const peakHour=hourly.indexOf(maximum);
          peakSummary.textContent=maximum>0?`Mayor uso: ${week?dayName(latest.week.days[peakHour]):hourRange(peakHour)} · ${preciseDuration(maximum)}`:'Sin actividad registrada';
          bars.forEach((bar,hour)=>{
            const inactive=week?isFutureWeekDay(hour):!(hourly[hour]>0);
            bar.disabled=inactive;
            if(inactive){bar.tabIndex=-1;bar.setAttribute('aria-hidden','true');bar.removeAttribute('aria-label');bar.classList.remove('is-selected');bar.firstChild.style.height='0%';return;}
            bar.removeAttribute('aria-hidden');
            const minutes=hourly[hour], height=maximum>0 && minutes>0 ? Math.max(3,minutes/maximum*100) : 0;
            bar.firstChild.style.height=`${height}%`;
            bar.classList.toggle('is-empty',!minutes);
            if(selectedAppHour===null) bar.tabIndex=hour===(maximum>0?peakHour:0)?0:-1;
            const status=week && !latest.week.measuredDays?.[hour]?'sin medición':minutes>0?preciseDuration(minutes):'sin uso';
            bar.setAttribute('aria-label',`${week?dayName(latest.week.days[hour]):hourRange(hour)}: ${status}`);
          });
          if(selectedAppHour!==null) selectAppHour(selectedAppHour);
        }};
      });
      document.getElementById('rows').replaceChildren(...rowViews.map(view=>view.row));
      if(openAppName){
        const openIndex=labels.indexOf(openAppName);
        if(openIndex>=0) rowViews[openIndex].setOpen(true);
        else openAppName=null;
      }
    }
    rowViews.forEach((view,i)=>{
      const name=labels[i], url=data.icons?.[name];
      if (view.icon.dataset.src === (url || '')) return;
      view.icon.dataset.src=url || '';
      view.icon.classList.toggle('has-image',Boolean(url));
      if(url) {
        const img=el('img',''); img.alt=''; img.width=32; img.height=32;

        img.onerror=()=>{view.icon.classList.remove('has-image');view.icon.textContent=appName(name).slice(0,1).toUpperCase();};
        img.src=url;
        view.icon.replaceChildren(img);
      } else view.icon.textContent=appName(name).slice(0,1).toUpperCase();
    });
    const maximum = minutes[0] || 1;
    rowViews.forEach((view,i) => {
      view.row.querySelector('.app-name').textContent=appName(view.name);
      view.row.querySelector('.app-name').title=latest.applications?.[view.name]?.path || appName(view.name);
      view.icon.style.setProperty('--app-color',color(view.name));view.detail.style.setProperty('--app-color',color(view.name));
      view.time.textContent=duration(minutes[i]); view.button.setAttribute('aria-label',`${appName(view.name)}, ${view.time.textContent}: ${view.detail.hidden?'ver':'ocultar'} actividad por ${week?'día':'hora'}`); view.fill.style.width=`${minutes[i]/maximum*100}%`;
      view.fill.style.setProperty('--app-color',color(labels[i]));
      if(week) data.week.days.forEach((day,index)=>{view.axis.children[index].textContent=shortDay(day);});
      view.setHours(series[i]);
    });
    if(openAppName && (restoreAppHour!==null || restoreAppFocus)){
      const restored=rowViews[labels.indexOf(openAppName)];
      if(restored){
        if(restoreAppHour!==null) restored.selectHour(restoreAppHour);
        if(restoreAppFocus==='hour') restored.focusHour(restoreAppHour ?? 0);
        else if(restoreAppFocus==='button') restored.button.focus();
      }
    }
    if(focusedName && !document.activeElement.closest('.app-row')) {
      const restored=rowViews.find(view=>view.name===focusedName) || rowViews[Math.min(focusedIndex,rowViews.length-1)];
      if(restored){if(restored.name===focusedName && focusedHour!==null && !restored.detail.hidden) restored.focusHour(focusedHour);else restored.button.focus();}
      else document.getElementById('date-trigger').focus();
    }
    const legend = labels.slice(0,5).map((name,i)=>{
      const item=el('div','legend-item'), label=el('div','legend-name'), swatch=el('span','swatch');
      swatch.style.setProperty('--app-color',color(name)); swatch.setAttribute('aria-hidden','true');
      label.title = appName(name); label.append(swatch,el('span','',appName(name))); item.append(label); return item;
    });
    if(labels.length>5){const item=el('div','legend-item'),label=el('div','legend-name'),swatch=el('span','swatch');swatch.style.setProperty('--app-color',otherColor);swatch.setAttribute('aria-hidden','true');label.append(swatch,el('span','','Otras aplicaciones'));item.append(label);legend.push(item);}
    document.getElementById('legend').replaceChildren(...legend);
    const chartLength=week?7:24;
    const datasets=labels.slice(0,5).map((name,i)=>({label:appName(name),data:series[i],backgroundColor:color(name),borderWidth:0,barPercentage:week?.72:.82,categoryPercentage:1}));
    if(labels.length>5) datasets.push({label:'Otras aplicaciones',data:Array.from({length:chartLength},(_,hour)=>series.slice(5).reduce((sum,values)=>sum+values[hour],0)),backgroundColor:otherColor,borderWidth:0,barPercentage:week?.72:.82,categoryPercentage:1});
    if(week) datasets.forEach(dataset=>{dataset.data=dataset.data.map((minutes,index)=>isFutureWeekDay(index)?null:minutes);});
    else datasets.forEach(dataset=>{dataset.data=dataset.data.map(minutes=>minutes>0?minutes:null);});
    datasets.forEach(dataset=>{dataset.borderRadius=0;});
    if(!chart) chart=new Chart(document.getElementById('chart'),{
      type:'bar',data:{labels:Array.from({length:24},(_,hour)=>String(hour)),datasets:[]},plugins:[quarterGuides,roundedBars],
      options:{responsive:true,maintainAspectRatio:false,animation:false,onResize:hideTooltip,onClick:(_event,elements)=>{if(elements.length){if(viewMode==='week')openWeekDay(elements[0].index);else selectHour(elements[0].index);}},interaction:{mode:'index',intersect:false},plugins:{legend:{display:false},tooltip:{enabled:false,external:externalTooltip}},scales:{
        x:{stacked:true,position:'top',border:{display:false},grid:{display:false},ticks:{autoSkip:false,maxRotation:0,padding:8,callback:(_,index)=>index%6===0?String(index).padStart(2,'0'):''}},
        y:{stacked:true,beginAtZero:true,max:60,display:true,ticks:{count:3,callback:value=>`${Math.round(value)} min`},grid:{display:false},border:{display:false}}
      }}
    });
    chart.data.labels=week?data.week.days.map(shortDay):Array.from({length:24},(_,hour)=>String(hour));
    chart.data.datasets=datasets;
    const axis=week?weeklyAxis(data.week.dailyTotals):dailyAxis();
    chart.options.scales.y.min=axis.min;
    chart.options.scales.y.max=axis.max;
    chart.options.scales.y.ticks=axis.ticks;
    chart.options.scales.x.ticks.callback=week?(_,index)=>`${shortDay(data.week.days[index])}${data.week.days[index]<=data.today && !data.week.measuredDays?.[index]?' —':''}`:(_,index)=>index%6===0?String(index).padStart(2,'0'):'';
    chart.options.scales.x.ticks.color=secondary;
    chart.options.scales.y.ticks.color=secondary;
    if(week && selectedWeekDay!==null){
      const index=selectedWeekDay;
      const status=data.week.measuredDays?.[index]?(data.week.dailyTotals[index]>0?preciseDuration(data.week.dailyTotals[index]):'sin uso registrado'):'sin medición';
      chartCanvas.setAttribute('aria-label',`${dayName(data.week.days[index])}: ${status}. Presioná Enter para abrir el día.`);
    } else if(week || selectedHour===null) chartCanvas.setAttribute('aria-label',week?'Uso por día. Usá las flechas y Enter para abrir un día.':'Uso por hora. Seleccioná una barra o usá las flechas para explorar las horas.');
    chart.update('none');
    renderSelectedHour();
    window.clearUsageError();
    window.markViewReady();
  };
  const refresh=()=>window.ipc.postMessage('refresh');
  refresh(); const timer=setInterval(refresh,1000);
  window.addEventListener('pagehide',()=>{clearInterval(timer);hideTooltip();if(chart)chart.destroy();tooltip.remove();});
})();
