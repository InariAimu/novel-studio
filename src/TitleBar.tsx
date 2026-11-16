import { isTauri } from '@tauri-apps/api/core';
import { getCurrentWindow } from '@tauri-apps/api/window';
import { Copy, Minus, Square, X } from 'lucide-react';
import { useEffect, useState } from 'react';

const appWindow = isTauri() ? getCurrentWindow() : null;

export default function TitleBar() {
  const [maximized, setMaximized] = useState(false);
  const [error, setError] = useState('');

  useEffect(() => {
    if (!appWindow) return;

    let active = true;
    const update = () => {
      void appWindow.isMaximized().then(value => {
        if (active) setMaximized(value);
      }).catch(() => {
        if (active) setError('无法读取窗口状态');
      });
    };
    const listener = appWindow.onResized(update);
    void listener.then(update).catch(() => {
      if (active) setError('无法监听窗口状态');
    });

    return () => {
      active = false;
      void listener.then(unlisten => unlisten()).catch(() => {});
    };
  }, []);

  function runWindowAction(action: 'minimize' | 'toggleMaximize' | 'close') {
    setError('');
    void appWindow?.[action]().catch(() => setError('窗口操作失败'));
  }

  return (
    <header className="titlebar">
      <div className="titlebar-drag" data-tauri-drag-region>
        <img src="/app-icon.png" width="24" height="24" alt="" draggable={false} />
        <span data-tauri-drag-region>Novel Studio</span>
      </div>
      {error && <span className="window-error" role="alert">{error}</span>}
      {appWindow && (
        <div className="window-controls">
          <button type="button" aria-label="最小化窗口" onClick={() => runWindowAction('minimize')}><Minus /></button>
          <button type="button" aria-label={maximized ? '还原窗口' : '最大化窗口'} onClick={() => runWindowAction('toggleMaximize')}>
            {maximized ? <Copy /> : <Square />}
          </button>
          <button type="button" className="window-close" aria-label="关闭窗口" onClick={() => runWindowAction('close')}><X /></button>
        </div>
      )}
    </header>
  );
}
