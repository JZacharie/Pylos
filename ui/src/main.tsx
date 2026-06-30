import { StrictMode } from 'react'
import { createRoot } from 'react-dom/client'
import './index.css'
import App from './App.tsx'

import { openobserveRum } from '@openobserve/browser-rum';
import { openobserveLogs } from '@openobserve/browser-logs';

const clientToken = import.meta.env.VITE_OPENOBSERVE_CLIENT_TOKEN;
const applicationId = import.meta.env.VITE_OPENOBSERVE_APPLICATION_ID;
const site = import.meta.env.VITE_OPENOBSERVE_SITE || 'o2-openobserve.p.zacharie.org';

if (clientToken && applicationId) {
  openobserveRum.init({
    applicationId,
    clientToken,
    site,
    organizationIdentifier: import.meta.env.VITE_OPENOBSERVE_ORG_ID || 'default',
    service: import.meta.env.VITE_OPENOBSERVE_SERVICE || 'pylos-ui',
    env: import.meta.env.VITE_OPENOBSERVE_ENV || 'production',
    version: import.meta.env.VITE_OPENOBSERVE_VERSION || '0.1.0',
    trackResources: true,
    trackLongTasks: true,
    trackUserInteractions: true,
    apiVersion: 'v1',
    insecureHTTP: false,
    defaultPrivacyLevel: 'allow',
    sessionSampleRate: 100,
    sessionReplaySampleRate: 50,
  });

  openobserveLogs.init({
    clientToken,
    site,
    organizationIdentifier: import.meta.env.VITE_OPENOBSERVE_ORG_ID || 'default',
    service: import.meta.env.VITE_OPENOBSERVE_SERVICE || 'pylos-ui',
    env: import.meta.env.VITE_OPENOBSERVE_ENV || 'production',
    version: import.meta.env.VITE_OPENOBSERVE_VERSION || '0.1.0',
    forwardErrorsToLogs: true,
    insecureHTTP: false,
    apiVersion: 'v1',
  });

  openobserveRum.startSessionReplayRecording();
}

createRoot(document.getElementById('root')!).render(
  <StrictMode>
    <App />
  </StrictMode>,
)
