import React from 'react';
import { render, screen } from '@testing-library/react';
import { MemoryRouter, Routes, Route } from 'react-router-dom';
import '@testing-library/jest-dom';
import { DashboardLayout } from '../components/layout/DashboardLayout';
import WebsitesView from '../views/Websites';
import DatabasesView from '../views/Databases';
import BackupsView from '../views/Backups';
import SecurityRulesUI from '../views/Security';

// Note: To test routes effectively, we'll test the views directly
// wrapped in MemoryRouter + DashboardLayout to simulate routes.

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
    </>
  );
}

describe('Smoke Tests for Routes', () => {
  const renderWithRouter = (ui: React.ReactElement, { route = '/' } = {}) => {
    return render(
      <MemoryRouter initialEntries={[route]}>
        <DashboardLayout>
          <Routes>
            <Route path="/" element={<Dashboard />} />
            <Route path="/websites" element={<WebsitesView />} />
            <Route path="/databases" element={<DatabasesView />} />
            <Route path="/backups" element={<BackupsView />} />
            <Route path="/security" element={<SecurityRulesUI />} />
          </Routes>
        </DashboardLayout>
      </MemoryRouter>
    );
  };

  it('renders Dashboard correctly', () => {
    renderWithRouter(<Dashboard />, { route: '/' });
    expect(screen.getByText('Welcome to Administration')).toBeInTheDocument();
  });

  it('renders WebsitesView correctly on /websites', () => {
    renderWithRouter(<WebsitesView />, { route: '/websites' });
    expect(screen.getByText('Websites & Virtual Hosts')).toBeInTheDocument();
  });

  it('renders DatabasesView correctly on /databases', () => {
    renderWithRouter(<DatabasesView />, { route: '/databases' });
    expect(screen.getByText('Databases Management')).toBeInTheDocument();
  });

  it('renders BackupsView correctly on /backups', () => {
    renderWithRouter(<BackupsView />, { route: '/backups' });
    expect(screen.getByText('Backup & Restore Management')).toBeInTheDocument();
  });

  it('renders SecurityRulesUI correctly on /security', () => {
    renderWithRouter(<SecurityRulesUI />, { route: '/security' });
    expect(screen.getByText('Security & Firewall Rules')).toBeInTheDocument();
  });
});
