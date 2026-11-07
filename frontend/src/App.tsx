import { BrowserRouter, Routes, Route, Navigate } from 'react-router-dom';
import { DashboardLayout } from './components/layout/DashboardLayout';
import { ProtectedRoute } from './components/layout/ProtectedRoute';
import { Dashboard } from './views/Dashboard';
import WebsitesView from './views/Websites';
import DatabasesView from './views/Databases';
import BackupsView from './views/Backups';
import SecurityRulesUI from './views/Security';
import AdministrationView from './views/Administration';
import LoginView from './views/Login';

function App() {
  return (
    <BrowserRouter>
      <Routes>
        <Route path="/login" element={<LoginView />} />

        <Route element={<ProtectedRoute />}>
          <Route path="/" element={<DashboardLayout><Dashboard /></DashboardLayout>} />
          <Route path="/websites" element={<DashboardLayout><WebsitesView /></DashboardLayout>} />
          <Route path="/databases" element={<DashboardLayout><DatabasesView /></DashboardLayout>} />
          <Route path="/backups" element={<DashboardLayout><BackupsView /></DashboardLayout>} />
          <Route path="/security" element={<DashboardLayout><SecurityRulesUI /></DashboardLayout>} />
          <Route path="/administration" element={<DashboardLayout><AdministrationView /></DashboardLayout>} />
        </Route>

        <Route path="*" element={<Navigate to="/" replace />} />
      </Routes>
    </BrowserRouter>
  );
}

export default App;
