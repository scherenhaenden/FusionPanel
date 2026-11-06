import React, { useState, useEffect } from 'react';
import { SiteList, Site } from './SiteList';
import { SiteDetail } from './SiteDetail';
import { CreateSiteModal, CreateSiteFormData } from './CreateSiteModal';
import { api } from '../../api';

export const WebsitesView: React.FC = () => {
  const [sites, setSites] = useState<Site[]>([]);
  const [selectedSite, setSelectedSite] = useState<Site | null>(null);
  const [isCreateModalOpen, setIsCreateModalOpen] = useState(false);
  const [isLoading, setIsLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);

  const loadSites = async () => {
    setIsLoading(true);
    setError(null);
    try {
      const data = await api.getSites();
      setSites(data);
    } catch (e: any) {
      setError(e?.response?.data?.error?.message || e.message || 'Failed to load sites');
    } finally {
      setIsLoading(false);
    }
  };

  useEffect(() => {
    loadSites();
  }, []);

  const handleCreateSite = async (data: CreateSiteFormData) => {
    setError(null);
    try {
      const newSite = await api.createSite(data);
      setSites([...sites, newSite]);
      setIsCreateModalOpen(false);
    } catch (e: any) {
      setError(e?.response?.data?.error?.message || e.message || 'Failed to create site');
    }
  };

  return (
    <div className="websites-view">
      <div className="websites-header" style={{ display: 'flex', justifyContent: 'space-between', alignItems: 'center', marginBottom: '1rem' }}>
        <h1>Websites & Virtual Hosts</h1>
        {!selectedSite && (
          <button onClick={() => setIsCreateModalOpen(true)}>Create Site</button>
        )}
      </div>

      {error && <div className="error-banner text-red-500 mb-4">{error} <button onClick={loadSites} className="underline ml-2">Retry</button></div>}

      {isLoading ? (
        <div>Loading...</div>
      ) : selectedSite ? (
        <SiteDetail site={selectedSite} onClose={() => setSelectedSite(null)} />
      ) : (
        <SiteList sites={sites} onSelectSite={setSelectedSite} />
      )}

      <CreateSiteModal
        isOpen={isCreateModalOpen}
        onClose={() => setIsCreateModalOpen(false)}
        onSubmit={handleCreateSite}
      />
    </div>
  );
};

export default WebsitesView;
