import React from 'react';
import { fireEvent, render, screen, waitFor, within } from '@testing-library/react';
import '@testing-library/jest-dom';
import MockAdapter from 'axios-mock-adapter';
import { api, WebsiteRecord } from '../api';
import { apiClient } from '../api/client';
import { JobRecord } from '../types';
import WebsitesView from '../views/Websites';

const makeStaticSite = (state: WebsiteRecord['state'] = 'active'): WebsiteRecord => ({
  id: 'site-1',
  domain: 'app.local',
  kind: 'static',
  document_root: '/srv/fusionpanel/sites/app.local',
  state,
  last_error: null,
  created_at: '2026-11-16T10:00:00Z',
  updated_at: '2026-11-16T10:00:00Z',
});

const makeJob = (state: JobRecord['state'] = 'queued'): JobRecord => ({
  id: 'job-1',
  kind: 'site.apply',
  payload: { site_id: 'site-1' },
  state,
  worker_id: null,
  error_text: null,
  created_at: '2026-11-16T10:00:00Z',
  updated_at: '2026-11-16T10:00:00Z',
});

const headerValue = (headers: unknown, name: string): string | undefined => {
  const value = headers as { get?: (key: string) => string | undefined; [key: string]: unknown };
  return value.get?.(name) || value[name] as string | undefined;
};

describe('Websites API and view', () => {
  let mock: MockAdapter;

  beforeEach(() => {
    mock = new MockAdapter(apiClient);
  });

  afterEach(() => {
    mock.restore();
    jest.useRealTimers();
  });

  it('uses the typed website envelopes and idempotency keys for mutations', async () => {
    const site = makeStaticSite();
    mock.onGet('/v1/websites').reply(200, { websites: [site] });
    mock.onGet('/v1/websites/site%2Fone').reply(200, { website: site });
    mock.onPost('/v1/websites').reply(202, { site, job: makeJob() });
    mock.onDelete('/v1/websites/site%2Fone').reply(202, { site, job: makeJob() });

    await expect(api.getSites()).resolves.toEqual([site]);
    await expect(api.getSite('site/one')).resolves.toEqual(site);
    await expect(api.createSite({
      domain: 'app.local',
      kind: 'static',
      document_root: '/srv/fusionpanel/sites/app.local',
    }, 'create-site-key')).resolves.toEqual({ site, job: makeJob() });
    await expect(api.deleteSite('site/one', 'delete-site-key')).resolves.toEqual({ site, job: makeJob() });

    expect(mock.history.get[1].url).toBe('/v1/websites/site%2Fone');
    expect(JSON.parse(mock.history.post[0].data)).toEqual({
      domain: 'app.local',
      kind: 'static',
      document_root: '/srv/fusionpanel/sites/app.local',
    });
    expect(headerValue(mock.history.post[0].headers, 'Idempotency-Key')).toBe('create-site-key');
    expect(mock.history.delete[0].url).toBe('/v1/websites/site%2Fone');
    expect(headerValue(mock.history.delete[0].headers, 'Idempotency-Key')).toBe('delete-site-key');
  });

  it('creates a static site and waits for persisted active state', async () => {
    const provisioningSite = makeStaticSite('provisioning');
    const activeSite = { ...provisioningSite, state: 'active' as const };
    mock.onGet('/v1/websites').reply(200, { websites: [] });
    mock.onPost('/v1/websites').reply(202, { site: provisioningSite, job: makeJob() });
    mock.onGet('/v1/websites/site-1').replyOnce(200, { website: activeSite });
    mock.onGet('/v1/jobs/job-1').replyOnce(200, { job: makeJob('succeeded') });

    render(<WebsitesView />);
    await screen.findByText('No websites found.');
    fireEvent.click(screen.getByRole('button', { name: 'Create website' }));

    const dialog = screen.getByRole('dialog');
    fireEvent.change(within(dialog).getByLabelText('Domain name'), { target: { value: 'App.Local' } });
    expect(within(dialog).getByLabelText('Document root')).toHaveValue('/srv/fusionpanel/sites/app.local');
    fireEvent.click(within(dialog).getByRole('button', { name: 'Create website' }));

    expect(await screen.findByText(/waiting for the agent to report its persisted state/i)).toBeInTheDocument();
    expect(await screen.findByText('Provisioning')).toBeInTheDocument();
    await waitFor(() => expect(screen.getByText('Active')).toBeInTheDocument(), { timeout: 6500 });
    expect(screen.getByText('app.local is active.')).toBeInTheDocument();
    expect(JSON.parse(mock.history.post[0].data)).toEqual({
      domain: 'app.local',
      kind: 'static',
      document_root: '/srv/fusionpanel/sites/app.local',
    });
    expect(headerValue(mock.history.post[0].headers, 'Idempotency-Key')).toBeTruthy();
    expect(mock.history.get.some((request) => request.url === '/v1/jobs/job-1')).toBe(true);
  });

  it('reuses a create idempotency key when retrying the same payload after a network error', async () => {
    const site = makeStaticSite();
    mock.onGet('/v1/websites').reply(200, { websites: [] });
    mock.onPost('/v1/websites').networkErrorOnce().onPost('/v1/websites').reply(202, { site, job: makeJob() });

    render(<WebsitesView />);
    await screen.findByText('No websites found.');
    fireEvent.click(screen.getByRole('button', { name: 'Create website' }));
    const dialog = screen.getByRole('dialog');
    fireEvent.change(within(dialog).getByLabelText('Domain name'), { target: { value: 'app.local' } });
    fireEvent.click(within(dialog).getByRole('button', { name: 'Create website' }));

    await within(dialog).findByRole('alert');
    fireEvent.click(within(dialog).getByRole('button', { name: 'Create website' }));
    expect(await screen.findByText('app.local is active.')).toBeInTheDocument();

    expect(mock.history.post).toHaveLength(2);
    expect(headerValue(mock.history.post[0].headers, 'Idempotency-Key')).toBeTruthy();
    expect(headerValue(mock.history.post[1].headers, 'Idempotency-Key')).toBe(
      headerValue(mock.history.post[0].headers, 'Idempotency-Key'),
    );
  });

  it('uses a fresh create idempotency key when a retry changes the payload', async () => {
    const site = { ...makeStaticSite(), document_root: '/srv/fusionpanel/sites/app.local/public' };
    mock.onGet('/v1/websites').reply(200, { websites: [] });
    mock.onPost('/v1/websites').networkErrorOnce().onPost('/v1/websites').reply(202, { site, job: makeJob() });

    render(<WebsitesView />);
    await screen.findByText('No websites found.');
    fireEvent.click(screen.getByRole('button', { name: 'Create website' }));
    const dialog = screen.getByRole('dialog');
    fireEvent.change(within(dialog).getByLabelText('Domain name'), { target: { value: 'app.local' } });
    fireEvent.click(within(dialog).getByRole('button', { name: 'Create website' }));

    await within(dialog).findByRole('alert');
    fireEvent.change(within(dialog).getByLabelText('Document root'), {
      target: { value: '/srv/fusionpanel/sites/app.local/public' },
    });
    fireEvent.click(within(dialog).getByRole('button', { name: 'Create website' }));
    expect(await screen.findByText('app.local is active.')).toBeInTheDocument();

    expect(mock.history.post).toHaveLength(2);
    expect(JSON.parse(mock.history.post[0].data).document_root).toBe('/srv/fusionpanel/sites/app.local');
    expect(JSON.parse(mock.history.post[1].data).document_root).toBe('/srv/fusionpanel/sites/app.local/public');
    expect(headerValue(mock.history.post[0].headers, 'Idempotency-Key')).not.toBe(
      headerValue(mock.history.post[1].headers, 'Idempotency-Key'),
    );
  });

  it('loads details from the detail endpoint and renders the site configuration', async () => {
    const site = makeStaticSite();
    mock.onGet('/v1/websites').reply(200, { websites: [site] });
    mock.onGet('/v1/websites/site-1').reply(200, { website: site });

    render(<WebsitesView />);
    fireEvent.click(await screen.findByRole('button', { name: 'View details for app.local' }));

    expect(await screen.findByRole('heading', { name: 'app.local' })).toBeInTheDocument();
    expect(screen.getByText('/srv/fusionpanel/sites/app.local')).toBeInTheDocument();
    expect(mock.history.get.some((request) => request.url === '/v1/websites/site-1')).toBe(true);
  });

  it('rejects document-root traversal before sending a static-site request', async () => {
    mock.onGet('/v1/websites').reply(200, { websites: [] });
    render(<WebsitesView />);
    await screen.findByText('No websites found.');
    fireEvent.click(screen.getByRole('button', { name: 'Create website' }));
    const dialog = screen.getByRole('dialog');
    fireEvent.change(within(dialog).getByLabelText('Domain name'), { target: { value: 'app.local' } });
    fireEvent.change(within(dialog).getByLabelText('Document root'), {
      target: { value: '/srv/fusionpanel/sites/app.local/../../outside' },
    });
    fireEvent.click(within(dialog).getByRole('button', { name: 'Create website' }));

    expect(await within(dialog).findByRole('alert')).toHaveTextContent('Document root must not contain traversal segments.');
    expect(mock.history.post).toHaveLength(0);
  });

  it('validates reverse proxy input, submits the typed shape, and displays persisted failure', async () => {
    const failedSite: WebsiteRecord = {
      ...makeStaticSite('failed'),
      kind: 'reverse_proxy',
      upstream_url: 'http://127.0.0.1:8080',
    };
    failedSite.last_error = 'nginx rejected the candidate configuration';
    mock.onGet('/v1/websites').reply(200, { websites: [] });
    mock.onPost('/v1/websites').reply(202, { site: failedSite, job: makeJob('failed') });

    render(<WebsitesView />);
    await screen.findByText('No websites found.');
    fireEvent.click(screen.getByRole('button', { name: 'Create website' }));
    const dialog = screen.getByRole('dialog');
    fireEvent.change(within(dialog).getByLabelText('Domain name'), { target: { value: 'app.local' } });
    fireEvent.click(within(dialog).getByLabelText('Reverse proxy'));

    const upstreamInput = within(dialog).getByLabelText('Upstream URL');
    fireEvent.change(upstreamInput, { target: { value: 'ftp://127.0.0.1:8080' } });
    fireEvent.click(within(dialog).getByRole('button', { name: 'Create website' }));
    expect(await within(dialog).findByRole('alert')).toHaveTextContent('Upstream URL must use HTTP or HTTPS.');
    expect(mock.history.post).toHaveLength(0);

    fireEvent.change(upstreamInput, { target: { value: 'http://127.0.0.1:8080' } });
    fireEvent.click(within(dialog).getByRole('button', { name: 'Create website' }));

    expect(await screen.findByRole('alert')).toHaveTextContent('nginx rejected the candidate configuration');
    expect(JSON.parse(mock.history.post[0].data)).toEqual({
      domain: 'app.local',
      kind: 'reverse_proxy',
      upstream_url: 'http://127.0.0.1:8080',
    });
    expect(JSON.parse(mock.history.post[0].data)).not.toHaveProperty('isPhpEnabled');
    expect(headerValue(mock.history.post[0].headers, 'Idempotency-Key')).toBeTruthy();
  });

  it('keeps the form open and shows API errors when creation is rejected', async () => {
    mock.onGet('/v1/websites').reply(200, { websites: [] });
    mock.onPost('/v1/websites').reply(409, {
      error: { code: 'idempotency_conflict', message: 'This request key belongs to a different site.' },
    });

    render(<WebsitesView />);
    await screen.findByText('No websites found.');
    fireEvent.click(screen.getByRole('button', { name: 'Create website' }));
    const dialog = screen.getByRole('dialog');
    fireEvent.change(within(dialog).getByLabelText('Domain name'), { target: { value: 'app.local' } });
    fireEvent.click(within(dialog).getByRole('button', { name: 'Create website' }));

    expect(await within(dialog).findByRole('alert')).toHaveTextContent('This request key belongs to a different site.');
    expect(screen.getByRole('dialog')).toBeInTheDocument();
  });

  it('deletes a site with an idempotency key and waits until the detail endpoint returns 404', async () => {
    const site = makeStaticSite();
    const deletingSite = { ...site, state: 'deleting' as const };
    mock.onGet('/v1/websites').reply(200, { websites: [site] });
    mock.onDelete('/v1/websites/site-1').reply(202, { site: deletingSite, job: makeJob() });
    mock.onGet('/v1/websites/site-1').replyOnce(404, { error: { code: 'not_found', message: 'Not found' } });

    render(<WebsitesView />);
    fireEvent.click(await screen.findByRole('button', { name: 'Delete app.local' }));
    fireEvent.click(screen.getByRole('button', { name: 'Confirm delete' }));

    expect(await screen.findByText(/Deletion queued for app.local/)).toBeInTheDocument();
    expect(await screen.findByText('No websites found.', {}, { timeout: 6500 })).toBeInTheDocument();
    expect(screen.getByText('app.local was deleted and is no longer available.')).toBeInTheDocument();
    expect(mock.history.delete).toHaveLength(1);
    expect(mock.history.delete[0].url).toBe('/v1/websites/site-1');
    expect(headerValue(mock.history.delete[0].headers, 'Idempotency-Key')).toBeTruthy();
  });

  it('reuses a delete idempotency key after a network error', async () => {
    const site = makeStaticSite();
    const deletingSite = { ...site, state: 'deleting' as const };
    mock.onGet('/v1/websites').reply(200, { websites: [site] });
    mock.onDelete('/v1/websites/site-1').networkErrorOnce()
      .onDelete('/v1/websites/site-1').reply(202, { site: deletingSite, job: makeJob() });

    render(<WebsitesView />);
    fireEvent.click(await screen.findByRole('button', { name: 'Delete app.local' }));
    fireEvent.click(screen.getByRole('button', { name: 'Confirm delete' }));
    await screen.findByRole('alert');

    fireEvent.click(screen.getByRole('button', { name: 'Delete app.local' }));
    fireEvent.click(screen.getByRole('button', { name: 'Confirm delete' }));
    expect(await screen.findByText(/Deletion queued for app.local/)).toBeInTheDocument();

    expect(mock.history.delete).toHaveLength(2);
    expect(headerValue(mock.history.delete[0].headers, 'Idempotency-Key')).toBeTruthy();
    expect(headerValue(mock.history.delete[1].headers, 'Idempotency-Key')).toBe(
      headerValue(mock.history.delete[0].headers, 'Idempotency-Key'),
    );
  });
});
