(() => {
  let ready = false, errorOrigin = null;
  const usable = node => {
    if(!node?.isConnected || typeof node.focus !== 'function' || node.disabled || node.closest('[hidden]')) return false;
    for(let parent=node;parent;parent=parent.parentElement) {
      const style=getComputedStyle(parent);
      if(style.display==='none' || style.visibility==='hidden') return false;
      if(parent.tagName==='DETAILS' && !parent.open && !parent.querySelector('summary')?.contains(node)) return false;
    }
    return node!==document.body && node!==document.documentElement;
  };
  window.markViewReady = () => {
    if (ready) return;
    ready = true;
    window.ipc.postMessage(document.body.dataset.view === 'settings' ? 'settings-ready' : 'dashboard-ready');
  };
  window.clearUsageError = () => {
    const panel=document.getElementById('load-error');
    const shouldRestore=panel && (panel.contains(document.activeElement) || !usable(document.activeElement));
    const main=document.querySelector('main');
    if(main)main.hidden=false;
    panel?.remove();
    if(shouldRestore) {
      const origin=usable(errorOrigin?.node)?errorOrigin.node:document.getElementById(errorOrigin?.id);
      const target=usable(origin)?origin:[...main?.querySelectorAll('button,input,summary,[tabindex="0"]') || []].find(usable);
      target?.focus();
    }
    if(panel)errorOrigin=null;
  };
  window.showUsageError = message => {
    let panel = document.getElementById('load-error');
    const entering=!panel;
    if (entering) {
      const origin=document.activeElement;errorOrigin={node:origin,id:origin?.id};
      panel = document.createElement('section');
      panel.id = 'load-error'; panel.className = 'config'; panel.setAttribute('role', 'alert');
      const title = document.createElement('h1'); title.id='load-error-title';title.tabIndex=-1; title.textContent = 'No se pudo cargar la información';
      const explanation=document.createElement('p'); explanation.textContent='No pudimos leer o guardar el historial. Podés reintentar o revisar Configuración. Copiá los detalles si necesitás ayuda.';
      const detail = document.createElement('p'); detail.id = 'load-error-detail';
      const retry = document.createElement('button'); retry.textContent = 'Reintentar';
      retry.onclick = () => window.ipc.postMessage('refresh');
      const settings=document.createElement('button');settings.textContent='Configuración';settings.onclick=()=>window.ipc.postMessage('open-settings');
      panel.append(title, explanation, detail, retry);if(document.body.dataset.view!=='settings')panel.append(settings); document.body.appendChild(panel);
    }
    const title=document.getElementById('load-error-title'),detail=document.getElementById('load-error-detail');
    const heading=String(message).startsWith('Guardado pendiente')?'No se pudo guardar el historial':'No se pudo cargar la información';
    if(title.textContent!==heading)title.textContent=heading;
    if(detail.textContent!==String(message))detail.textContent=String(message);
    const main = document.querySelector('main');
    if (main) main.hidden = document.body.dataset.view !== 'settings';
    if(entering)document.getElementById('load-error-title').focus();
    window.markViewReady();
  };
  window.addEventListener('error', event => window.showUsageError(event.message || 'La interfaz no pudo iniciarse. Cerrá esta ventana y volvé a abrirla.'));
  window.addEventListener('unhandledrejection', () => window.showUsageError('La interfaz no pudo completar la carga. Volvé a intentarlo.'));
})();
