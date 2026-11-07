import React from 'react';
import { render, screen, fireEvent } from '@testing-library/react';
import { MemoryRouter, Routes, Route } from 'react-router-dom';
import '@testing-library/jest-dom';
import AdministrationView from '../views/Administration';

describe('AdministrationView', () => {
  it('renders the Administration title and tabs', () => {
    render(<AdministrationView />);

    // Check title
    expect(screen.getByText('Administration')).toBeInTheDocument();

    // Check tabs
    expect(screen.getByText('Operators')).toBeInTheDocument();
    expect(screen.getByText('Audit Logs')).toBeInTheDocument();
  });

  it('renders the Operators tab by default and switches to Audit Logs', () => {
    render(<AdministrationView />);

    // Check that operators are rendered
    expect(screen.getByText('admin@example.com')).toBeInTheDocument();
    expect(screen.getByText('Invite Operator')).toBeInTheDocument();

    // Switch to Audit Logs
    const auditTab = screen.getByText('Audit Logs');
    fireEvent.click(auditTab);

    // Check that audit logs are rendered
    expect(screen.getByText('Successful login')).toBeInTheDocument();
    // Invite button should not be present
    expect(screen.queryByText('Invite Operator')).not.toBeInTheDocument();
  });

  it('opens and closes the Invite Operator modal', () => {
    render(<AdministrationView />);

    const inviteBtn = screen.getByText('Invite Operator');
    fireEvent.click(inviteBtn);

    // Check that the modal opened
    expect(screen.getByText('Email Address')).toBeInTheDocument();
    expect(screen.getByRole('button', { name: 'Send Invite' })).toBeInTheDocument();

    // Close modal
    const cancelBtn = screen.getByText('Cancel');
    fireEvent.click(cancelBtn);

    // Check that it's closed
    expect(screen.queryByText('Email Address')).not.toBeInTheDocument();
  });

  it('verifies the presence of dark mode classes', () => {
    const { container } = render(<AdministrationView />);
    const heading = screen.getByText('Administration');
    // Ensure dark mode text classes are present
    expect(heading).toHaveClass('dark:text-white');

    // Check main container or elements for dark background classes
    // The table container has dark:bg-gray-800
    const tableContainer = container.querySelector('.dark\\:bg-gray-800');
    expect(tableContainer).toBeInTheDocument();
  });

  it('can be loaded via routing', () => {
    render(
      <MemoryRouter initialEntries={['/administration']}>
        <Routes>
          <Route path="/administration" element={<AdministrationView />} />
        </Routes>
      </MemoryRouter>
    );
    expect(screen.getByText('Administration')).toBeInTheDocument();
    expect(screen.getByText('Operators')).toBeInTheDocument();
  });
});
