import React from 'react';
import { render, screen, fireEvent, waitFor } from '@testing-library/react';
import '@testing-library/jest-dom';
import DatabasesView from '../views/Databases';
import { api } from '../api';
import MockAdapter from 'axios-mock-adapter';
import { apiClient } from '../api/client';

describe('DatabasesView (Unit Evidence)', () => {
  let mock: MockAdapter;

  beforeEach(() => {
    mock = new MockAdapter(apiClient);
  });

  afterEach(() => {
    mock.restore();
    jest.clearAllMocks();
  });

  it('renders a list of databases successfully', async () => {
    mock.onGet('/v1/databases').reply(200, [
      { id: '1', name: 'db_one', engine: 'mariadb', state: 'ready' },
      { id: '2', name: 'db_two', engine: 'mariadb', state: 'creating' }
    ]);
    mock.onGet('/v1/capabilities').reply(200, {
      capabilities: ['database.create', 'database.delete'],
      unavailable: ['database.backup', 'database.restore']
    });

    render(<DatabasesView />);

    expect(screen.getByText('Loading...')).toBeInTheDocument();

    await waitFor(() => {
      expect(screen.queryByText('Loading...')).not.toBeInTheDocument();
    });

    expect(screen.getByText('db_one')).toBeInTheDocument();
    expect(screen.getByText('db_two')).toBeInTheDocument();
  });

  it('shows error state when fetching fails', async () => {
    mock.onGet('/v1/databases').reply(503, { error: { message: 'dependency_unavailable' } });
    mock.onGet('/v1/capabilities').reply(200, { capabilities: [] });

    render(<DatabasesView />);

    await waitFor(() => {
      expect(screen.getByText('dependency_unavailable')).toBeInTheDocument();
    });
  });

  it('handles database creation successfully', async () => {
    mock.onGet('/v1/databases').reply(200, []);
    mock.onGet('/v1/capabilities').reply(200, { capabilities: [] });
    mock.onPost('/v1/databases').reply(201, { id: '3', name: 'new_db', state: 'creating' });

    render(<DatabasesView />);
    await waitFor(() => expect(screen.queryByText('Loading...')).not.toBeInTheDocument());

    fireEvent.click(screen.getByText('Create Database'));

    const input = screen.getByLabelText(/Database Name:/i);
    fireEvent.change(input, { target: { value: 'new_db' } });

    fireEvent.click(screen.getByRole('button', { name: 'Create' }));

    await waitFor(() => {
      expect(mock.history.post.length).toBe(1);
      expect(mock.history.post[0].url).toBe('/v1/databases');
      expect(JSON.parse(mock.history.post[0].data)).toEqual({ name: 'new_db', engine: 'mariadb' });
    });
  });

  it('shows error when creating database fails with 409', async () => {
    mock.onGet('/v1/databases').reply(200, []);
    mock.onGet('/v1/capabilities').reply(200, { capabilities: [] });
    mock.onPost('/v1/databases').reply(409, { error: { message: 'exists_unowned' } });

    render(<DatabasesView />);
    await waitFor(() => expect(screen.queryByText('Loading...')).not.toBeInTheDocument());

    fireEvent.click(screen.getByText('Create Database'));

    const input = screen.getByLabelText(/Database Name:/i);
    fireEvent.change(input, { target: { value: 'dup_db' } });

    fireEvent.click(screen.getByRole('button', { name: 'Create' }));

    await waitFor(() => {
      expect(screen.getByText('exists_unowned')).toBeInTheDocument();
    });
  });

  it('handles Create SQL account flow', async () => {
    mock.onGet('/v1/databases').reply(200, [{ id: '1', name: 'test_db', state: 'ready' }]);
    mock.onGet('/v1/capabilities').reply(200, { capabilities: [] });
    mock.onPost('/v1/databases/test_db/users').reply(201, {
      host: '127.0.0.1', port: 3306, database: 'test_db', username: 'test_user', password: 'one-time-password-123'
    });

    render(<DatabasesView />);
    await waitFor(() => expect(screen.queryByText('Loading...')).not.toBeInTheDocument());

    fireEvent.click(screen.getByText('Create SQL Account'));

    await waitFor(() => {
      expect(screen.getByText('Database Credentials')).toBeInTheDocument();
      expect(screen.getByText('one-time-password-123')).toBeInTheDocument();
      expect(screen.getByText(/Warning: This password is shown exactly once/)).toBeInTheDocument();
    });

    // Close modal
    fireEvent.click(screen.getByRole('button', { name: 'Close' }));
    expect(screen.queryByText('one-time-password-123')).not.toBeInTheDocument();
  });
});
