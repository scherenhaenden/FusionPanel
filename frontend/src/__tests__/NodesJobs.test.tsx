import React from 'react';
import { fireEvent, render, screen, waitFor } from '@testing-library/react';
import '@testing-library/jest-dom';
import { api } from '../api';
import { JobRecord, NodeRecord } from '../types';
import NodesView from '../views/Nodes';
import JobsView from '../views/Jobs';

jest.mock('../api', () => ({
  api: {
    getNodes: jest.fn(),
    getJobs: jest.fn(),
    getJob: jest.fn(),
    getJobLogs: jest.fn(),
    createSystemInfoJob: jest.fn(),
  },
}));

const node: NodeRecord = {
  node_id: 'node-1',
  name: 'local-node',
  state: 'online',
  created_at: '2026-11-15T12:00:00Z',
  last_heartbeat: '2026-11-15T12:01:00Z',
};

const job: JobRecord = {
  id: 'job-1',
  kind: 'agent_operation',
  payload: { operation: 'system.info', payload: {} },
  state: 'succeeded',
  worker_id: 'node-1',
  error_text: null,
  created_at: '2026-11-15T12:00:00Z',
  updated_at: '2026-11-15T12:01:00Z',
};

const apiMock = jest.mocked(api);

describe('Nodes and Jobs views', () => {
  beforeEach(() => {
    jest.clearAllMocks();
  });

  it('shows registered node status and heartbeat time', async () => {
    apiMock.getNodes.mockResolvedValue([node]);

    render(<NodesView />);

    expect(await screen.findByText('local-node')).toBeInTheDocument();
    expect(screen.getByText('online')).toBeInTheDocument();
    expect(screen.getByText('node-1')).toBeInTheDocument();
  });

  it('shows a node API error and permits a retry', async () => {
    apiMock.getNodes
      .mockRejectedValueOnce({ response: { data: { error: { message: 'Node API unavailable' } } } })
      .mockResolvedValueOnce([]);

    render(<NodesView />);

    expect(await screen.findByRole('alert')).toHaveTextContent('Node API unavailable');
    fireEvent.click(screen.getByRole('button', { name: 'Retry' }));
    expect(await screen.findByText('No nodes registered yet.')).toBeInTheDocument();
  });

  it('queues a real system info request and displays its returned status and logs', async () => {
    apiMock.getJobs.mockResolvedValue([job]);
    apiMock.createSystemInfoJob.mockResolvedValue(job);
    apiMock.getJob.mockResolvedValue(job);
    apiMock.getJobLogs.mockResolvedValue([
      { sequence: 1, stream: 'stdout', text: 'System information collected', created_at: job.updated_at },
    ]);

    render(<JobsView />);

    await waitFor(() => expect(apiMock.getJobs).toHaveBeenCalled());
    fireEvent.click(screen.getByRole('button', { name: 'Run System Info Job' }));

    expect(await screen.findByText('System information collected')).toBeInTheDocument();
    expect(apiMock.createSystemInfoJob).toHaveBeenCalledTimes(1);
    expect(apiMock.getJob).toHaveBeenCalledWith('job-1');
    expect(apiMock.getJobLogs).toHaveBeenCalledWith('job-1');
    expect(screen.getAllByText('succeeded').length).toBeGreaterThan(0);
  });

  it('reports enqueue failures instead of showing a completed job', async () => {
    apiMock.getJobs.mockResolvedValue([]);
    apiMock.createSystemInfoJob.mockRejectedValue({
      response: { data: { error: { message: 'Agent API unavailable' } } },
    });

    render(<JobsView />);

    fireEvent.click(screen.getByRole('button', { name: 'Run System Info Job' }));

    expect(await screen.findByRole('alert')).toHaveTextContent('Agent API unavailable');
    expect(apiMock.getJob).not.toHaveBeenCalled();
  });
});
