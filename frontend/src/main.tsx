import { StrictMode } from 'react'
import { createRoot } from 'react-dom/client'
import './index.css'
import App from './App.tsx'
import { PortalReviewQualification } from './components/PortalImageReview'

createRoot(document.getElementById('root')!).render(
  <StrictMode>
    {window.location.hash === '#portal-consent-qualification' ? <PortalReviewQualification /> : <App />}
  </StrictMode>,
)
