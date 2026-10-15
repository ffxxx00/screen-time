const { test } = require('node:test');
const assert = require('node:assert/strict');
const fs = require('node:fs');
const { JSDOM } = require('jsdom');
function page(view = 'dashboard') {
  const dom = new JSDOM(fs.readFileSync(`assets/${view}.html`, 'utf8').replace('__CSS__',fs.readFileSync('assets/dashboard.css','utf8')), { runScripts: 'outside-only' });
  const w = dom.window, messages = [];
  let chart, timer, cleared = false;
  w.ipc = { postMessage: message => messages.push(message) };
  w.setInterval = callback => { timer = callback; return 7; };
  w.clearInterval = id => { assert.equal(id, 7); cleared = true; };
  w.Chart = class {
    constructor(canvas, config) { this.canvas = canvas; this.data = config.data; this.options = config.options; this.options.scales.y.grid ||= {}; chart = this; }
    update(mode) { assert.equal(mode, 'none'); }
    destroy() { this.destroyed = true; }
  };
  w.eval(fs.readFileSync('assets/view-state.js', 'utf8'));
  if (view === 'dashboard') w.eval(fs.readFileSync('assets/dashboard.js', 'utf8'));
  else w.eval([...w.document.querySelectorAll('script')].at(-1).textContent);
  return { w, messages, get chart() { return chart; }, get cleared() { return cleared; }, tick: () => timer(), close: () => dom.window.close() };
}
function usage(overrides = {}) {
  return { labels: ['game.exe'], minutes: [1.5], hourly: [Array.from({length:24}, (_,i) => i === 10 ? 1.5 : 0)],
    week: {start:'2026-09-28',end:'2026-10-04',days:['2026-09-28','2026-09-29','2026-09-30','2026-10-01','2026-10-02','2026-10-03','2026-10-04'],dailyTotals:[1.5,0,0,0,0,0,0],measuredDays:[true,false,false,false,false,false,false],totalMinutes:1.5,averageMinutes:1.5,averageDays:1,labels:['game.exe'],minutes:[1.5],daily:[[1.5,0,0,0,0,0,0]]},
    icons: {}, date: '2026-09-28', today: '2026-09-28', firstDate: '2026-09-27', measured:true, updated: '10:00:00',
    darkMode: true, pauseWhenIdle: false, autostart: true, ...overrides };
}
test('zero use and missing measurement have distinct daily and weekly states', () => {
  const p=page();
  try {
    const doc=p.w.document;
    const empty={labels:[],minutes:[],hourly:[]};
    p.w.updateUsage(usage({...empty,measured:true}));
    assert.equal(doc.getElementById('total').textContent,'0 min');
    assert.match(doc.getElementById('empty').textContent,/Sin uso registrado/);
    p.w.updateUsage(usage({...empty,measured:false}));
    assert.equal(doc.getElementById('total').textContent,'—');
    assert.match(doc.getElementById('empty').textContent,/Sin medición/);
    p.w.updateUsage(usage({...empty,measured:false,today:'2026-09-30',week:{start:'2026-09-28',end:'2026-10-04',days:['2026-09-28','2026-09-29','2026-09-30','2026-10-01','2026-10-02','2026-10-03','2026-10-04'],dailyTotals:[0,0,0,0,0,0,0],measuredDays:[false,true,false,false,false,false,false],totalMinutes:0,averageMinutes:0,averageDays:3,labels:[],minutes:[],daily:[]}}));
    doc.getElementById('mode-week').click();
    assert.equal(doc.getElementById('total').textContent,'0 min');
    assert.match(doc.getElementById('date-label').textContent,/3 días/);
    assert.match(p.chart.options.scales.x.ticks.callback(null,0),/—/);
    assert.doesNotMatch(p.chart.options.scales.x.ticks.callback(null,1),/—/);
    p.w.updateUsage(usage({...empty,today:'2026-09-30',week:{...usage().week,labels:[],minutes:[],daily:[],measuredDays:[false,false,false,false,false,false,false],dailyTotals:[0,0,0,0,0,0,0],totalMinutes:0,averageMinutes:0,averageDays:3}}));
    assert.equal(doc.getElementById('total').textContent,'0 min');
    assert.match(doc.getElementById('empty').textContent,/promedio cuenta estos días como cero/);
    const week=usage().week;
    p.w.updateUsage(usage({today:'2026-09-30',week:{...week,measuredDays:[true,false,true,false,false,false,false]}}));
    doc.querySelector('.app-button').click();
    doc.querySelectorAll('.app-hour-bar')[1].dispatchEvent(new p.w.MouseEvent('mouseenter'));
    assert.match(doc.querySelector('.app-hour-readout').textContent,/Sin medición/);
  } finally { p.close(); }
});
test('chart axis shows whole minutes while retaining fractional usage in both modes', () => {
  const p = page();
  try {
    const value = 60.00476546666667;
    const data = usage({minutes:[value],hourly:[Array.from({length:24},(_,i)=>i===10?value:0)],week:{...usage().week,dailyTotals:[value,0,0,0,0,0,0],daily:[[value,0,0,0,0,0,0]]}});
    p.w.updateUsage(data);
    let axis = p.chart.options.scales.y;
    assert.equal(axis.max,60);
    assert.equal(axis.ticks.callback(0),'0 min');
    assert.equal(axis.ticks.callback(30),'30 min');
    assert.equal(axis.ticks.callback(60),'60 min');
    assert.equal(Math.max(...p.chart.data.datasets[0].data),value);
    p.w.document.getElementById('mode-week').click();
    axis=p.chart.options.scales.y;
    assert.equal(axis.max,120);
    assert.equal(axis.ticks.callback(0),'0 h');
    assert.equal(axis.ticks.callback(60),'1 h');
    assert.equal(axis.ticks.callback(120),'2 h');
    assert.equal(Math.max(...p.chart.data.datasets[0].data),value);
  } finally { p.close(); }
});

test('day keeps its one-hour scale and week uses whole hours through updates and mode switches', () => {
  const p = page();
  try {
    const value = 905.0047654666667;
    p.w.updateUsage(usage({labels:['a.exe','b.exe'],minutes:[500,value-500],hourly:[Array(24).fill(500),Array(24).fill(value-500)],week:{...usage().week,labels:['a.exe','b.exe'],dailyTotals:Array(7).fill(value),daily:[Array(7).fill(500),Array(7).fill(value-500)]}}));
    assert.equal(p.chart.options.scales.y.max,60);
    p.w.document.getElementById('mode-week').click();
    const axis=p.chart.options.scales.y;
    assert.equal(axis.max,960);
    assert.equal(axis.ticks.callback(0),'0 h');
    assert.equal(axis.ticks.callback(axis.max/2),'8 h');
    assert.equal(axis.ticks.callback(axis.max),'16 h');
    for (const [minutes,maxHours] of [[0,2],[20,2],[60,2],[120,2],[359,6],[600,12],[905,16],[1201,24],[1440,24]]) {
      p.w.updateUsage(usage({week:{...usage().week,dailyTotals:[minutes,0,0,0,0,0,0],daily:[[minutes,0,0,0,0,0,0]]}}));
      assert.equal(p.chart.options.scales.y.max,maxHours*60);
      assert.equal(p.chart.options.scales.y.ticks.callback(maxHours*60),`${maxHours} h`);
    }
    p.w.document.getElementById('mode-day').click();
    for (const value of [0,30,60,60.00476546666667,120]) {
      p.w.updateUsage(usage({hourly:[Array(24).fill(value)]}));
      const axis=p.chart.options.scales.y;
      assert.equal(axis.max,60);
      assert.equal(axis.ticks.stepSize,30);
      assert.equal(axis.ticks.callback(60),'60 min');
    }
  } finally { p.close(); }
});

test('future week days have no tooltip, selection or app-chart interaction', () => {
  const p=page();
  try {
    const doc=p.w.document;
    p.w.updateUsage(usage({today:'2026-09-30'}));
    doc.getElementById('mode-week').click();
    const hover=index=>p.chart.options.plugins.tooltip.external({chart:p.chart,tooltip:{opacity:1,dataPoints:[{dataIndex:index}]}});
    const tip=doc.getElementById('usage-tooltip');
    hover(0);assert.equal(tip.hidden,false);
    for(const index of [3,4,5,6]) {
      hover(index);
      assert.equal(tip.hidden,true);
      assert.equal(p.chart.data.datasets[0].data[index],null);
      const count=p.messages.length;
      p.chart.options.onClick(null,[{index}]);
      assert.equal(p.messages.length,count);
      assert.equal(p.chart.data.labels.length,7);
    }
    const chart=doc.getElementById('chart');
    chart.dispatchEvent(new p.w.KeyboardEvent('keydown',{key:'End'}));
    assert.match(chart.getAttribute('aria-label'),/miércoles/);
    chart.dispatchEvent(new p.w.KeyboardEvent('keydown',{key:'ArrowRight'}));
    assert.match(chart.getAttribute('aria-label'),/miércoles/);
    doc.querySelector('.app-button').click();
    const bars=doc.querySelectorAll('.app-hour-bar');
    bars[2].dispatchEvent(new p.w.MouseEvent('mouseenter'));
    const readout=doc.querySelector('.app-hour-readout').textContent;
    for(const index of [3,4,5,6]) {
      assert.equal(bars[index].disabled,true);
      assert.equal(bars[index].tabIndex,-1);
      assert.equal(bars[index].getAttribute('aria-hidden'),'true');
      assert.equal(bars[index].hasAttribute('aria-label'),false);
      bars[index].dispatchEvent(new p.w.MouseEvent('mouseenter'));
      bars[index].click();
      assert.equal(doc.querySelector('.app-hour-readout').textContent,readout);
      assert.equal(bars[index].classList.contains('is-selected'),false);
    }
    bars[0].dispatchEvent(new p.w.KeyboardEvent('keydown',{key:'End'}));
    assert.equal(doc.activeElement,bars[2]);
    assert.doesNotMatch(doc.body.textContent,/día futuro/i);
  } finally { p.close(); }
});

test('week mode shows average, weekly total, seven days and opens a day', () => {
  const p=page();
  try {
    const doc=p.w.document;
    p.w.updateUsage(usage({today:'2026-09-30',week:{start:'2026-09-28',end:'2026-10-04',days:['2026-09-28','2026-09-29','2026-09-30','2026-10-01','2026-10-02','2026-10-03','2026-10-04'],dailyTotals:[60,120,0,0,0,0,0],totalMinutes:180,averageMinutes:60,averageDays:3,labels:['work.exe','game.exe'],minutes:[120,60],daily:[[0,120,0,0,0,0,0],[60,0,0,0,0,0,0]]}}));
    doc.getElementById('mode-week').click();
    assert.equal(doc.querySelector('.summary-top').children.length,2);
    assert.equal(doc.getElementById('total').textContent,'1 h 00 min');
    assert.match(doc.getElementById('date-label').textContent,/3 días/);
    assert.equal(doc.getElementById('weekly-total'),null);
    assert.equal(doc.getElementById('app-count').textContent,'Total semanal: 3 h 00 min');
    assert.equal(doc.getElementById('next').disabled,true);
    doc.getElementById('prev').click();
    assert.deepEqual(JSON.parse(p.messages.at(-1)),{command:'select-date',date:'2026-09-21'});
    doc.getElementById('date-trigger').click();
    assert.equal(doc.querySelectorAll('.calendar-day.is-selected').length,3);
    assert.equal(doc.querySelector('.calendar-day[data-date="2026-10-01"]').disabled,true);
    assert.equal(doc.querySelectorAll('.app-row').length,2);
    assert.equal(p.chart.data.labels.length,7);
    assert.equal(p.chart.data.datasets[0].data[1],120);
    doc.querySelector('.app-button').click();
    assert.equal(doc.querySelectorAll('.app-row:first-child .app-hour-bar').length,7);
    assert.match(doc.querySelector('.app-hour-peak').textContent,/martes.*2 h 00 min/);
    p.chart.options.onClick({},[{index:1}]);
    assert.deepEqual(JSON.parse(p.messages.at(-1)),{command:'select-date',date:'2026-09-29'});
    assert.equal(doc.getElementById('mode-day').getAttribute('aria-pressed'),'true');
  } finally { p.close(); }
});
test('first render applies theme, correct totals and signals ready once', () => {
  const p = page();
  try {
    p.w.updateUsage(usage());
    assert.equal(p.w.document.documentElement.dataset.theme, 'dark');
    assert.equal(p.w.document.getElementById('total').textContent, '1 min');
    assert.equal(p.w.document.querySelector('.app-time').textContent, '1 min');
    assert.equal(p.chart.data.datasets[0].data[10], 1.5);
    assert.equal(p.chart.options.scales.y.max, 60);
    p.w.updateUsage(usage({ minutes: [2] }));
    assert.equal(p.messages.filter(x => x === 'dashboard-ready').length, 1);
    assert.equal(p.w.document.querySelectorAll('.app-row').length, 1);
    assert.equal(p.w.document.getElementById('next').disabled, true);
    p.w.document.getElementById('prev').click();
    assert.deepEqual(JSON.parse(p.messages.at(-1)), { command:'select-date', date:'2026-09-27' });
  } finally { p.close(); }
});
test('update time sits opposite the day label and changes only by minute', () => {
  const p=page();
  try {
    const doc=p.w.document, meta=doc.querySelector('.summary-meta'), updated=doc.getElementById('updated');
    assert.equal(meta.firstElementChild.id,'date-label');
    assert.equal(meta.lastElementChild.contains(updated),true);
    assert.equal(doc.querySelector('footer'),null);
    p.w.updateUsage(usage({updated:'04:26:17'}));
    assert.equal(updated.textContent,'04:26');
    assert.equal(updated.dateTime,'04:26');
    const textNode=updated.firstChild;
    p.w.updateUsage(usage({updated:'04:26:58'}));
    assert.equal(updated.firstChild,textNode);
    p.w.updateUsage(usage({updated:'04:27:01'}));
    assert.equal(updated.textContent,'04:27');
  } finally { p.close(); }
});
test('summary formatting, app labels, chart guides and compact app detail', () => {
  const p = page();
  try {
    const edge = Array.from({length:24}, (_,i) => i === 10 ? 359 : 0);
    p.w.updateUsage(usage({labels:['msedge.exe'],minutes:[359],hourly:[edge]}));
    const row = p.w.document.querySelector('.app-row');
    assert.equal(p.w.document.getElementById('total').textContent, '5 h 59 min');
    assert.equal(row.querySelector('.app-name').textContent, 'Microsoft Edge');
    assert.equal(row.querySelector('.app-time').textContent, '5 h 59 min');
    assert.equal(p.w.document.getElementById('status'), null);
    assert.equal(p.chart.options.scales.x.position, 'top');
    assert.equal(p.chart.options.scales.x.grid.display, false);
    assert.equal(p.chart.options.scales.y.display, true);
    assert.equal(p.chart.options.scales.y.max, 60);
    assert.equal(p.chart.data.datasets[0].borderRadius, 0);
    const button = row.querySelector('.app-button');
    button.click();
    assert.equal(button.getAttribute('aria-expanded'), 'true');
    assert.equal(row.querySelectorAll('.app-hour-bar').length, 24);
    assert.match(row.querySelector('.app-hour-peak').textContent, /10:00–11:00 · 5 h 59 min/);
    assert.match(row.querySelector('.app-hour-bar:nth-child(11)').getAttribute('aria-label'), /5 h 59 min/);
    assert.equal(p.w.document.querySelector('.hour-detail'), null);
  } finally { p.close(); }
});
test('empty daily hours cannot be selected and keyboard skips gaps in both charts', () => {
  const p=page();
  try {
    const doc=p.w.document,chart=doc.getElementById('chart'),inspector=doc.getElementById('hour-inspector');
    const values=Array.from({length:24},(_,hour)=>hour===9?10:hour===14?5:0);
    p.w.updateUsage(usage({minutes:[15],hourly:[values]}));
    p.chart.options.onClick(null,[{index:10}]);
    assert.equal(inspector.hidden,true);
    assert.equal(p.chart.data.datasets[0].data[10],null);
    chart.dispatchEvent(new p.w.KeyboardEvent('keydown',{key:'Home'}));
    assert.match(doc.getElementById('selected-hour').textContent,/09:00/);
    p.chart.options.onClick(null,[{index:10}]);
    assert.match(doc.getElementById('selected-hour').textContent,/09:00/);
    chart.dispatchEvent(new p.w.KeyboardEvent('keydown',{key:'ArrowRight'}));
    assert.match(doc.getElementById('selected-hour').textContent,/14:00/);
    chart.dispatchEvent(new p.w.KeyboardEvent('keydown',{key:'ArrowLeft'}));
    assert.match(doc.getElementById('selected-hour').textContent,/09:00/);
    chart.dispatchEvent(new p.w.KeyboardEvent('keydown',{key:'End'}));
    assert.match(doc.getElementById('selected-hour').textContent,/14:00/);
    doc.querySelector('.app-button').click();
    const bars=doc.querySelectorAll('.app-hour-bar');
    bars[9].focus();
    const readout=doc.querySelector('.app-hour-readout').textContent;
    for(let hour=0;hour<24;hour++) if(!values[hour]) {
      assert.equal(bars[hour].disabled,true);
      assert.equal(bars[hour].tabIndex,-1);
      bars[hour].dispatchEvent(new p.w.MouseEvent('mouseenter'));bars[hour].click();
      assert.equal(doc.querySelector('.app-hour-readout').textContent,readout);
    }
    bars[9].dispatchEvent(new p.w.KeyboardEvent('keydown',{key:'ArrowRight'}));
    assert.equal(doc.activeElement,bars[14]);
    bars[14].dispatchEvent(new p.w.KeyboardEvent('keydown',{key:'Home'}));
    assert.equal(doc.activeElement,bars[9]);
    p.w.updateUsage(usage({hourly:[Array(24).fill(0)]}));
    assert.equal(inspector.hidden,true);
    assert.equal(doc.querySelector('.app-hour-readout').hidden,true);
    assert.equal([...bars].every(bar=>bar.disabled && bar.tabIndex===-1),true);
  } finally { p.close(); }
});

test('hour inspector shows a ranked selection and supports keyboard exploration', () => {
  const p=page();
  try {
    const at10=value=>Array.from({length:24},(_,i)=>i===10?value:0);
    p.w.updateUsage(usage({labels:['one.exe','two.exe','three.exe','four.exe'],minutes:[20,15,10,.5],hourly:[at10(20),at10(15),at10(10),at10(.5)]}));
    const doc=p.w.document, canvas=doc.getElementById('chart'), inspector=doc.getElementById('hour-inspector');
    assert.equal(inspector.hidden,true);
    p.chart.options.onClick({},[{index:10}]);
    assert.equal(inspector.hidden,false);
    assert.equal(doc.getElementById('selected-total').textContent,'45 min');
    assert.equal(doc.querySelectorAll('.hour-inspector-app:not([hidden])').length,3);
    const more=doc.getElementById('more-hour-apps');
    assert.equal(more.textContent,'Ver 1 aplicación más');
    more.click();
    assert.equal(doc.querySelectorAll('.hour-inspector-app:not([hidden])').length,4);
    assert.match(doc.querySelector('.hour-inspector-app:last-child').textContent,/<1 min/);
    canvas.dispatchEvent(new p.w.KeyboardEvent('keydown',{key:'ArrowRight',bubbles:true}));
    assert.equal(doc.getElementById('selected-hour').textContent,'10:00–11:00');
    assert.equal(doc.getElementById('selected-total').textContent,'45 min');
    doc.getElementById('clear-hour').click();
    assert.equal(inspector.hidden,true);
    assert.equal(doc.activeElement,canvas);
  } finally { p.close(); }
});
test('app mini chart keeps focus and the open row across live updates', () => {
  const p=page();
  try {
    const at10=value=>Array.from({length:24},(_,i)=>i===10?value:0);
    const data={labels:['one.exe','two.exe'],minutes:[5,3],hourly:[at10(5),at10(3)]};
    p.w.updateUsage(usage(data));
    const rows=p.w.document.querySelectorAll('.app-row');
    rows[0].querySelector('.app-button').click();
    const bar=rows[0].querySelector('.app-hour-bar:nth-child(11)');
    const detail=rows[0].querySelector('.app-detail'), childCount=detail.children.length;
    bar.dispatchEvent(new p.w.MouseEvent('mouseenter'));
    assert.equal(detail.children.length,childCount);
    assert.equal(detail.lastElementChild.className,'app-hours-axis');
    assert.equal(detail.querySelector('.app-hour-readout').parentElement.className,'app-hour-peak');
    assert.equal(detail.querySelector('.app-peak-summary').hidden,true);
    bar.focus();
    assert.equal(p.w.document.activeElement,bar);
    assert.match(rows[0].querySelector('.app-hour-readout').textContent,/10:00–11:00 · 5 min/);
    p.w.updateUsage(usage({...data,minutes:[6,3],hourly:[at10(6),at10(3)]}));
    assert.equal(p.w.document.activeElement,bar);
    assert.equal(rows[0].querySelector('.app-detail').hidden,false);
    assert.match(rows[0].querySelector('.app-hour-readout').textContent,/6 min/);
    p.w.updateUsage(usage({labels:['two.exe','one.exe'],minutes:[7,6],hourly:[at10(7),at10(6)]}));
    const reordered=p.w.document.querySelectorAll('.app-row');
    assert.equal(reordered[1].querySelector('.app-detail').hidden,false);
    assert.equal(p.w.document.activeElement,reordered[1].querySelector('.app-hour-bar:nth-child(11)'));
    reordered[0].querySelector('.app-button').click();
    assert.equal(reordered[1].querySelector('.app-detail').hidden,true);
    assert.equal(reordered[0].querySelector('.app-detail').hidden,false);
    p.w.updateUsage(usage({...data,date:'2026-09-27'}));
    assert.equal(p.w.document.querySelector('.app-detail').hidden,true);
  } finally { p.close(); }
});
test('custom calendar respects history bounds and selects a day without native date input', () => {
  const p=page();
  try {
    p.w.updateUsage(usage({date:'2026-09-15',today:'2026-09-29',firstDate:'2026-08-28'}));
    const doc=p.w.document, trigger=doc.getElementById('date-trigger'), calendar=doc.getElementById('calendar');
    assert.equal(doc.querySelector('input[type=date]'),null);
    assert.equal(calendar.contains(doc.getElementById('today')),true);
    assert.equal(doc.getElementById('calendar-today').hidden,false);
    assert.match(trigger.textContent,/sept/i);
    trigger.click();
    assert.equal(calendar.hidden,false);
    assert.equal(trigger.getAttribute('aria-expanded'),'true');
    assert.equal(doc.querySelector('.calendar-day.is-selected').textContent,'15');
    assert.equal(doc.querySelectorAll('.calendar-day').length,35);
    assert.ok([...doc.querySelectorAll('.calendar-day.is-outside')].every(button=>button.disabled));
    doc.getElementById('calendar-prev').click();
    assert.equal(doc.querySelectorAll('.calendar-day').length,42);
    const day=number=>[...doc.querySelectorAll('.calendar-day:not(.is-outside)')].find(button=>button.textContent===String(number));
    assert.equal(day(27).disabled,true);
    assert.equal(day(28).disabled,false);
    doc.getElementById('calendar-next').click();
    day(14).click();
    assert.deepEqual(JSON.parse(p.messages.at(-1)),{command:'select-date',date:'2026-09-14'});
    assert.equal(calendar.hidden,true);
    p.w.updateUsage(usage({date:'2026-09-14',today:'2026-09-29',firstDate:'2026-08-28'}));
    assert.equal(calendar.hidden,true);
    trigger.click();
    assert.equal(doc.querySelector('.calendar-day.is-selected').textContent,'14');
    trigger.click();
    assert.equal(calendar.hidden,true);
    trigger.click();
    doc.dispatchEvent(new p.w.KeyboardEvent('keydown',{key:'Escape',bubbles:true}));
    assert.equal(calendar.hidden,true);
    trigger.click();
    doc.body.dispatchEvent(new p.w.Event('pointerdown',{bubbles:true}));
    assert.equal(calendar.hidden,true);
    trigger.click();
    doc.getElementById('today').click();
    assert.deepEqual(JSON.parse(p.messages.at(-1)),{command:'select-date',date:'2026-09-29'});
    assert.equal(calendar.hidden,true);
    p.w.updateUsage(usage({date:'2026-09-29',today:'2026-09-29',firstDate:'2026-08-28'}));
    assert.equal(doc.getElementById('calendar-today').hidden,true);
    assert.equal(calendar.hidden,true);
  } finally { p.close(); }
});
test('calendar month and year headings drill down and closes on date selection', () => {
  const p=page();
  try {
    p.w.updateUsage(usage({date:'2026-09-15',today:'2026-09-29',firstDate:'2023-03-15'}));
    const doc=p.w.document, calendar=doc.getElementById('calendar');
    doc.getElementById('date-trigger').click();
    doc.getElementById('calendar-year-label').click();
    assert.ok(doc.querySelector('.calendar-years'));
    const choice=label=>[...doc.querySelectorAll('.calendar-choice')].find(button=>button.textContent===label);
    assert.equal(choice('2022').disabled,true);
    assert.equal(choice('2027').disabled,true);
    choice('2023').click();
    assert.ok(doc.querySelector('.calendar-months'));
    assert.equal(choice('Febrero').disabled,true);
    doc.getElementById('calendar-year-label').click();
    choice('2024').click();
    choice('Febrero').click();
    assert.ok(doc.querySelector('.calendar-days'));
    const day=[...doc.querySelectorAll('.calendar-day')].find(button=>button.textContent==='10');
    day.click();
    assert.deepEqual(JSON.parse(p.messages.at(-1)),{command:'select-date',date:'2024-02-10'});
    assert.equal(calendar.hidden,true);
    p.w.updateUsage(usage({date:'2024-02-10',today:'2026-09-29',firstDate:'2023-03-15'}));
    doc.getElementById('date-trigger').click();
    assert.equal(doc.querySelector('.calendar-day.is-selected').textContent,'10');
    assert.equal(calendar.hidden,false);
  } finally { p.close(); }
});
test('calendar stays open across the focus gap before an internal pointer click', async () => {
  const p=page();
  try {
    p.w.updateUsage(usage({firstDate:'2026-01-01'}));
    const doc=p.w.document,calendar=doc.getElementById('calendar'),trigger=doc.getElementById('date-trigger');
    trigger.click();
    doc.activeElement.blur();
    // A browser event checkpoint can occur before the destination gains focus.
    await Promise.resolve();
    assert.equal(calendar.hidden,false);
    const month=doc.getElementById('calendar-month-label');
    month.focus();month.click();
    assert.equal(calendar.hidden,false);
    assert.ok(doc.querySelector('.calendar-months'));
    doc.getElementById('calendar-year-label').focus();
    doc.getElementById('calendar-year-label').click();
    assert.equal(calendar.hidden,false);
    assert.ok(doc.querySelector('.calendar-years'));
    doc.getElementById('settings').focus();
    assert.equal(calendar.hidden,true);
    assert.equal(trigger.getAttribute('aria-expanded'),'false');
    trigger.click();
    doc.getElementById('settings').dispatchEvent(new p.w.Event('pointerdown',{bubbles:true}));
    assert.equal(calendar.hidden,true);
  } finally { p.close(); }
});

test('calendar stays inside a narrow, short viewport and flips above its trigger', () => {
  const p=page();
  try {
    p.w.updateUsage(usage());
    const trigger=p.w.document.getElementById('date-trigger');
    const calendar=p.w.document.getElementById('calendar');
    Object.defineProperties(p.w,{innerWidth:{configurable:true,value:360},innerHeight:{configurable:true,value:480}});
    Object.defineProperty(calendar,'scrollHeight',{configurable:true,value:420});
    let top=50, bottom=90;
    trigger.getBoundingClientRect=()=>({left:25,width:260,right:285,top,bottom});
    trigger.click();
    assert.equal(p.w.getComputedStyle(calendar).position,'fixed');
    assert.ok(parseFloat(calendar.style.left)>=8);
    assert.ok(parseFloat(calendar.style.left)+parseFloat(calendar.style.width)<=352);
    assert.ok(parseFloat(calendar.style.top)+parseFloat(calendar.style.maxHeight)<=472);
    top=410;bottom=450;
    p.w.dispatchEvent(new p.w.Event('resize'));
    assert.ok(parseFloat(calendar.style.top)<top);
    assert.ok(parseFloat(calendar.style.top)>=8);
    assert.ok(parseFloat(calendar.style.top)+parseFloat(calendar.style.maxHeight)<=472);
  } finally { p.close(); }
});
test('empty day clears rows and pagehide releases timer and chart', () => {
  const p = page();
  try {
    p.w.updateUsage(usage()); p.w.updateUsage(usage({labels:[],minutes:[],hourly:[]}));
    assert.equal(p.w.document.getElementById('empty').hidden, false);
    assert.equal(p.w.document.querySelectorAll('.app-row').length, 0);
    p.w.dispatchEvent(new p.w.Event('pagehide'));
    assert.equal(p.cleared, true); assert.equal(p.chart.destroyed, true);
    assert.equal(p.w.document.getElementById('usage-tooltip'), null);
  } finally { p.close(); }
});
test('history error is visible, signals ready, retries and recovers without HTML injection', () => {
  const p = page();
  try {
    p.w.showUsageError('<script>bad()</script>');
    assert.equal(p.messages.at(-1), 'dashboard-ready');
    assert.equal(p.w.document.querySelector('main').hidden, true);
    assert.equal(p.w.document.getElementById('load-error-detail').textContent, '<script>bad()</script>');
    assert.equal(p.w.document.querySelector('#load-error script'), null);
    p.w.document.querySelector('#load-error button').click(); assert.equal(p.messages.at(-1), 'refresh');
    p.w.updateUsage(usage());
    assert.equal(p.w.document.getElementById('load-error'), null);
    assert.equal(p.w.document.querySelector('main').hidden, false);
  } finally { p.close(); }
});
test('script failure shows an error and configuration works independently of history', () => {
  const p = page(); const settings = page('settings');
  try {
    p.w.dispatchEvent(new p.w.ErrorEvent('error', { message: 'Chart failed' }));
    assert.equal(p.messages.at(-1), 'dashboard-ready');
    assert.equal(p.w.document.getElementById('load-error-detail').textContent, 'Chart failed');
    settings.w.updateUsage({darkMode:true,pauseWhenIdle:false,autostart:true});
    assert.equal(settings.messages.at(-1), 'settings-ready');
    assert.equal(settings.w.document.getElementById('autostart').checked, true);
    assert.equal(settings.w.document.getElementById('idle-pause').checked, false);
  } finally { p.close(); settings.close(); }
});
test('late icons reuse rows and hover never enters the dashboard layout or hitbox', () => {
  const p = page();
  try {
    p.w.updateUsage(usage()); const row = p.w.document.querySelector('.app-row');
    p.w.updateUsage(usage({icons:{'game.exe':'data:image/png;base64,AA=='}}));
    assert.equal(p.w.document.querySelector('.app-row'), row); assert.ok(row.querySelector('img'));
    const tip = p.w.document.getElementById('usage-tooltip');
    assert.equal(tip.parentElement,p.w.document.body);
    Object.defineProperties(tip,{offsetWidth:{value:280},offsetHeight:{value:300}});
    Object.defineProperties(p.w,{innerWidth:{value:360},innerHeight:{value:480}});
    p.chart.options.plugins.tooltip.external({chart:p.chart,tooltip:{opacity:1,dataPoints:[{dataIndex:10}],caretX:350,caretY:350}});
    assert.equal(tip.hidden,false);
    assert.equal(p.w.getComputedStyle(tip).position,'fixed');
    assert.equal(p.w.getComputedStyle(tip).pointerEvents,'none');
    assert.ok(parseFloat(tip.style.left)>=8 && parseFloat(tip.style.left)+280<=352);
    assert.ok(parseFloat(tip.style.top)>=8 && parseFloat(tip.style.top)+300<=472);
    assert.equal(tip.querySelectorAll('button').length,0);
    p.w.dispatchEvent(new p.w.Event('resize')); assert.equal(tip.hidden, true);
  } finally { p.close(); }
});
test('main chart tooltip fits its content and exposes extra apps in the persistent inspector', () => {
  const p=page();
  try {
    const hourly=Array.from({length:6},(_,app)=>Array.from({length:24},(_,hour)=>hour===10?app+1:hour===11&&app===0?2:0));
    p.w.updateUsage(usage({labels:Array.from({length:6},(_,i)=>`app${i}.exe`),minutes:[6,5,4,3,2,1],hourly}));
    const hover=hour=>p.chart.options.plugins.tooltip.external({chart:p.chart,tooltip:{opacity:1,dataPoints:[{dataIndex:hour}]}});
    hover(10);
    const tip=p.w.document.getElementById('usage-tooltip'), list=tip.querySelector('.tooltip-list');
    assert.equal(list.children.length,4);
    assert.equal(p.w.getComputedStyle(list).height,'auto');
    assert.equal(p.w.getComputedStyle(list.firstChild).height,'26px');
    assert.equal(tip.querySelector('.tooltip-hint').hidden,false);
    assert.match(tip.querySelector('.tooltip-hint').textContent,/\+2/);
    hover(11);
    assert.equal(list.children.length,1);
    assert.equal(p.w.getComputedStyle(list).height,'auto');
    assert.equal(tip.querySelector('.tooltip-hint').hidden,true);
  } finally { p.close(); }
});
test('icon arrival preserves category colors', () => {
  const p=page();
  try {
    p.w.updateUsage(usage({labels:['msedge.exe']}));
    const color=p.chart.data.datasets[0].backgroundColor;
    p.w.updateUsage(usage({labels:['msedge.exe'],icons:{'msedge.exe':'data:image/png;base64,AA=='}}));
    assert.equal(p.chart.data.datasets[0].backgroundColor,color);
    assert.equal(p.w.document.querySelector('.swatch').style.getPropertyValue('--app-color'),color);
  } finally {p.close();}
});

test('adjacent bars remain available while the detail is open', () => {
  const p = page();
  try {
    const hours = Array.from({length:24}, (_,i)=>i===10 || i===11 ? 8 : 0);
    p.w.updateUsage(usage({hourly:[hours]}));
    const tip=p.w.document.getElementById('usage-tooltip');
    const hover=hour=>p.chart.options.plugins.tooltip.external({chart:p.chart,tooltip:{opacity:1,dataPoints:[{dataIndex:hour}]}});
    hover(10);
    assert.equal(tip.classList.contains('is-visible'),true);
    p.chart.options.onClick({},[{index:10}]);
    assert.equal(p.w.document.getElementById('hour-inspector').hidden,false);
    assert.equal(p.w.document.getElementById('selected-hour').textContent,'10:00–11:00');
    assert.match(tip.querySelector('.tooltip-heading').textContent,/10:00/);
    hover(11);
    assert.match(tip.querySelector('.tooltip-heading').textContent,/11:00/);
    tip.querySelector('.tooltip-list').dispatchEvent(new p.w.Event('scroll',{bubbles:true}));
    assert.equal(tip.hidden,false);
    hover(10);
    assert.match(tip.querySelector('.tooltip-heading').textContent,/10:00/);
  } finally { p.close(); }
});
test('wheel over chart leaves page scrolling available', () => {
  const p=page();
  try {
    p.w.updateUsage(usage());
    const tip=p.w.document.getElementById('usage-tooltip');
    const external=p.chart.options.plugins.tooltip.external;
    external({chart:p.chart,tooltip:{opacity:1,dataPoints:[{dataIndex:10}]}});
    const list=tip.querySelector('.tooltip-list');
    Object.defineProperties(list,{scrollHeight:{value:600},clientHeight:{value:200}});
    const wheel=new p.w.WheelEvent('wheel',{deltaY:120,cancelable:true});
    p.chart.canvas.dispatchEvent(wheel);
    assert.equal(wheel.defaultPrevented,false);
    assert.equal(list.scrollTop,0);
    external({chart:p.chart,tooltip:{opacity:1,dataPoints:[{dataIndex:10}]}});
    assert.equal(list.scrollTop,0);
    const zoom=new p.w.WheelEvent('wheel',{deltaY:100,ctrlKey:true,cancelable:true});
    p.chart.canvas.dispatchEvent(zoom);
    assert.equal(zoom.defaultPrevented,false);
    p.w.dispatchEvent(new p.w.Event('resize'));
    const ordinary=new p.w.WheelEvent('wheel',{deltaY:100,cancelable:true});
    p.chart.canvas.dispatchEvent(ordinary);
    assert.equal(ordinary.defaultPrevented,false);
  } finally { p.close(); }
});

test('empty hours fade out and returning to a bar cancels the pending exit', () => {
  const p=page();
  try {
    let now=0, nextId=0;
    const timers=new Map();
    p.w.setTimeout=(callback,delay)=>{const id=++nextId;timers.set(id,{callback,at:now+delay});return id;};
    p.w.clearTimeout=id=>timers.delete(id);
    const advance=ms=>{
      const end=now+ms;
      while(true) {
        const next=[...timers].filter(([,timer])=>timer.at<=end).sort((a,b)=>a[1].at-b[1].at)[0];
        if(!next) break;
        now=next[1].at;timers.delete(next[0]);next[1].callback();
      }
      now=end;
    };
    p.w.updateUsage(usage());
    const tip=p.w.document.getElementById('usage-tooltip');
    const hover=hour=>p.chart.options.plugins.tooltip.external({chart:p.chart,tooltip:{opacity:1,dataPoints:[{dataIndex:hour}]}});
    hover(10);hover(11);
    assert.equal(tip.hidden,false);
    advance(30);hover(12);advance(20);
    assert.equal(tip.classList.contains('is-visible'),false);
    assert.equal(tip.hidden,false,'keep the card mounted throughout its exit');
    advance(60);hover(10);advance(200);
    assert.equal(tip.hidden,false,'an old exit must not hide a newly shown card');
    assert.equal(tip.classList.contains('is-visible'),true);
    hover(11);advance(189);
    assert.equal(tip.hidden,false);
    advance(1);
    assert.equal(tip.hidden,true);
    hover(10);
    assert.equal(tip.hidden,false);
    assert.equal(tip.classList.contains('is-visible'),true);
  } finally { p.close(); }
});

test('closed ranked rows retain identity and keyboard focus, with a fallback on removal',()=>{
  const p=page();try {
    const doc=p.w.document, data=usage({labels:['a.exe','b.exe'],minutes:[2,1],hourly:[Array(24).fill(0),Array(24).fill(0)]});
    p.w.updateUsage(data);const button=doc.querySelectorAll('.app-button')[1];button.focus();
    p.w.updateUsage({...data,labels:['b.exe','a.exe']});
    assert.equal(doc.activeElement,button);assert.equal(doc.querySelector('.app-button'),button);
    p.w.updateUsage(usage({labels:['a.exe']}));assert.equal(doc.activeElement,doc.querySelector('.app-button'));
    p.w.updateUsage(usage({labels:[],minutes:[],hourly:[]}));assert.equal(doc.activeElement,doc.getElementById('date-trigger'));
  } finally{p.close();}
});

test('selected chart descriptions survive repeated snapshots in both periods',()=>{
  const p=page();try {
    const doc=p.w.document,data=usage();p.w.updateUsage(data);const chart=doc.getElementById('chart');
    chart.dispatchEvent(new p.w.KeyboardEvent('keydown',{key:'ArrowRight'}));const label=chart.getAttribute('aria-label');
    p.w.updateUsage(data);p.w.updateUsage(data);assert.equal(chart.getAttribute('aria-label'),label);assert.match(label,/10:00–11:00.*1 min/);
    doc.getElementById('mode-week').click();chart.dispatchEvent(new p.w.KeyboardEvent('keydown',{key:'Home'}));
    const weekLabel=chart.getAttribute('aria-label');p.w.updateUsage(data);assert.equal(chart.getAttribute('aria-label'),weekLabel);
  } finally{p.close();}
});

test('calendar enters the date, supports arrows across months and restores focus between levels',()=>{
  const p=page();try{
    const doc=p.w.document;p.w.updateUsage(usage({date:'2026-08-31',today:'2026-09-30',firstDate:'2023-01-01'}));
    const trigger=doc.getElementById('date-trigger');trigger.click();assert.equal(doc.activeElement.dataset.date,'2026-08-31');
    assert.equal(doc.querySelectorAll('#calendar-days [tabindex="0"]').length,1);
    doc.activeElement.dispatchEvent(new p.w.KeyboardEvent('keydown',{key:'ArrowRight',bubbles:true,cancelable:true}));
    assert.equal(doc.activeElement.dataset.date,'2026-09-01');
    const year=doc.getElementById('calendar-year-label');year.focus();year.click();assert.ok(doc.activeElement.matches('.calendar-choice'));
    doc.activeElement.click();assert.ok(doc.activeElement.matches('.calendar-months .calendar-choice'));
    doc.activeElement.click();assert.ok(doc.activeElement.matches('.calendar-day'));
    doc.activeElement.click();assert.equal(doc.getElementById('calendar').hidden,true);assert.equal(doc.activeElement,trigger);
  }finally{p.close();}
});

test('subminute activity is readable, accessible and every chart category has a legend',()=>{
  const p=page();try{
    const doc=p.w.document,labels=Array.from({length:7},(_,i)=>`app${i}.exe`),hourly=labels.map(()=>Array.from({length:24},(_,h)=>h===10?.5:0));
    p.w.updateUsage(usage({labels,minutes:labels.map(()=>.5),hourly}));
    assert.equal(doc.querySelector('.app-time').textContent,'<1 min');assert.match(doc.querySelector('.app-button').getAttribute('aria-label'),/<1 min/);
    assert.equal(doc.querySelectorAll('.legend-item').length,6);assert.equal(p.chart.options.scales.y.ticks.callback(30),'30 min');
    p.chart.options.plugins.tooltip.external({chart:p.chart,tooltip:{opacity:1,dataPoints:[{dataIndex:10}]}});
    assert.equal(doc.querySelector('.tooltip-time').textContent,'<1 min');
    p.chart.options.onClick({},[{index:10}]);doc.getElementById('more-hour-apps').click();assert.equal([...doc.querySelectorAll('.hour-inspector-app')].filter(row=>!row.hidden).length,7);
  }finally{p.close();}
});

test('manual and automatic tracking pauses are distinct and partial average explains the divisor',()=>{
  const p=page();try{
    const doc=p.w.document,data=usage({trackingState:'manual',today:'2026-09-30'});p.w.updateUsage(data);
    assert.equal(doc.getElementById('pause-tracking').textContent,'Reanudar');doc.getElementById('pause-tracking').click();assert.equal(p.messages.at(-1),'toggle-pause');
    p.w.updateUsage({...data,trackingState:'idle'});assert.match(doc.getElementById('tracking-state').textContent,/inactividad/);assert.equal(doc.getElementById('pause-tracking').textContent,'Pausar');
    doc.getElementById('mode-week').click();assert.match(doc.getElementById('date-label').textContent,/Promedio diario registrado/);assert.match(doc.getElementById('coverage-method').textContent,/días de calendario.*no certifican cero uso/);assert.equal(doc.getElementById('coverage-note').classList.contains('incomplete'),true);
  }finally{p.close();}
});

test('privacy actions are available and errors offer configuration with copyable details',()=>{
  const settings=page('settings');try{
    settings.w.updateUsage({darkMode:false,autostart:false,pauseWhenIdle:false,dataLocation:'C:\\Synthetic'});
    const doc=settings.w.document;assert.equal(doc.getElementById('data-location').textContent,'C:\\Synthetic');
    for(const command of ['open-data-folder','export-history','clear-history']){doc.getElementById(command).click();assert.equal(settings.messages.at(-1),command);}
  }finally{settings.close();}
  const p=page();try{p.w.showUsageError('Guardado pendiente; detalle sintético');const doc=p.w.document;assert.match(doc.querySelector('#load-error h1').textContent,/guardar/);assert.equal(p.w.getComputedStyle(doc.getElementById('load-error-detail')).userSelect,'text');[...doc.querySelectorAll('#load-error button')].find(button=>button.textContent==='Configuración').click();assert.equal(p.messages.at(-1),'open-settings');}finally{p.close();}
});

test('calendar selection text exceeds 4.5:1 in both themes',()=>{
  const css=fs.readFileSync('assets/dashboard.css','utf8');
  const luminance=hex=>{const channels=hex.replace('#','').match(/../g).map(value=>parseInt(value,16)/255).map(value=>value<=.04045?value/12.92:((value+.055)/1.055)**2.4);return channels[0]*.2126+channels[1]*.7152+channels[2]*.0722;};
  for(const [theme,background,text] of [['light','#006de0','#ffffff'],['dark','#65b1ff','#141416']]){
    assert.ok(css.includes(`--accent:${background}`));assert.ok(css.includes(theme==='light'?'--on-accent:#fff':`--on-accent:${text}`));
    const a=luminance(background),b=luminance(text),contrast=(Math.max(a,b)+.05)/(Math.min(a,b)+.05);assert.ok(contrast>=4.5,`${theme}: ${contrast}`);
  }
  assert.match(css,/calendar-choice\.is-selected:hover\{color:var\(--on-accent\)\}/);
});


test('error entry, repeated errors and recovery preserve a visible focus target',()=>{
  for(const originId of ['chart','date-trigger']) {
    const p=page();try {
      const doc=p.w.document;p.w.updateUsage(usage());const origin=doc.getElementById(originId);origin.focus();
      p.w.showUsageError('Error sintético');assert.equal(doc.activeElement.id,'load-error-title');assert.equal(doc.querySelector('main').hidden,true);
      const retry=[...doc.querySelectorAll('#load-error button')].find(button=>button.textContent==='Reintentar');retry.focus();
      p.w.showUsageError('Error sintético repetido');assert.equal(doc.activeElement,retry);
      p.w.updateUsage(usage());assert.equal(doc.activeElement,origin);assert.equal(doc.querySelector('main').hidden,false);
    }finally{p.close();}
  }
});

test('error recovery falls back when the origin disappears and respects navigation in settings',()=>{
  const p=page();try{
    const doc=p.w.document;p.w.updateUsage(usage());doc.querySelector('.app-button').focus();p.w.showUsageError('Error sintético');
    p.w.updateUsage(usage({labels:[],minutes:[],hourly:[]}));assert.equal(doc.activeElement.id,'settings');assert.equal(doc.activeElement.closest('[hidden]'),null);
  }finally{p.close();}
  const settings=page('settings');try{
    const doc=settings.w.document;doc.getElementById('done').focus();settings.w.showUsageError('Error sintético');
    doc.getElementById('dark-mode').focus();settings.w.clearUsageError();assert.equal(doc.activeElement.id,'dark-mode');
  }finally{settings.close();}
});

test('all visible app categories have unique colors distinct from Others through ranking, dates and modes',()=>{
  const p=page();try{
    const labels=['a.exe','b.exe','c.exe','d.exe','e.exe','f.exe','g.exe','h.exe','i.exe','j.exe','k.exe'];
    const data=usage({labels,minutes:labels.map(()=>1),hourly:labels.map(()=>Array(24).fill(1)),week:{...usage().week,labels,minutes:labels.map(()=>1),daily:labels.map(()=>Array(7).fill(1))}});
    const check=()=>{const sets=p.chart.data.datasets,colors=sets.slice(0,5).map(set=>set.backgroundColor);assert.equal(new Set(colors).size,5);assert.ok(!colors.includes(sets[5].backgroundColor));};
    p.w.updateUsage(data);check();const reversed=[...labels].reverse();
    p.w.updateUsage({...data,labels:reversed,date:'2026-09-27'});check();
    p.w.updateUsage({...data,labels:['f.exe',...labels.filter(name=>name!=='f.exe')]});check();
    p.w.document.getElementById('mode-week').click();check();
    p.w.updateUsage({...data,week:{...data.week,labels:reversed}});check();
    p.w.document.getElementById('mode-day').click();check();
  }finally{p.close();}
});

test('methodology remains keyboard accessible in collapsed help and privacy avoids internal terms',()=>{
  const p=page();try{
    const doc=p.w.document;p.w.updateUsage(usage({today:'2026-09-30',week:{...usage().week,averageDays:3}}));doc.getElementById('mode-week').click();
    const help=doc.querySelector('.usage-help');assert.equal(help.open,false);assert.match(doc.getElementById('coverage-note').textContent,/Cobertura parcial: 1 de 3 días/);
    assert.match(doc.getElementById('coverage-method').textContent,/cero al cálculo; no certifican cero uso/);
    help.querySelector('summary').focus();help.open=true;p.w.updateUsage(usage());assert.equal(help.open,true);assert.equal(doc.activeElement,help.querySelector('summary'));
    assert.equal(doc.querySelectorAll('.legend-time').length,0);assert.match(help.textContent,/Cerrar esta ventana mantiene el seguimiento/);
    assert.doesNotMatch(doc.querySelector('main').textContent,/\btracker\b|\bhooks\b|\bbackups\b/i);
  }finally{p.close();}
  const settings=page('settings');try{const doc=settings.w.document;assert.deepEqual([...doc.querySelectorAll('.privacy-section h3')].map(node=>node.textContent),['Qué se guarda','Dónde está','Qué elimina el borrado']);assert.doesNotMatch(doc.querySelector('main').textContent,/\btracker\b|\bhooks\b|\bbackups\b/i);assert.match(doc.querySelector('main').textContent,/exportaciones y copias de seguridad/);}finally{settings.close();}
});

test('weekly comparison handles increases, decreases, zero and absent history', () => {
  const p=page();
  try {
    const data=usage();
    const update=(average,previous)=>p.w.updateUsage({...data,week:{...data.week,averageMinutes:average,previousAverageMinutes:previous}});
    update(150,100);p.w.document.getElementById('mode-week').click();
    const comparison=p.w.document.getElementById('week-comparison');
    assert.equal(comparison.textContent,'↑ 50% más que la semana pasada');
    update(50,100);assert.equal(comparison.textContent,'↓ 50% menos que la semana pasada');
    update(0,100);assert.match(comparison.textContent,/100% menos/);
    update(100,100);assert.match(comparison.textContent,/Sin cambios/);
    update(100,0);assert.match(comparison.textContent,/no hubo uso registrado/);
    update(100,null);assert.match(comparison.textContent,/Sin comparación anterior/);
    p.w.document.getElementById('mode-day').click();assert.equal(comparison.hidden,true);
  } finally {p.close();}
});
