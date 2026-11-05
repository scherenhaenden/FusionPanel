import { BrowserRouter, Routes, Route, Navigate } from 'react-router-dom';
import { DashboardLayout } from './components/layout/DashboardLayout';
import { ProtectedRoute } from './components/layout/ProtectedRoute';
import WebsitesView from './views/Websites';
import DatabasesView from './views/Databases';
import BackupsView from './views/Backups';
import SecurityRulesUI from './views/Security';
import LoginView from './views/Login';

function Dashboard() {
  return (
    <>
      <div className="grid grid-cols-1 gap-4 mb-4">
        <div className="flex items-center justify-center h-24 rounded-sm bg-gray-50 dark:bg-gray-800">
          <p className="text-2xl text-gray-400 dark:text-gray-500">
            Welcome to Administration
          </p>
        </div>
      </div>
      <div className="grid grid-cols-2 gap-4 mb-4">
        <div className="flex items-center justify-center rounded-sm bg-gray-50 h-28 dark:bg-gray-800">
          <p className="text-2xl text-gray-400 dark:text-gray-500">
            Card 1
          </p>
        </div>
        <div className="flex items-center justify-center rounded-sm bg-gray-50 h-28 dark:bg-gray-800">
          <p className="text-2xl text-gray-400 dark:text-gray-500">
            Card 2
          </p>
        </div>
      </div>
    </>
  );
}

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
        </Route>

        <Route path="*" element={<Navigate to="/" replace />} />
      </Routes>
    </BrowserRouter>
  );
}

export default App;
